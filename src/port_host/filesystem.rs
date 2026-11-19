use super::{identifier, Error, Result};
use cap_fs_ext::{DirExt, FollowSymlinks, MetadataExt, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use std::{
    collections::BTreeMap,
    io::{Seek, SeekFrom, Write},
};

const MAX_HANDLES: usize = 256;
const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_SAVE_ENTRIES: usize = 100_000;
mod save_metadata;
#[cfg(windows)]
mod windows_replace;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Assets,
    Save,
    PreviousSave,
}
struct Mount {
    directory: Dir,
    access: Access,
    quota: Option<u64>,
}
struct File {
    file: std::fs::File,
    grant: u64,
    writable: bool,
}
#[derive(Clone, Copy)]
struct RomView {
    grant: u64,
    start: u64,
    length: u64,
}
#[derive(Default)]
pub(super) struct Files {
    mounts: BTreeMap<u64, Mount>,
    files: BTreeMap<u64, File>,
    roms: BTreeMap<u64, std::fs::File>,
    rom_views: BTreeMap<u64, RomView>,
    volatile: BTreeMap<u64, (std::fs::File, u64)>,
}
fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|_| Error::Failed)
}

/// Same portable path grammar on Windows and Unix. Capabilities additionally
/// prevent escapes through intermediate symlinks or concurrently renamed paths.
pub(super) fn path(bytes: &[u8], allow_empty: bool) -> Result<&str> {
    let s = std::str::from_utf8(bytes).map_err(|_| Error::Invalid)?;
    if allow_empty && s.is_empty() {
        return Ok(".");
    }
    if s.is_empty() || s.len() > 4096 || s.contains(['\\', ':']) || s.chars().any(char::is_control)
    {
        return Err(Error::Invalid);
    }
    for part in s.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with([' ', '.'])
            || part.len() > 255
        {
            return Err(Error::Invalid);
        }
        let stem = part
            .split('.')
            .next()
            .unwrap()
            .trim_end_matches(' ')
            .to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|n| {
                matches!(
                    n,
                    "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
            || part.contains(['<', '>', '"', '|', '?', '*'])
        {
            return Err(Error::Invalid);
        }
    }
    Ok(s)
}
impl Files {
    pub fn grants(&self) -> Vec<(u64, u32)> {
        let mut grants: Vec<_> = self
            .mounts
            .iter()
            .map(|(&id, m)| {
                let kind = match m.access {
                    Access::Assets => 1,
                    Access::Save => 2,
                    Access::PreviousSave => 5,
                };
                (id, kind)
            })
            .chain(self.roms.keys().map(|&id| (id, 3)))
            .chain(self.volatile.keys().map(|&id| (id, 4)))
            .collect();
        // IDs increase as ROMs are granted, so sorting retains their
        // selection order while also ordering the other grant kinds.
        grants.sort_unstable();
        grants
    }
    pub fn grant_directory(&mut self, directory: Dir, access: Access) -> Result<u64> {
        if self.mounts.len() >= 16 {
            return Err(Error::Busy);
        }
        let id = identifier()?;
        self.mounts.insert(
            id,
            Mount {
                directory,
                access,
                quota: if access == Access::Save {
                    Some(1024 * 1024 * 1024)
                } else {
                    None
                },
            },
        );
        Ok(id)
    }
    pub fn grant_save_quota(&mut self, directory: Dir, bytes: u64) -> Result<u64> {
        if bytes > 1024 * 1024 * 1024 * 1024 {
            return Err(Error::Invalid);
        }
        let id = self.grant_directory(directory, Access::Save)?;
        self.mounts.get_mut(&id).unwrap().quota = Some(bytes);
        Ok(id)
    }
    fn quota(&self, grant: u64, growth: u64, new_entry: bool) -> Result<()> {
        let mount = self.mounts.get(&grant).ok_or(Error::Denied)?;
        let Some(limit) = mount.quota else {
            return Ok(());
        };
        let (total, entries) = self.save_usage(grant, mount)?;
        if (new_entry && entries >= MAX_SAVE_ENTRIES)
            || total.checked_add(growth).is_none_or(|total| total > limit)
        {
            return Err(Error::Busy);
        }
        Ok(())
    }
    fn save_usage(&self, grant: u64, mount: &Mount) -> Result<(u64, usize)> {
        let mut total = 0u64;
        let mut entries = 0usize;
        let mut seen = std::collections::BTreeSet::new();
        let mut directories = vec![io(mount.directory.try_clone())?];
        let mut count_file = |metadata: cap_std::fs::Metadata| -> Result<()> {
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || MetadataExt::nlink(&metadata) > 1
            {
                return Err(Error::Denied);
            }
            let identity = (MetadataExt::dev(&metadata), MetadataExt::ino(&metadata));
            if identity.1 == 0 || seen.insert(identity) {
                total = total.checked_add(metadata.len()).ok_or(Error::Busy)?;
            }
            Ok(())
        };
        while let Some(directory) = directories.pop() {
            for entry in io(directory.entries())? {
                entries += 1;
                if entries > MAX_SAVE_ENTRIES {
                    return Err(Error::Busy);
                }
                let entry = io(entry)?;
                let name = entry.file_name();
                let metadata = io(directory.symlink_metadata(&name))?;
                if metadata.file_type().is_symlink() {
                    return Err(Error::Denied);
                }
                if metadata.is_dir() {
                    directories.push(io(directory.open_dir_nofollow(&name))?);
                } else {
                    count_file(metadata)?;
                }
            }
        }
        // Unlinked/replaced files still occupy storage while an open handle
        // retains them. Count each actual inode once, including those handles.
        for file in self.files.values().filter(|file| file.grant == grant) {
            let metadata = io(cap_std::fs::File::from_std(io(file.file.try_clone())?).metadata())?;
            if !metadata.is_file() || MetadataExt::nlink(&metadata) > 1 {
                return Err(Error::Denied);
            }
            let identity = (MetadataExt::dev(&metadata), MetadataExt::ino(&metadata));
            if identity.1 == 0 || seen.insert(identity) {
                total = total.checked_add(metadata.len()).ok_or(Error::Busy)?;
            }
        }
        Ok((total, entries))
    }
    pub fn grant_rom(&mut self, file: std::fs::File) -> Result<u64> {
        if self.roms.len() >= 16 {
            return Err(Error::Busy);
        }
        if !io(file.metadata())?.is_file() {
            return Err(Error::Invalid);
        }
        let id = identifier()?;
        self.roms.insert(id, file);
        Ok(id)
    }
    pub fn grant_volatile(&mut self, bytes: u64) -> Result<u64> {
        if bytes == 0 || bytes > 1024 * 1024 * 1024 {
            return Err(Error::Invalid);
        }
        if !self.volatile.is_empty() {
            return Err(Error::Busy);
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::{AsRawFd, FromRawFd};
            let fd = unsafe {
                libc::memfd_create(
                    c"GamePKG disposable cache".as_ptr(),
                    libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
                )
            };
            if fd < 0 {
                return Err(Error::Unsupported);
            }
            let file = unsafe { std::fs::File::from_raw_fd(fd) };
            io(file.set_len(bytes))?;
            if unsafe {
                libc::fcntl(
                    file.as_raw_fd(),
                    libc::F_ADD_SEALS,
                    libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
                )
            } != 0
            {
                return Err(Error::Failed);
            }
            let id = identifier()?;
            self.volatile.insert(id, (file, bytes));
            Ok(id)
        }
        #[cfg(not(target_os = "linux"))]
        Err(Error::Unsupported)
    }
    pub fn revoke(&mut self, grant: u64) -> Result<()> {
        if self.mounts.remove(&grant).is_some() {
            self.files.retain(|_, f| f.grant != grant);
            Ok(())
        } else if self.roms.remove(&grant).is_some() {
            self.rom_views.retain(|_, view| view.grant != grant);
            Ok(())
        } else if self.volatile.remove(&grant).is_some() {
            Ok(())
        } else {
            Err(Error::Denied)
        }
    }
    fn check_handle_capacity(&self) -> Result<()> {
        if self.files.len() + self.rom_views.len() >= MAX_HANDLES {
            Err(Error::Busy)
        } else {
            Ok(())
        }
    }
    /// An independently closeable byte range over a selected read-only ROM.
    pub fn rom_view(&mut self, grant: u64, start: u64, length: u64) -> Result<u64> {
        let file = self.roms.get(&grant).ok_or(Error::Denied)?;
        let end = start.checked_add(length).ok_or(Error::Invalid)?;
        if end > io(file.metadata())?.len() {
            return Err(Error::Invalid);
        }
        self.check_handle_capacity()?;
        let id = identifier()?;
        self.rom_views.insert(
            id,
            RomView {
                grant,
                start,
                length,
            },
        );
        Ok(id)
    }
    pub fn open(&mut self, grant: u64, mode: u32, name: &[u8]) -> Result<u64> {
        let name = path(name, false)?;
        let mount = self.mounts.get(&grant).ok_or(Error::Denied)?;
        let writable = match mode {
            0 => false,
            1..=3 => true,
            _ => return Err(Error::Invalid),
        };
        if writable && mount.access != Access::Save {
            return Err(Error::Denied);
        }
        if writable {
            self.quota(grant, 0, mode != 2)?;
        }
        self.check_handle_capacity()?;
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        options.write(writable);
        match mode {
            1 => {
                options.create(true);
            }
            3 => {
                options.create_new(true);
            }
            _ => {}
        }
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK).mode(0o600);
        }
        // Reserve the opaque identity before an operation can create a file.
        let id = identifier()?;
        let file = io(mount.directory.open_with(name, &options))?;
        let meta = io(file.metadata())?;
        // Refuse hardlink aliases before granting writes, as well as special
        // files and final symlinks. Capability resolution protects parents.
        if !meta.is_file() || MetadataExt::nlink(&meta) != 1 || (writable && meta.len() > MAX_FILE)
        {
            return Err(Error::Denied);
        }
        self.files.insert(
            id,
            File {
                file: file.into_std(),
                grant,
                writable,
            },
        );
        Ok(id)
    }
    /// Distinguish actual absence from access/I/O errors for save recovery.
    /// Like open, this never treats links, aliases or special files as saves.
    pub fn exists(&self, grant: u64, name: &[u8]) -> Result<bool> {
        let name = path(name, false)?;
        let mount = self.mounts.get(&grant).ok_or(Error::Denied)?;
        let metadata = match mount.directory.symlink_metadata(name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(_) => return Err(Error::Failed),
        };
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || MetadataExt::nlink(&metadata) != 1
        {
            return Err(Error::Denied);
        }
        Ok(true)
    }
    pub fn read(&mut self, handle: u64, offset: u64, length: usize) -> Result<Vec<u8>> {
        if length > super::MAX_MESSAGE {
            return Err(Error::Invalid);
        }
        if let Some(view) = self.rom_views.get(&handle) {
            let file = self.roms.get(&view.grant).ok_or(Error::Denied)?;
            let count = view.length.saturating_sub(offset).min(length as u64) as usize;
            if count == 0 {
                return Ok(Vec::new());
            }
            let mut bytes = vec![0; count];
            let n = io(read_at(file, &mut bytes, view.start + offset))?;
            bytes.truncate(n);
            return Ok(bytes);
        }
        let mut bytes = vec![0; length];
        let n = if let Some(f) = self.files.get_mut(&handle) {
            io(read_at(&f.file, &mut bytes, offset))?
        } else if let Some(f) = self.roms.get_mut(&handle) {
            io(read_at(f, &mut bytes, offset))?
        } else if let Some((f, capacity)) = self.volatile.get(&handle) {
            if offset
                .checked_add(length as u64)
                .is_none_or(|end| end > *capacity)
            {
                return Err(Error::Invalid);
            }
            io(read_at(f, &mut bytes, offset))?
        } else {
            return Err(Error::Denied);
        };
        bytes.truncate(n);
        Ok(bytes)
    }
    pub fn write(&mut self, handle: u64, offset: u64, bytes: &[u8]) -> Result<()> {
        if let Some((file, capacity)) = self.volatile.get_mut(&handle) {
            if bytes.len() > super::MAX_MESSAGE
                || offset
                    .checked_add(bytes.len() as u64)
                    .is_none_or(|end| end > *capacity)
            {
                return Err(Error::Invalid);
            }
            io(file.seek(SeekFrom::Start(offset)))?;
            return io(file.write_all(bytes));
        }
        let f = self.files.get(&handle).ok_or(Error::Denied)?;
        if !f.writable {
            return Err(Error::Denied);
        }
        if offset
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > MAX_FILE)
        {
            return Err(Error::Invalid);
        }
        let length = io(f.file.metadata())?.len();
        self.quota(
            f.grant,
            (offset + bytes.len() as u64).saturating_sub(length),
            false,
        )?;
        let f = self.files.get_mut(&handle).unwrap();
        io(f.file.seek(SeekFrom::Start(offset)))?;
        io(f.file.write_all(bytes))
    }
    pub fn size(&self, handle: u64) -> Result<u64> {
        if let Some(view) = self.rom_views.get(&handle) {
            return Ok(view.length);
        }
        if let Some((_, capacity)) = self.volatile.get(&handle) {
            return Ok(*capacity);
        }
        if let Some(f) = self.files.get(&handle) {
            return Ok(io(f.file.metadata())?.len());
        }
        if let Some(f) = self.roms.get(&handle) {
            return Ok(io(f.metadata())?.len());
        }
        Err(Error::Denied)
    }
    /// Open-file identity, never a path lookup. Seconds use floor division so
    /// pre-epoch timestamps also have a canonical 0..1e9 nanosecond fraction.
    pub fn metadata(&self, handle: u64) -> Result<(u64, i64, u32)> {
        let file = self.files.get(&handle).ok_or(Error::Denied)?;
        let metadata = io(file.file.metadata())?;
        let modified = metadata.modified().map_err(|_| Error::Unsupported)?;
        let (seconds, nanos) = match modified.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => (
                i64::try_from(duration.as_secs()).map_err(|_| Error::Unsupported)?,
                duration.subsec_nanos(),
            ),
            Err(error) => {
                let duration = error.duration();
                let seconds = i64::try_from(duration.as_secs()).map_err(|_| Error::Unsupported)?;
                if duration.subsec_nanos() == 0 {
                    (-seconds, 0)
                } else {
                    (
                        seconds
                            .checked_add(1)
                            .and_then(i64::checked_neg)
                            .ok_or(Error::Unsupported)?,
                        1_000_000_000 - duration.subsec_nanos(),
                    )
                }
            }
        };
        Ok((metadata.len(), seconds, nanos))
    }
    pub fn save_file_info(&self, handle: u64) -> Result<(u64, [u64; 4], u32)> {
        let file = self.files.get(&handle).ok_or(Error::Denied)?;
        self.writable_mount(file.grant)?;
        save_metadata::query(&file.file)
    }
    pub fn save_file_set_attributes(&self, handle: u64, attributes: u32) -> Result<()> {
        if attributes & !save_metadata::SETTABLE != 0 {
            return Err(Error::Invalid);
        }
        let file = self.files.get(&handle).ok_or(Error::Denied)?;
        if !file.writable {
            return Err(Error::Denied);
        }
        self.writable_mount(file.grant)?;
        let meta = io(cap_std::fs::File::from_std(io(file.file.try_clone())?).metadata())?;
        if !meta.is_file() || MetadataExt::nlink(&meta) > 1 {
            return Err(Error::Denied);
        }
        save_metadata::set_attributes(&file.file, attributes)
    }
    /// Retain an opened file or view, including its rights and revocation root.
    pub fn duplicate(&mut self, handle: u64) -> Result<u64> {
        if let Some(view) = self.rom_views.get(&handle).copied() {
            self.check_handle_capacity()?;
            let id = identifier()?;
            self.rom_views.insert(id, view);
            return Ok(id);
        }
        let file = self.files.get(&handle).ok_or(Error::Denied)?;
        self.check_handle_capacity()?;
        let duplicate = File {
            file: io(file.file.try_clone())?,
            grant: file.grant,
            writable: file.writable,
        };
        let id = identifier()?;
        self.files.insert(id, duplicate);
        Ok(id)
    }
    pub fn close(&mut self, handle: u64) -> Result<()> {
        if self.files.remove(&handle).is_some()
            || self.rom_views.remove(&handle).is_some()
            || self.volatile.remove(&handle).is_some()
        {
            Ok(())
        } else {
            Err(Error::Denied)
        }
    }
    pub fn list(&self, grant: u64, name: &[u8], capacity: usize) -> Result<Vec<u8>> {
        let mount = self.mounts.get(&grant).ok_or(Error::Denied)?;
        let directory = io(mount.directory.open_dir_nofollow(path(name, true)?))?;
        let mut entries = Vec::new();
        let mut total = 0usize;
        for item in io(directory.entries())? {
            let item = io(item)?;
            let name = item.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if path(name.as_bytes(), false).is_err() {
                continue;
            }
            let kind = io(item.file_type())?;
            if !kind.is_file() && !kind.is_dir() {
                continue;
            }
            total = total.checked_add(8 + name.len()).ok_or(Error::Invalid)?;
            if total > capacity || entries.len() >= 4096 {
                return Err(Error::Busy);
            }
            entries.push((name.to_owned(), if kind.is_dir() { 2u32 } else { 1u32 }));
        }
        entries.sort();
        let mut out = Vec::with_capacity(total);
        for (name, kind) in entries {
            out.extend(kind.to_le_bytes());
            out.extend((name.len() as u32).to_le_bytes());
            out.extend(name.as_bytes());
        }
        Ok(out)
    }
    fn writable_mount(&self, grant: u64) -> Result<&Mount> {
        let mount = self.mounts.get(&grant).ok_or(Error::Denied)?;
        if mount.access != Access::Save {
            return Err(Error::Denied);
        }
        Ok(mount)
    }
    pub fn save_volume(&self, grant: u64) -> Result<(u64, u64)> {
        let mount = self.writable_mount(grant)?;
        let limit = mount.quota.ok_or(Error::Failed)?;
        let (total, entries) = self.save_usage(grant, mount)?;
        let available = if entries == MAX_SAVE_ENTRIES {
            0
        } else {
            limit.saturating_sub(total)
        };
        Ok((limit, available))
    }
    /// Open a name for observation only, optionally creating its final component.
    /// The result describes the observed namespace entry; it is not a handle.
    pub fn save_directory(&self, grant: u64, name: &[u8], create: bool) -> Result<u32> {
        let mount = self.writable_mount(grant)?;
        if name.is_empty() && !create {
            return Ok(1);
        }
        let name = path(name, false)?;
        let (parent, leaf) = name.rsplit_once('/').unwrap_or(("", name));
        let mut directory = io(mount.directory.try_clone())?;
        for component in parent.split('/').filter(|part| !part.is_empty()) {
            match directory.open_dir_nofollow(component) {
                Ok(opened) => directory = opened,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => {
                    return Ok(0);
                }
                Err(_) => return Err(Error::Failed),
            }
        }
        match directory.open_dir_nofollow(leaf) {
            Ok(_) => return Ok(1),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Failed),
        }
        if !create {
            return Ok(0);
        }
        self.quota(grant, 0, true)?;
        // Resolve the creation from the capability root. The observation's
        // pinned parent could have been moved outside the root in the meantime.
        match mount.directory.create_dir(name) {
            Ok(()) => Ok(2),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                io(mount.directory.open_dir_nofollow(name)).map(|_| 1)
            }
            Err(_) => Err(Error::Failed),
        }
    }
    pub fn mkdir(&self, grant: u64, name: &[u8]) -> Result<()> {
        self.quota(grant, 0, true)?;
        io(self
            .writable_mount(grant)?
            .directory
            .create_dir(path(name, false)?))
    }
    pub fn remove(&self, grant: u64, name: &[u8]) -> Result<()> {
        let mount = self.writable_mount(grant)?;
        let name = path(name, false)?;
        let meta = io(mount.directory.symlink_metadata(name))?;
        if !meta.is_file() || meta.file_type().is_symlink() || MetadataExt::nlink(&meta) != 1 {
            return Err(Error::Denied);
        }
        io(mount.directory.remove_file(name))
    }
    pub fn remove_directory(&self, grant: u64, name: &[u8]) -> Result<()> {
        let mount = self.writable_mount(grant)?;
        let name = path(name, false)?;
        let metadata = io(mount.directory.symlink_metadata(name))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::Denied);
        }
        // Never recursive: the port explicitly enumerates its own bounded tree.
        // A nonempty directory fails without removing any contents.
        io(mount.directory.remove_dir(name))
    }
    pub fn resize(&mut self, handle: u64, size: u64) -> Result<()> {
        let file = self.files.get(&handle).ok_or(Error::Denied)?;
        if !file.writable {
            return Err(Error::Denied);
        }
        if size > MAX_FILE {
            return Err(Error::Invalid);
        }
        self.quota(
            file.grant,
            size.saturating_sub(io(file.file.metadata())?.len()),
            false,
        )?;
        io(file.file.set_len(size))
    }
    pub fn sync(&self, handle: u64) -> Result<()> {
        let file = self.files.get(&handle).ok_or(Error::Denied)?;
        if !file.writable {
            return Err(Error::Denied);
        }
        io(file.file.sync_all())
    }
    /// Flush source contents, then atomically replace within a private save
    /// grant. Each parent is opened once and pinned; rename never follows the
    /// final component. The launcher must exclusively own this private root.
    pub fn replace(
        &self,
        grant: u64,
        source: &[u8],
        destination: &[u8],
        durable: bool,
    ) -> Result<()> {
        let mount = self.writable_mount(grant)?;
        let source = path(source, false)?;
        let destination = path(destination, false)?;
        if source == destination {
            return Err(Error::Invalid);
        }
        // Unsupported platforms must fail before publication. Windows uses
        // the original saveguard's documented write-through replacement.
        #[cfg(not(any(unix, windows)))]
        if durable {
            return Err(Error::Unsupported);
        }
        let parent = |name: &str| -> Result<(Dir, String)> {
            let (base, leaf) = name.rsplit_once('/').unwrap_or((".", name));
            Ok((
                io(mount.directory.open_dir_nofollow(base))?,
                leaf.to_owned(),
            ))
        };
        let (from, source) = parent(source)?;
        let (to, destination) = parent(destination)?;
        let check = |dir: &Dir, name: &str, optional: bool| -> Result<()> {
            match dir.symlink_metadata(name) {
                Ok(meta)
                    if meta.is_file()
                        && !meta.file_type().is_symlink()
                        && MetadataExt::nlink(&meta) == 1 =>
                {
                    Ok(())
                }
                Err(e) if optional && e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(_) => Err(Error::Failed),
                _ => Err(Error::Denied),
            }
        };
        check(&from, &source, false)?;
        check(&to, &destination, true)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = io(from.open_with(&source, &options))?;
        let metadata = io(file.metadata())?;
        if !metadata.is_file() || MetadataExt::nlink(&metadata) != 1 || metadata.len() > MAX_FILE {
            return Err(Error::Denied);
        }
        io(file.sync_all())?;
        let current = io(from.symlink_metadata(&source))?;
        if MetadataExt::dev(&metadata) != MetadataExt::dev(&current)
            || MetadataExt::ino(&metadata) != MetadataExt::ino(&current)
        {
            return Err(Error::Denied);
        }
        // Recheck after the flush; a stale destination must not turn into a
        // followed link. Renaming a final link would not escape containment,
        // but is nevertheless disallowed by the API.
        check(&to, &destination, true)?;
        drop(file);
        #[cfg(not(windows))]
        io(from.rename(&source, &to, &destination))?;
        #[cfg(windows)]
        {
            let from_file = io(from.try_clone())?.into_std_file();
            let to_file = io(to.try_clone())?.into_std_file();
            if let Err(error) =
                windows_replace::replace(&from_file, &source, &to_file, &destination, durable)
            {
                // A namespace change followed by a write-through failure must
                // never be reported as a retryable, unpublished replacement.
                if from
                    .symlink_metadata(&source)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                    && to.symlink_metadata(&destination).is_ok_and(|m| {
                        MetadataExt::dev(&m) == MetadataExt::dev(&metadata)
                            && MetadataExt::ino(&m) == MetadataExt::ino(&metadata)
                    })
                {
                    return Err(Error::CommittedNotDurable);
                }
                if durable
                    && (error.kind() == std::io::ErrorKind::Unsupported
                        || matches!(error.raw_os_error(), Some(1 | 50)))
                {
                    return Err(Error::Unsupported);
                }
                return Err(Error::Failed);
            }
        }
        #[cfg(unix)]
        if durable {
            // Visibility already changed: report this distinct error so the
            // caller does not retry a destructive replace or claim rollback.
            let flush = || -> std::io::Result<()> {
                // Dir may carry an O_PATH descriptor; open a real readable
                // directory descriptor before fsync rather than flushing it.
                from.open(".")?.sync_all()?;
                to.open(".")?.sync_all()
            };
            flush().map_err(|_| Error::CommittedNotDurable)?;
        }
        Ok(())
    }
}

fn read_at(file: &std::fs::File, bytes: &mut [u8], offset: u64) -> std::io::Result<usize> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_at(bytes, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        file.seek_read(bytes, offset)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, bytes, offset);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "positional reads unavailable",
        ))
    }
}

#[cfg(test)]
mod quota_tests {
    use super::*;
    fn grant(root: &std::path::Path, quota: u64) -> (Files, u64) {
        let mut files = Files::default();
        let id = files
            .grant_save_quota(
                Dir::open_ambient_dir(root, cap_std::ambient_authority()).unwrap(),
                quota,
            )
            .unwrap();
        (files, id)
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn disposable_gib_cache_is_sealed_bounded_isolated_and_never_a_save_directory() {
        let mut files = Files::default();
        assert_eq!(files.grant_volatile(0), Err(Error::Invalid));
        assert_eq!(
            files.grant_volatile(1024 * 1024 * 1024 + 1),
            Err(Error::Invalid)
        );
        let id = files.grant_volatile(1024 * 1024 * 1024).unwrap();
        assert_eq!(files.grants(), [(id, 4)]);
        assert_eq!(files.grant_volatile(1), Err(Error::Busy));
        assert_eq!(files.read(id, 1024 * 1024 * 1024 - 1, 1).unwrap(), [0]);
        files.write(id, 1024 * 1024 * 1024 - 1, b"x").unwrap();
        assert_eq!(files.read(id, 1024 * 1024 * 1024 - 1, 1).unwrap(), b"x");
        assert_eq!(
            files.write(id, 1024 * 1024 * 1024, b"x"),
            Err(Error::Invalid)
        );
        assert_eq!(files.read(id, u64::MAX, 1), Err(Error::Invalid));
        assert_eq!(files.resize(id, 1), Err(Error::Denied));
        assert_eq!(files.open(id, 1, b"persistent"), Err(Error::Denied));
        let mut other = Files::default();
        assert_eq!(other.read(id, 0, 1), Err(Error::Denied));
        assert!(files.volatile[&id]
            .0
            .set_len(1024 * 1024 * 1024 + 1)
            .is_err());
        assert!(files.volatile[&id].0.set_len(1).is_err());
        files.close(id).unwrap();
        assert_eq!(files.size(id), Err(Error::Denied));
        let fresh = files.grant_volatile(1).unwrap();
        assert_ne!(id, fresh);
        assert_eq!(files.read(fresh, 0, 1).unwrap(), [0]);
        files.revoke(fresh).unwrap();
        assert_eq!(files.read(fresh, 0, 1), Err(Error::Denied));
    }
    #[test]
    fn save_growth_is_charged_once_per_inode_and_rejected_before_mutation() {
        let root = tempfile::tempdir().unwrap();
        let (mut files, id) = grant(root.path(), 8);
        let a = files.open(id, 1, b"a").unwrap();
        let alias = files.open(id, 1, b"a").unwrap();
        files.write(a, 0, b"12345678").unwrap();
        files.write(alias, 7, b"x").unwrap();
        assert_eq!(files.write(alias, 8, b"!"), Err(Error::Busy));
        assert_eq!(files.resize(a, 9), Err(Error::Busy));
        assert_eq!(std::fs::read(root.path().join("a")).unwrap(), b"1234567x");
        files.resize(a, 4).unwrap();
        let b = files.open(id, 1, b"b").unwrap();
        files.write(b, 0, b"1234").unwrap();
        files.remove(id, b"b").unwrap();
        assert_eq!(files.write(a, 4, b"!"), Err(Error::Busy));
        files.close(b).unwrap();
        files.write(a, 4, b"!").unwrap();
        files.revoke(id).unwrap();
        assert_eq!(files.write(a, 0, b"!"), Err(Error::Denied));
    }
    #[test]
    fn rom_views_share_handle_budget_and_revoke_with_only_their_parent() {
        let root = tempfile::tempdir().unwrap();
        let (mut files, save) = grant(root.path(), 8);
        let opened = files.open(save, 1, b"save").unwrap();
        let rom = files.grant_rom(tempfile::tempfile().unwrap()).unwrap();
        let other = files.grant_rom(tempfile::tempfile().unwrap()).unwrap();
        let other_view = files.rom_view(other, 0, 0).unwrap();
        let first = files.rom_view(rom, 0, 0).unwrap();
        for _ in 0..MAX_HANDLES - 3 {
            files.duplicate(first).unwrap();
        }
        assert_eq!(files.rom_view(rom, 0, 0), Err(Error::Busy));
        assert_eq!(files.open(save, 1, b"new"), Err(Error::Busy));
        assert!(!root.path().join("new").exists());
        files.revoke(rom).unwrap();
        assert_eq!(files.size(first), Err(Error::Denied));
        assert_eq!(files.duplicate(first), Err(Error::Denied));
        assert!(files.read(other_view, 0, 1).unwrap().is_empty());
        assert_eq!(files.size(opened), Ok(0));
        files.open(save, 1, b"new").unwrap();
        assert_eq!(files.rom_views.len(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn quota_walk_rejects_links_and_does_not_follow_outside_the_grant() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), b"secret").unwrap();
        let (mut files, id) = grant(root.path(), 8);
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        assert_eq!(files.open(id, 1, b"new"), Err(Error::Denied));
        assert!(!root.path().join("new").exists());
        std::fs::remove_file(root.path().join("escape")).unwrap();
        std::fs::hard_link(outside.path().join("secret"), root.path().join("alias")).unwrap();
        assert_eq!(files.open(id, 1, b"new"), Err(Error::Denied));
        assert_eq!(
            std::fs::read(outside.path().join("secret")).unwrap(),
            b"secret"
        );
    }
}
