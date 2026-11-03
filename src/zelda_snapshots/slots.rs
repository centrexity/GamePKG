//! Host-owned fixed slots; all operations stay relative to an open directory.
use super::{Context, Snapshot, SIZE};
use anyhow::{ensure, Context as _, Result};
use cap_fs_ext::{DirExt, FollowSymlinks, MetadataExt, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
};

const HEADER: usize = 96;
const MAGIC: &[u8; 8] = b"GPZHSLOT";
pub(crate) const COUNT: u8 = 20;
pub(crate) fn slot_names(manifest: &crate::manifest::Manifest) -> Result<Vec<String>> {
    match Context::new(manifest)? {
        Some(context) => (0..COUNT).map(|slot| context.slot_name(slot)).collect(),
        None => Ok(Vec::new()),
    }
}
struct Slots(Dir);
impl Slots {
    fn open(root: &Path, create: bool) -> Result<Self> {
        let root = Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
        if create {
            let builder = cap_std::fs::DirBuilder::new();
            #[cfg(unix)]
            let builder = {
                use cap_std::fs::DirBuilderExt;
                let mut builder = builder;
                builder.mode(0o700);
                builder
            };
            match root.create_dir_with("states", &builder) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        let directory = root
            .open_dir_nofollow("states")
            .context("snapshot folder is missing or is not a real directory")?;
        #[cfg(unix)]
        {
            use cap_std::fs::MetadataExt as _;
            ensure!(
                directory.dir_metadata()?.mode() & 0o077 == 0,
                "snapshot folder must be private (permissions 0700)"
            );
        }
        Ok(Self(directory))
    }
    fn options() -> OpenOptions {
        let mut options = OpenOptions::new();
        options.follow(FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK).mode(0o600);
        }
        options
    }
    fn read(&self, name: &str) -> Result<cap_std::fs::File> {
        let file = self.0.open_with(name, Self::options().read(true))?;
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file()
                && MetadataExt::nlink(&metadata) == 1
                && metadata.len() == (HEADER + SIZE) as u64,
            "snapshot slot must be an exact-sized regular file without hard links"
        );
        Ok(file)
    }
    fn replace(&self, name: &str, header: &[u8; HEADER], bytes: &[u8]) -> Result<()> {
        // Refuse non-regular existing entries. Atomic rename never follows the
        // destination, including a link substituted after this preflight.
        match self.0.symlink_metadata(name) {
            Ok(metadata) => ensure!(
                metadata.is_file()
                    && !metadata.file_type().is_symlink()
                    && MetadataExt::nlink(&metadata) == 1,
                "snapshot destination must be a regular file without links"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random)
            .map_err(|e| anyhow::anyhow!("snapshot temporary name: {e}"))?;
        let temporary = format!(".snapshot-{}.tmp", hex::encode(random));
        let file = self
            .0
            .open_with(&temporary, Self::options().write(true).create_new(true))?;
        let result = (|| {
            let mut file = file;
            file.write_all(header)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            self.0.rename(&temporary, &self.0, name)?;
            #[cfg(unix)]
            self.0.open(".")?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = self.0.remove_file(&temporary);
        }
        result
    }
}
impl Context {
    fn slot_name(&self, slot: u8) -> Result<String> {
        ensure!(slot < COUNT, "snapshot slot must be between 1 and 20");
        let mut hash = Sha256::new();
        hash.update(b"GamePKG Zelda slots v1\0");
        hash.update(self.owner);
        hash.update(self.schema.to_le_bytes());
        Ok(format!(
            "{}-{:02}.gpstate",
            hex::encode(hash.finalize()),
            slot + 1
        ))
    }
    fn header(&self, slot: u8, snapshot: &Snapshot) -> [u8; HEADER] {
        let mut header = [0; HEADER];
        header[..8].copy_from_slice(MAGIC);
        header[8..12].copy_from_slice(&1u32.to_le_bytes());
        header[12..16].copy_from_slice(&(SIZE as u32).to_le_bytes());
        header[16..48].copy_from_slice(&self.owner);
        header[48..52].copy_from_slice(&self.schema.to_le_bytes());
        header[52..56].copy_from_slice(&u32::from(slot).to_le_bytes());
        header[56..64].copy_from_slice(&snapshot.state.to_le_bytes());
        let mut hash = Sha256::new();
        hash.update(&header[..64]);
        hash.update(&snapshot.bytes);
        header[64..].copy_from_slice(&hash.finalize());
        header
    }
    pub(crate) fn write_slot(&self, root: &Path, slot: u8, snapshot: &Snapshot) -> Result<()> {
        let name = self.slot_name(slot)?;
        let _incoming = self.incoming(snapshot)?;
        let _scratch = self.scratch()?;
        Slots::open(root, true)?.replace(&name, &self.header(slot, snapshot), &snapshot.bytes)
    }
    pub(crate) fn read_slot(&self, root: &Path, slot: u8) -> Result<Snapshot> {
        let name = self.slot_name(slot)?;
        let _scratch = self.scratch()?;
        let mut file = Slots::open(root, false)?.read(&name)?;
        let mut header = [0; HEADER];
        file.read_exact(&mut header)?;
        ensure!(
            &header[..8] == MAGIC
                && header[8..12] == 1u32.to_le_bytes()
                && header[12..16] == (SIZE as u32).to_le_bytes()
                && header[16..48] == self.owner
                && header[48..52] == self.schema.to_le_bytes()
                && header[52..56] == u32::from(slot).to_le_bytes(),
            "snapshot slot identity, version or extent does not match this game"
        );
        let state = u64::from_le_bytes(header[56..64].try_into()?);
        let mut snapshot = self.allocate(state)?;
        file.read_exact(&mut snapshot.bytes)?;
        ensure!(
            file.read(&mut [0u8; 1])? == 0,
            "snapshot slot grew while reading"
        );
        ensure!(
            self.header(slot, &snapshot) == header,
            "snapshot slot checksum mismatch"
        );
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(key: u8, schema: u32) -> Context {
        let mut m =
            crate::manifest::Manifest::parse(include_bytes!("../../ports/zelda3/manifest.toml"))
                .unwrap();
        m.creator_key = hex::encode([key; 32]);
        m.save_schema = schema;
        Context::new(&m).unwrap().unwrap()
    }
    #[test]
    fn all_slots_round_trip_exact_state_and_replace_without_buffer_leaks() {
        let root = tempfile::tempdir().unwrap();
        let a = context(1, 1);
        let mut snapshot = a.allocate(u64::MAX).unwrap();
        snapshot.bytes.fill(0x7b);
        let before = a.budget.used();
        for slot in 0..COUNT {
            a.write_slot(root.path(), slot, &snapshot).unwrap();
            if slot == 0 {
                // Independent LE vector: same bytes and path on ELF64, ELF32
                // and PE32, not merely a round trip through the same codec.
                assert_eq!(
                    a.slot_name(0).unwrap(),
                    "72736d57ca23353bdb1609f4d96c227939449035a38ada0e40c45e2ae391f0a9-01.gpstate"
                );
                let bytes = std::fs::read(root.path().join("states").join(a.slot_name(0).unwrap()))
                    .unwrap();
                assert_eq!(
                    hex::encode(Sha256::digest(bytes)),
                    "0f915c1de23d7b7213ac920e52e680177e13844fb0a95f01844b1e193c28d927"
                );
            }
            let restored = a.read_slot(root.path(), slot).unwrap();
            assert_eq!(restored.state(), u64::MAX);
            assert_eq!(restored.bytes(), snapshot.bytes());
            drop(restored);
            assert_eq!(a.budget.used(), before);
        }
        snapshot.state = 7;
        snapshot.bytes[17] = 23;
        a.write_slot(root.path(), 0, &snapshot).unwrap();
        let restored = context(1, 1).read_slot(root.path(), 0).unwrap();
        assert_eq!(restored.state(), 7);
        assert_eq!(restored.bytes(), snapshot.bytes());
        assert_eq!(
            std::fs::read_dir(root.path().join("states"))
                .unwrap()
                .count(),
            20
        );
        for invalid in [20, 21, u8::MAX] {
            assert!(a.write_slot(root.path(), invalid, &snapshot).is_err());
            assert!(a.read_slot(root.path(), invalid).is_err());
        }
        assert!(context(2, 1).read_slot(root.path(), 0).is_err());
        assert!(context(1, 2).read_slot(root.path(), 0).is_err());
    }
    #[test]
    fn exact_valid_slots_cannot_be_read_or_replaced_through_hard_links() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let context = context(1, 1);
        let snapshot = context.allocate(17).unwrap();
        context.write_slot(root.path(), 0, &snapshot).unwrap();
        let path = root
            .path()
            .join("states")
            .join(context.slot_name(0).unwrap());
        let sentinel = outside.path().join("valid-slot");
        std::fs::rename(&path, &sentinel).unwrap();
        std::fs::hard_link(&sentinel, &path).unwrap();
        let before = std::fs::read(&sentinel).unwrap();
        assert!(context.read_slot(root.path(), 0).is_err());
        assert!(context.write_slot(root.path(), 0, &snapshot).is_err());
        assert_eq!(std::fs::read(&sentinel).unwrap(), before);
    }
    #[test]
    fn header_payload_extent_identity_and_slot_corruption_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let a = context(1, 1);
        let snapshot = a.allocate(17).unwrap();
        a.write_slot(root.path(), 0, &snapshot).unwrap();
        let path = root.path().join("states").join(a.slot_name(0).unwrap());
        let original = std::fs::read(&path).unwrap();
        let before = a.budget.used();
        for offset in (0..HEADER).chain([HEADER, HEADER + SIZE - 1]) {
            let mut broken = original.clone();
            broken[offset] ^= 0x80;
            std::fs::write(&path, broken).unwrap();
            assert!(
                a.read_slot(root.path(), 0).is_err(),
                "corrupt offset {offset}"
            );
            assert_eq!(a.budget.used(), before);
        }
        for length in [0, HEADER - 1, HEADER, HEADER + SIZE - 1, HEADER + SIZE + 1] {
            let mut broken = original.clone();
            broken.resize(length, 0);
            std::fs::write(&path, broken).unwrap();
            assert!(a.read_slot(root.path(), 0).is_err());
        }
        std::fs::write(&path, &original).unwrap();
        let other = root.path().join("states").join(a.slot_name(1).unwrap());
        std::fs::copy(&path, other).unwrap();
        assert!(
            a.read_slot(root.path(), 1).is_err(),
            "renamed slot accepted"
        );
        let other = context(2, 1);
        std::fs::copy(
            &path,
            root.path().join("states").join(other.slot_name(0).unwrap()),
        )
        .unwrap();
        assert!(
            other.read_slot(root.path(), 0).is_err(),
            "foreign creator accepted"
        );
        let other = context(1, 2);
        std::fs::copy(
            &path,
            root.path().join("states").join(other.slot_name(0).unwrap()),
        )
        .unwrap();
        assert!(
            other.read_slot(root.path(), 0).is_err(),
            "foreign schema accepted"
        );
        assert_eq!(a.budget.used(), before);
    }
    #[cfg(unix)]
    #[test]
    fn links_special_files_and_replaced_parent_cannot_escape_slot_directory() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let a = context(1, 1);
        let snapshot = a.allocate(5).unwrap();
        let sentinel = outside.path().join(a.slot_name(0).unwrap());
        std::fs::write(&sentinel, b"untouched").unwrap();
        symlink(outside.path(), root.path().join("states")).unwrap();
        assert!(a.write_slot(root.path(), 0, &snapshot).is_err());
        assert!(a.read_slot(root.path(), 0).is_err());
        std::fs::remove_file(root.path().join("states")).unwrap();
        a.write_slot(root.path(), 0, &snapshot).unwrap();
        let path = root.path().join("states").join(a.slot_name(0).unwrap());
        std::fs::remove_file(&path).unwrap();
        symlink(&sentinel, &path).unwrap();
        assert!(a.read_slot(root.path(), 0).is_err());
        assert!(a.write_slot(root.path(), 0, &snapshot).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::hard_link(&sentinel, &path).unwrap();
        assert!(a.read_slot(root.path(), 0).is_err());
        assert!(a.write_slot(root.path(), 0, &snapshot).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(a.read_slot(root.path(), 0).is_err());
        assert!(a.write_slot(root.path(), 0, &snapshot).is_err());
        std::fs::remove_dir(&path).unwrap();
        let fifo = std::ffi::CString::new(std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()))
            .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(a.read_slot(root.path(), 0).is_err());
        assert!(a.write_slot(root.path(), 0, &snapshot).is_err());
        std::fs::remove_file(&path).unwrap();
        let pinned = Slots::open(root.path(), false).unwrap();
        std::fs::rename(root.path().join("states"), root.path().join("moved")).unwrap();
        symlink(outside.path(), root.path().join("states")).unwrap();
        pinned
            .replace(
                &a.slot_name(0).unwrap(),
                &a.header(0, &snapshot),
                snapshot.bytes(),
            )
            .unwrap();
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"untouched");
        assert!(root
            .path()
            .join("moved")
            .join(a.slot_name(0).unwrap())
            .is_file());
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 1);
    }
}
