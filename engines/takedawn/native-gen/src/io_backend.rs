//! Capability-scoped host storage backend for the reviewed NT I/O frontend.
//! No ambient opens from guest names. C supplies staged bytes and native tokens;
//! every callback releases Rust guards before C can deliver a guest fault.
use cap_fs_ext::{DirExt, MetadataExt};
use cap_std::fs::{Dir, File};
use gamepkg_takedawn_media::{Budget, Lease};
#[cfg(unix)]
use std::io::{Read, Write};
use std::{
    collections::BTreeMap,
    ffi::c_void,
    io,
    io::{Seek, SeekFrom},
    sync::Mutex,
};

const INVALID_HANDLE: u32 = 0xc0000008;
const INVALID_PARAMETER: u32 = 0xc000000d;
const NO_MEMORY: u32 = 0xc0000017;
const ACCESS_DENIED: u32 = 0xc0000022;
const NAME_INVALID: u32 = 0xc0000033;
const NAME_NOT_FOUND: u32 = 0xc0000034;
const NAME_COLLISION: u32 = 0xc0000035;
const PATH_NOT_FOUND: u32 = 0xc000003a;
const SHARING_VIOLATION: u32 = 0xc0000043;
const NOT_DIRECTORY: u32 = 0xc0000103;
const IS_DIRECTORY: u32 = 0xc00000ba;
const UNSUCCESSFUL: u32 = 0xc0000001;
#[derive(Debug)]
enum Error {
    Guest(u32),
    Host(i32),
}
type Result<T> = std::result::Result<T, Error>;
fn os_error(e: io::Error) -> Error {
    Error::Guest(match e.kind() {
        io::ErrorKind::NotFound => NAME_NOT_FOUND,
        io::ErrorKind::AlreadyExists => NAME_COLLISION,
        io::ErrorKind::PermissionDenied => ACCESS_DENIED,
        io::ErrorKind::NotADirectory => NOT_DIRECTORY,
        io::ErrorKind::OutOfMemory => NO_MEMORY,
        _ => UNSUCCESSFUL,
    })
}

/// Explicit emulated volume geometry; host total/free capacity is queried from
/// an actual opened directory. This does not claim the host is a FATX volume.
#[derive(Clone, Copy)]
pub struct Policy {
    pub writable: bool,
    pub bytes_per_sector: u32,
    pub sectors_per_allocation_unit: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity(u64, u64);
enum Object {
    Directory(Dir),
    File(File),
}
impl Object {
    fn directory(&self) -> Result<&Dir> {
        match self {
            Self::Directory(directory) => Ok(directory),
            Self::File(_) => Err(Error::Guest(NOT_DIRECTORY)),
        }
    }
    fn metadata(&self) -> Result<cap_std::fs::Metadata> {
        match self {
            Self::Directory(directory) => directory.dir_metadata(),
            Self::File(file) => file.metadata(),
        }
        .map_err(os_error)
    }
    fn capacity(&self) -> Result<(u128, u128)> {
        match self {
            Self::Directory(directory) => capacity(directory),
            Self::File(file) => capacity(file),
        }
    }
}
struct Handle {
    object: Object,
    identity: Identity,
    access: u32,
    share: u32,
    options: u32,
    position: u64,
    _memory: Lease,
}
struct State {
    next: u64,
    handles: BTreeMap<u64, Handle>,
}
struct Context {
    root: Dir,
    policy: Policy,
    budget: Budget,
    state: Mutex<State>,
    _memory: Lease,
}
/// Owns the backend context. Its Box has a stable address even if Backend moves.
/// Keep it alive and quiescent until every C service/table using its descriptor
/// is released. Dropping it closes all remaining native storage capabilities.
pub struct Backend {
    context: Box<Context>,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    pub context: *mut c_void,
    pub open: Option<
        unsafe extern "C" fn(
            *mut c_void,
            u32,
            u32,
            *const u8,
            u32,
            u32,
            u32,
            u32,
            u32,
            *mut u32,
        ) -> i32,
    >,
    pub volume: Option<unsafe extern "C" fn(*mut c_void, u32, u32, *mut u32) -> i32>,
    pub close: Option<unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut u32) -> i32>,
    pub create: Option<
        unsafe extern "C" fn(*mut c_void, u32, u32, *const u8, u32, *const u32, *mut u32) -> i32,
    >,
    pub query: Option<unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut u32) -> i32>,
    pub stage: Option<unsafe extern "C" fn(*mut c_void, u32, *mut Buffer, *mut u32) -> i32>,
    pub unstage: Option<unsafe extern "C" fn(*mut c_void, *mut Buffer) -> i32>,
    pub transfer: Option<
        unsafe extern "C" fn(*mut c_void, u32, u32, *mut Buffer, *const u32, *mut u32) -> i32,
    >,
}
/// Host-only staging ownership. Never serialize this record into guest memory.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Buffer {
    pub owner: *mut c_void,
    pub data: *mut u8,
    pub length: u32,
}
struct Allocation {
    context: *const Context,
    bytes: Vec<u8>,
    _memory: Lease,
}
impl Backend {
    pub fn new(root: Dir, policy: Policy, budget: Budget) -> anyhow::Result<Self> {
        anyhow::ensure!(
            policy.bytes_per_sector.is_power_of_two()
                && policy.sectors_per_allocation_unit.is_power_of_two(),
            "invalid emulated volume geometry"
        );
        anyhow::ensure!(
            root.dir_metadata()?.is_dir(),
            "storage root must be an opened directory"
        );
        let memory = budget.reserve(4096)?;
        Ok(Self {
            context: Box::new(Context {
                root,
                policy,
                budget,
                state: Mutex::new(State {
                    next: 1,
                    handles: BTreeMap::new(),
                }),
                _memory: memory,
            }),
        })
    }
    /// # Safety
    /// Native callbacks require live, aligned and disjoint caller-owned buffers
    /// matching gamepkg_x86_io.h. The context and all descriptor copies must not
    /// outlive this Backend; no guest pointer or callback belongs in this API.
    pub unsafe fn descriptor(&self) -> Descriptor {
        Descriptor {
            context: std::ptr::from_ref(self.context.as_ref()).cast_mut().cast(),
            open: Some(open),
            volume: Some(volume),
            close: Some(close),
            create: Some(create),
            query: Some(query),
            stage: Some(stage),
            unstage: Some(unstage),
            transfer: Some(transfer),
        }
    }
    pub fn open_handles(&self) -> anyhow::Result<usize> {
        Ok(self
            .context
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("storage context poisoned"))?
            .handles
            .len())
    }
}
fn access_mask(access: u32, writable: bool) -> Result<u32> {
    if access & 0x02000000 != 0 {
        return Err(Error::Host(-2));
    }
    let mut rights = access & 0x0fffffff;
    if access & 0x80000000 != 0 {
        rights |= 0x00120089;
    }
    if access & 0x40000000 != 0 {
        rights |= 0x00120116;
    }
    if access & 0x20000000 != 0 {
        rights |= 0x001200a0;
    }
    if access & 0x10000000 != 0 {
        rights |= 0x001f01ff;
    }
    if rights & !0x001f01ff != 0 {
        return Err(Error::Guest(ACCESS_DENIED));
    }
    if !writable && rights & (2 | 4 | 0x10 | 0x40 | 0x100 | 0x10000 | 0x40000 | 0x80000) != 0 {
        return Err(Error::Guest(ACCESS_DENIED));
    }
    Ok(rights)
}
fn share_classes(access: u32) -> u32 {
    u32::from(access & (1 | 0x20) != 0)
        | (u32::from(access & (2 | 4) != 0) << 1)
        | (u32::from(access & 0x10000 != 0) << 2)
}
fn components(name: &[u8]) -> Result<Vec<&str>> {
    if name.is_empty() {
        return Ok(Vec::new());
    }
    if name.iter().any(|&b| b >= 128) {
        return Err(Error::Host(-2));
    }
    if name.iter().any(|&b| {
        b < 32 || b == 127 || matches!(b, b'/' | b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|')
    }) {
        return Err(Error::Guest(NAME_INVALID));
    }
    let text = std::str::from_utf8(name).map_err(|_| Error::Guest(NAME_INVALID))?;
    let parts: Vec<_> = text.split('\\').collect();
    if parts.iter().any(|p| {
        p.is_empty() || matches!(*p, "." | "..") || p.len() > 42 || p.ends_with([' ', '.'])
    }) {
        return Err(Error::Guest(NAME_INVALID));
    }
    Ok(parts)
}
fn selected_name(
    directory: &Dir,
    name: &str,
    insensitive: bool,
) -> Result<Option<std::ffi::OsString>> {
    let mut selected = None;
    for item in directory.entries().map_err(os_error)? {
        let item = item.map_err(os_error)?;
        let entry = item.file_name();
        let matches = if insensitive {
            entry.to_str().is_some_and(|s| s.eq_ignore_ascii_case(name))
        } else {
            entry == std::ffi::OsStr::new(name)
        };
        if matches {
            if selected.is_some() {
                return Err(Error::Guest(NAME_COLLISION));
            }
            selected = Some(entry);
        }
    }
    Ok(selected)
}
fn child(
    directory: &Dir,
    name: &str,
    insensitive: bool,
    last: bool,
    directory_required: bool,
) -> Result<Dir> {
    let selected = selected_name(directory, name, insensitive)?.ok_or(Error::Guest(if last {
        NAME_NOT_FOUND
    } else {
        PATH_NOT_FOUND
    }))?;
    selected_directory(directory, &selected, last, directory_required)
}
fn selected_directory(
    directory: &Dir,
    selected: &std::ffi::OsStr,
    last: bool,
    directory_required: bool,
) -> Result<Dir> {
    let metadata = directory.symlink_metadata(selected).map_err(os_error)?;
    if metadata.file_type().is_symlink() {
        return Err(Error::Guest(ACCESS_DENIED));
    }
    if !metadata.is_dir() {
        return Err(if !metadata.is_file() {
            Error::Guest(ACCESS_DENIED)
        } else if !last || directory_required {
            Error::Guest(NOT_DIRECTORY)
        } else {
            Error::Host(-2)
        });
    }
    directory.open_dir_nofollow(selected).map_err(os_error)
}
#[derive(Clone, Copy)]
struct CreateRequest {
    parent: u64,
    access: u32,
    share: u32,
    options: u32,
    insensitive: u32,
    disposition: u32,
    attributes: u32,
    allocation: Option<u64>,
}
fn record(
    state: &mut State,
    object: Object,
    rights: u32,
    share: u32,
    options: u32,
    memory: Lease,
) -> Result<u64> {
    let metadata = object.metadata()?;
    let identity = Identity(metadata.dev(), metadata.ino());
    for h in state.handles.values().filter(|h| h.identity == identity) {
        if share_classes(rights) == 0 || share_classes(h.access) == 0 {
            continue;
        }
        if share_classes(rights) & !h.share != 0 || share_classes(h.access) & !share != 0 {
            return Err(Error::Guest(SHARING_VIOLATION));
        }
    }
    let mut id = state.next;
    while state.handles.contains_key(&id) {
        id = id.checked_add(1).unwrap_or(1);
        if id == state.next {
            return Err(Error::Guest(0xc000011f));
        }
    }
    let next = id.checked_add(1).unwrap_or(1);
    state.handles.insert(
        id,
        Handle {
            object,
            identity,
            access: rights,
            share,
            options,
            position: 0,
            _memory: memory,
        },
    );
    state.next = next;
    Ok(id)
}
impl Context {
    fn transfer(
        &self,
        id: u64,
        bytes: &mut [u8],
        write: u32,
        offset: Option<i64>,
    ) -> Result<[u32; 3]> {
        if write > 1 {
            return Err(Error::Host(-1));
        }
        let mut state = self.state.lock().map_err(|_| Error::Host(-1))?;
        let h = state
            .handles
            .get_mut(&id)
            .ok_or(Error::Guest(INVALID_HANDLE))?;
        if h.options & 0x20 == 0 {
            return Err(Error::Host(-2));
        }
        let Object::File(file) = &mut h.object else {
            return Err(Error::Host(-2));
        };
        if (write == 0 && h.access & 1 == 0) || (write != 0 && h.access & (2 | 4) == 0) {
            return Err(Error::Guest(ACCESS_DENIED));
        }
        let append_only = write != 0 && h.access & 2 == 0;
        let offset = if append_only { Some(-1) } else { offset };
        if offset.is_some_and(|n| n < 0 && n != -2 && !(write != 0 && n == -1)) {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        native_transfer(file, bytes, write != 0, offset, &mut h.position)
    }
    fn query(&self, id: u64, class: u32) -> Result<[u32; 14]> {
        if class != 34 {
            return Err(Error::Host(-2));
        }
        let state = self.state.lock().map_err(|_| Error::Host(-1))?;
        let h = state.handles.get(&id).ok_or(Error::Guest(INVALID_HANDLE))?;
        // This supervisor-kernel profile permits metadata queries on an
        // already opened object, including a write-only data handle. Logical
        // read-data rights are not expanded by internal metadata access.
        let (times, length, attributes) = network_information(&h.object)?;
        let cluster = u64::from(self.policy.bytes_per_sector)
            * u64::from(self.policy.sectors_per_allocation_unit);
        let allocation = length
            .checked_add(cluster - 1)
            .map(|n| n / cluster * cluster)
            .filter(|&n| n <= i64::MAX as u64)
            .ok_or(Error::Host(-2))?;
        if length > i64::MAX as u64 {
            return Err(Error::Host(-2));
        }
        let mut out = [0; 14];
        for (i, value) in times.into_iter().chain([allocation, length]).enumerate() {
            out[1 + i * 2] = value as u32;
            out[2 + i * 2] = (value >> 32) as u32;
        }
        out[13] = attributes;
        Ok(out)
    }
    fn create(&self, name: &[u8], r: CreateRequest) -> Result<(u64, u32, u32)> {
        if r.options & 1 != 0 {
            return self
                .create_directory(name, r)
                .map(|(id, info)| (id, 1, info));
        }
        if r.share & !7 != 0 || r.insensitive > 1 || r.disposition > 5 {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        // Regular OPEN/CREATE/OPEN_IF only. Overwrite, supersede, asynchronous
        // operations and nonzero allocation reservations remain explicit gaps.
        if r.options & !0x804062 != 0
            || !matches!(r.disposition, 1..=3)
            || !matches!(r.attributes, 0 | 4 | 0x80)
            || r.allocation.is_some_and(|v| v != 0)
        {
            return Err(Error::Host(-2));
        }
        let rights = access_mask(r.access, self.policy.writable)?;
        if r.options & 0x20 == 0 {
            return Err(Error::Host(-2));
        }
        if rights & 0x100000 == 0 {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        let parts = components(name)?;
        let directory_result = || {
            if r.options & 0x40 != 0 {
                Error::Guest(IS_DIRECTORY)
            } else {
                // Without NON_DIRECTORY, NT can open either object kind.
                // Automatic directory selection needs its own qualification.
                Error::Host(-2)
            }
        };
        let (last, parents) = parts.split_last().ok_or_else(directory_result)?;
        let memory = self
            .budget
            .reserve(4096 + name.len() as u64)
            .map_err(|_| Error::Guest(NO_MEMORY))?;
        let mut state = self.state.lock().map_err(|_| Error::Host(-1))?;
        let mut directory = if r.parent == 0 {
            self.root.try_clone().map_err(os_error)?
        } else {
            state
                .handles
                .get(&r.parent)
                .ok_or(Error::Guest(INVALID_HANDLE))?
                .object
                .directory()?
                .try_clone()
                .map_err(os_error)?
        };
        for part in parents {
            directory = child(&directory, part, r.insensitive != 0, false, true)?;
        }
        let selected = selected_name(&directory, last, r.insensitive != 0)?;
        if let Some(selected) = &selected {
            let metadata = directory.symlink_metadata(selected).map_err(os_error)?;
            if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
                return Err(Error::Guest(ACCESS_DENIED));
            }
            if metadata.is_dir() {
                return Err(directory_result());
            }
            if r.disposition == 2 {
                return Err(Error::Guest(NAME_COLLISION));
            }
        } else if r.disposition == 1 {
            return Err(Error::Guest(NAME_NOT_FOUND));
        } else if !self.policy.writable {
            return Err(Error::Guest(ACCESS_DENIED));
        }
        let (file, information) = regular_file(&directory, selected.as_deref(), last, r, rights)?;
        let metadata = file.metadata().map_err(os_error)?;
        // A hard link may name an object outside this capability. Never permit
        // guest writes to it, even if its in-root name itself is well formed.
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
            return Err(Error::Guest(ACCESS_DENIED));
        }
        #[cfg(windows)]
        {
            use cap_std::fs::MetadataExt as _;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(Error::Guest(ACCESS_DENIED));
            }
        }
        let id = record(
            &mut state,
            Object::File(file),
            rights,
            r.share,
            r.options,
            memory,
        )?;
        Ok((id, 2, information))
    }
    fn open(
        &self,
        parent: u64,
        name: &[u8],
        access: u32,
        share: u32,
        options: u32,
        insensitive: u32,
    ) -> Result<u64> {
        if share & !7 != 0 || insensitive > 1 {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        if options & !0x804061 != 0 {
            return Err(Error::Host(-2));
        }
        if options & 0x41 == 0x41 {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        let rights = access_mask(access, self.policy.writable)?;
        if options & 0x20 != 0 && rights & 0x100000 == 0 {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        let parts = components(name)?;
        let mut directory = {
            let state = self.state.lock().map_err(|_| Error::Host(-1))?;
            if parent == 0 {
                self.root.try_clone().map_err(os_error)?
            } else {
                state
                    .handles
                    .get(&parent)
                    .ok_or(Error::Guest(INVALID_HANDLE))?
                    .object
                    .directory()?
                    .try_clone()
                    .map_err(os_error)?
            }
        };
        let memory = self
            .budget
            .reserve(4096 + name.len() as u64)
            .map_err(|_| Error::Guest(NO_MEMORY))?;
        for (i, part) in parts.iter().enumerate() {
            directory = child(
                &directory,
                part,
                insensitive != 0,
                i + 1 == parts.len(),
                options & 1 != 0,
            )?;
        }
        if options & 0x40 != 0 {
            return Err(Error::Guest(IS_DIRECTORY));
        }
        let mut state = self.state.lock().map_err(|_| Error::Host(-1))?;
        record(
            &mut state,
            Object::Directory(directory),
            rights,
            share,
            options,
            memory,
        )
    }
    fn create_directory(&self, name: &[u8], r: CreateRequest) -> Result<(u64, u32)> {
        if r.share & !7 != 0 || r.insensitive > 1 || r.disposition > 5 {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        if r.options & !0x804061 != 0 {
            return Err(Error::Host(-2));
        }
        if r.options & 0x41 == 0x41 {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        if r.options & 1 == 0 {
            return Err(Error::Host(-2));
        }
        if !matches!(r.disposition, 1..=3) {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        if !matches!(r.attributes, 0 | 0x80) || r.allocation.is_some_and(|v| v != 0) {
            return Err(Error::Host(-2));
        }
        let rights = access_mask(r.access, self.policy.writable)?;
        if r.options & 0x20 != 0 && rights & 0x100000 == 0 {
            return Err(Error::Guest(INVALID_PARAMETER));
        }
        let parts = components(name)?;
        // Charge storage before any mkdir. The guard covers only native work;
        // C gets control back only after every Rust guard has been dropped.
        let memory = self
            .budget
            .reserve(4096 + name.len() as u64)
            .map_err(|_| Error::Guest(NO_MEMORY))?;
        let mut state = self.state.lock().map_err(|_| Error::Host(-1))?;
        let mut directory = if r.parent == 0 {
            self.root.try_clone().map_err(os_error)?
        } else {
            state
                .handles
                .get(&r.parent)
                .ok_or(Error::Guest(INVALID_HANDLE))?
                .object
                .directory()?
                .try_clone()
                .map_err(os_error)?
        };
        let mut information = 1;
        if let Some((last, parents)) = parts.split_last() {
            for part in parents {
                directory = child(&directory, part, r.insensitive != 0, false, true)?;
            }
            let mut selected = selected_name(&directory, last, r.insensitive != 0)?;
            if selected.is_some() && r.disposition == 2 {
                return Err(Error::Guest(NAME_COLLISION));
            }
            if selected.is_none() {
                if r.disposition == 1 {
                    return Err(Error::Guest(NAME_NOT_FOUND));
                }
                if !self.policy.writable {
                    return Err(Error::Guest(ACCESS_DENIED));
                }
                match directory.create_dir(last) {
                    Ok(()) => {
                        information = 2;
                        selected = Some(std::ffi::OsString::from(last));
                    }
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                        if r.disposition == 2 {
                            return Err(Error::Guest(NAME_COLLISION));
                        }
                        selected = selected_name(&directory, last, r.insensitive != 0)?;
                    }
                    Err(e) => return Err(os_error(e)),
                }
            }
            let selected = selected.ok_or(Error::Guest(NAME_NOT_FOUND))?;
            directory = selected_directory(&directory, &selected, true, true)?;
        } else if r.disposition == 2 {
            return Err(Error::Guest(NAME_COLLISION));
        }
        let id = record(
            &mut state,
            Object::Directory(directory),
            rights,
            r.share,
            r.options,
            memory,
        )?;
        Ok((id, information))
    }
    fn volume(&self, id: u64) -> Result<[u32; 7]> {
        let (total, available) = {
            let state = self.state.lock().map_err(|_| Error::Host(-1))?;
            let h = state.handles.get(&id).ok_or(Error::Guest(INVALID_HANDLE))?;
            if h.options & 0x800000 == 0 && h.access & (1 | 0x80) == 0 {
                return Err(Error::Guest(ACCESS_DENIED));
            }
            h.object.capacity()?
        };
        let cluster = u128::from(self.policy.bytes_per_sector)
            * u128::from(self.policy.sectors_per_allocation_unit);
        let total = u64::try_from(total / cluster).map_err(|_| Error::Host(-1))?;
        let available = u64::try_from(available / cluster).map_err(|_| Error::Host(-1))?;
        if total > i64::MAX as u64 || available > total {
            return Err(Error::Host(-1));
        }
        Ok([
            0,
            total as u32,
            (total >> 32) as u32,
            available as u32,
            (available >> 32) as u32,
            self.policy.bytes_per_sector,
            self.policy.sectors_per_allocation_unit,
        ])
    }
    fn close(&self, id: u64, terminal: u32) -> Result<()> {
        if terminal > 1 {
            return Err(Error::Host(-1));
        }
        let handle = self
            .state
            .lock()
            .map_err(|_| Error::Host(-1))?
            .handles
            .remove(&id)
            .ok_or(Error::Guest(INVALID_HANDLE))?;
        drop(handle);
        Ok(())
    }
}
#[cfg(unix)]
fn native_transfer(
    file: &mut File,
    bytes: &mut [u8],
    write: bool,
    offset: Option<i64>,
    position: &mut u64,
) -> Result<[u32; 3]> {
    use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
    let append = write && offset == Some(-1);
    let target = if append {
        file.metadata().map_err(os_error)?.len()
    } else {
        offset.filter(|&n| n >= 0).map_or(*position, |n| n as u64)
    };
    if target > i64::MAX as u64
        || target
            .checked_add(bytes.len() as u64)
            .is_none_or(|n| n > i64::MAX as u64)
    {
        return Err(Error::Guest(INVALID_PARAMETER));
    }
    let flags = fcntl_getfl(&*file).map_err(|e| os_error(e.into()))?;
    let change = append && !flags.contains(OFlags::APPEND);
    if change {
        fcntl_setfl(&*file, flags | OFlags::APPEND).map_err(|e| os_error(e.into()))?;
    }
    // The context mutex exclusively owns this file description. O_APPEND
    // makes the data write atomic with respect to other opened descriptions;
    // no guest memory or callbacks occur while the flag/guard is live.
    let operation = (|| {
        file.seek(SeekFrom::Start(target)).map_err(os_error)?;
        *position = target;
        let result = loop {
            let result = if write {
                file.write(bytes)
            } else {
                file.read(bytes)
            };
            if result
                .as_ref()
                .is_err_and(|e| e.kind() == io::ErrorKind::Interrupted)
            {
                continue;
            }
            break result;
        };
        *position = file.stream_position().map_err(|_| Error::Host(-1))?;
        match result {
            Ok(0) if !write && !bytes.is_empty() => Ok([0xc0000011, 0, 1]),
            Ok(count) => Ok([0, u32::try_from(count).map_err(|_| Error::Host(-1))?, 1]),
            Err(e) => match os_error(e) {
                Error::Guest(status) => Ok([status, 0, 1]),
                error => Err(error),
            },
        }
    })();
    if change {
        fcntl_setfl(&*file, flags).map_err(|_| Error::Host(-1))?;
    }
    operation
}
#[cfg(windows)]
fn native_transfer(
    file: &mut File,
    bytes: &mut [u8],
    write: bool,
    offset: Option<i64>,
    position: &mut u64,
) -> Result<[u32; 3]> {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    struct IoStatus {
        status: usize,
        information: usize,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtReadFile(
            file: *mut c_void,
            event: *mut c_void,
            apc: *mut c_void,
            context: *mut c_void,
            status: *mut IoStatus,
            buffer: *mut c_void,
            length: u32,
            offset: *const i64,
            key: *const u32,
        ) -> i32;
        fn NtWriteFile(
            file: *mut c_void,
            event: *mut c_void,
            apc: *mut c_void,
            context: *mut c_void,
            status: *mut IoStatus,
            buffer: *const c_void,
            length: u32,
            offset: *const i64,
            key: *const u32,
        ) -> i32;
        fn NtWaitForSingleObject(file: *mut c_void, alert: u8, timeout: *const i64) -> i32;
    }
    // Synchronize the native and logical positions before current-offset I/O.
    file.seek(SeekFrom::Start(*position)).map_err(os_error)?;
    let mut status = IoStatus {
        status: usize::MAX,
        information: usize::MAX,
    };
    let pointer = offset.as_ref().map_or(std::ptr::null(), std::ptr::from_ref);
    let length = u32::try_from(bytes.len()).map_err(|_| Error::Host(-1))?;
    let mut rc = unsafe {
        if write {
            NtWriteFile(
                file.as_raw_handle(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut status,
                bytes.as_ptr().cast(),
                length,
                pointer,
                std::ptr::null(),
            )
        } else {
            NtReadFile(
                file.as_raw_handle(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut status,
                bytes.as_mut_ptr().cast(),
                length,
                pointer,
                std::ptr::null(),
            )
        }
    };
    if rc == 0x103 {
        // Owned synchronous handles stay live throughout completion. Never
        // release an in-flight kernel buffer after an impossible wait failure.
        if unsafe { NtWaitForSingleObject(file.as_raw_handle(), 0, std::ptr::null()) } < 0 {
            std::process::abort();
        }
        rc = status.status as u32 as i32;
        if rc == 0x103 {
            std::process::abort();
        }
    }
    *position = file.stream_position().map_err(|_| Error::Host(-1))?;
    if status.status == usize::MAX && status.information == usize::MAX {
        return Ok([rc as u32, 0, 0]);
    }
    if status.status as u32 != rc as u32 || status.information > bytes.len() {
        return Err(Error::Host(-1));
    }
    Ok([rc as u32, status.information as u32, 1])
}
#[cfg(not(any(unix, windows)))]
fn native_transfer(
    _: &mut File,
    _: &mut [u8],
    _: bool,
    _: Option<i64>,
    _: &mut u64,
) -> Result<[u32; 3]> {
    Err(Error::Host(-2))
}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn nt_ticks(seconds: i128, nanoseconds: u32) -> Result<u64> {
    if nanoseconds >= 1_000_000_000 {
        return Err(Error::Host(-1));
    }
    let n = seconds
        .checked_mul(10_000_000)
        .and_then(|v| v.checked_add(i128::from(nanoseconds / 100)))
        .and_then(|v| v.checked_add(116_444_736_000_000_000))
        .filter(|&v| (0..=i128::from(i64::MAX)).contains(&v))
        .ok_or(Error::Host(-2))?;
    Ok(n as u64)
}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn creation_ticks(time: cap_std::time::SystemTime) -> Result<u64> {
    let (seconds, nanos) = match time.into_std().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => (i128::from(d.as_secs()), d.subsec_nanos()),
        Err(e) => {
            let d = e.duration();
            if d.subsec_nanos() == 0 {
                (-i128::from(d.as_secs()), 0)
            } else {
                (
                    -i128::from(d.as_secs()) - 1,
                    1_000_000_000 - d.subsec_nanos(),
                )
            }
        }
    };
    nt_ticks(seconds, nanos)
}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn network_information(object: &Object) -> Result<([u64; 4], u64, u32)> {
    use cap_std::fs::MetadataExt as _;
    let m = object.metadata()?;
    let attributes = match object {
        Object::Directory(_) => 0x10 | u32::from(m.permissions().readonly()),
        Object::File(file) => {
            let mut bytes = [0; 4];
            match rustix::fs::fgetxattr(file, ATTRIBUTE_XATTR, &mut bytes) {
                Ok(4) => {
                    let value = u32::from_le_bytes(bytes);
                    if value & !0xa7 != 0 || (value & 0x80 != 0 && value != 0x80) {
                        return Err(Error::Host(-2));
                    }
                    value
                }
                Ok(_) => return Err(Error::Host(-2)),
                Err(e) if e == rustix::io::Errno::NODATA => {
                    0x20 | u32::from(m.permissions().readonly())
                }
                Err(e) if e == rustix::io::Errno::OPNOTSUPP => return Err(Error::Host(-2)),
                Err(e) => return Err(os_error(e.into())),
            }
        }
    };
    let times = [
        creation_ticks(m.created().map_err(|_| Error::Host(-2))?)?,
        nt_ticks(
            i128::from(m.atime()),
            u32::try_from(m.atime_nsec()).map_err(|_| Error::Host(-1))?,
        )?,
        nt_ticks(
            i128::from(m.mtime()),
            u32::try_from(m.mtime_nsec()).map_err(|_| Error::Host(-1))?,
        )?,
        nt_ticks(
            i128::from(m.ctime()),
            u32::try_from(m.ctime_nsec()).map_err(|_| Error::Host(-1))?,
        )?,
    ];
    // Directories expose no byte stream through this query. Their host
    // directory-entry storage length is not the guest's logical file EOF.
    Ok((times, if m.is_dir() { 0 } else { m.len() }, attributes))
}
#[cfg(windows)]
fn network_information(object: &Object) -> Result<([u64; 4], u64, u32)> {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    struct IoStatus {
        status: usize,
        information: usize,
    }
    #[repr(C, align(8))]
    struct Info {
        times: [u64; 4],
        allocation: i64,
        length: i64,
        attributes: u32,
        padding: u32,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryInformationFile(
            file: *mut c_void,
            status: *mut IoStatus,
            info: *mut c_void,
            length: u32,
            class: u32,
        ) -> i32;
        fn NtWaitForSingleObject(file: *mut c_void, alert: u8, timeout: *const i64) -> i32;
    }
    let handle = match object {
        Object::Directory(d) => d.as_raw_handle(),
        Object::File(f) => f.as_raw_handle(),
    };
    let mut status = IoStatus {
        status: 0,
        information: 0,
    };
    let mut info = Info {
        times: [0; 4],
        allocation: 0,
        length: 0,
        attributes: 0,
        padding: 0,
    };
    let mut rc = unsafe {
        NtQueryInformationFile(
            handle,
            &mut status,
            std::ptr::from_mut(&mut info).cast(),
            std::mem::size_of::<Info>() as u32,
            34,
        )
    };
    if rc == 0x103 {
        let waited = unsafe { NtWaitForSingleObject(handle, 0, std::ptr::null()) };
        if waited < 0 {
            return Err(Error::Guest(waited as u32));
        }
        rc = status.status as u32 as i32;
    }
    if rc < 0 {
        return Err(Error::Guest(rc as u32));
    }
    if rc != 0 || status.information != 56 || info.length < 0 || info.allocation < 0 {
        return Err(Error::Host(-1));
    }
    // Allocation is normalized by Context to its explicit virtual geometry;
    // timestamps, EOF and attributes come directly from the opened host object.
    Ok((info.times, info.length as u64, info.attributes))
}
#[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
fn network_information(_: &Object) -> Result<([u64; 4], u64, u32)> {
    Err(Error::Host(-2))
}
// The virtual FATX flags live on the opened inode, never a guest-visible sidecar.
// Unsupported host filesystems fail explicitly before a new name is published.
#[cfg(any(target_os = "linux", target_os = "android"))]
const ATTRIBUTE_XATTR: &str = "user.gamepkg.fatx.attributes.v1";
#[cfg(any(target_os = "linux", target_os = "android"))]
fn regular_file(
    directory: &Dir,
    selected: Option<&std::ffi::OsStr>,
    name: &str,
    r: CreateRequest,
    rights: u32,
) -> Result<(File, u32)> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
    use rustix::fs::{AtFlags, Mode, OFlags, XattrFlags};
    use std::os::fd::AsRawFd;
    if let Some(selected) = selected {
        let mut options = cap_std::fs::OpenOptions::new();
        // No truncation or creation here. Validate the actual opened inode
        // before recording it, including a race replacing a regular leaf.
        options
            .read(true)
            .write(rights & (2 | 4) != 0)
            .follow(FollowSymlinks::No)
            .nonblock(true)
            .sync(r.options & 2 != 0);
        return directory
            .open_with(selected, &options)
            .map(|f| (f, 1))
            .map_err(os_error);
    }
    let mut flags = OFlags::TMPFILE | OFlags::RDWR | OFlags::CLOEXEC;
    if r.options & 2 != 0 {
        flags |= OFlags::SYNC;
    }
    let fd = rustix::fs::openat(directory, ".", flags, Mode::RUSR | Mode::WUSR).map_err(|e| {
        if matches!(e, rustix::io::Errno::OPNOTSUPP | rustix::io::Errno::INVAL) {
            Error::Host(-2)
        } else {
            os_error(e.into())
        }
    })?;
    let file = File::from_std(std::fs::File::from(fd));
    // Newly created files carry ARCHIVE, plus the supported requested flags.
    let attributes = (0x20 | (r.attributes & !0x80)).to_le_bytes();
    rustix::fs::fsetxattr(&file, ATTRIBUTE_XATTR, &attributes, XattrFlags::CREATE).map_err(
        |e| {
            if e == rustix::io::Errno::OPNOTSUPP {
                Error::Host(-2)
            } else {
                os_error(e.into())
            }
        },
    )?;
    let parent_sync = if r.options & 2 != 0 {
        file.sync_all().map_err(os_error)?;
        Some(
            rustix::fs::openat(
                directory,
                ".",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| os_error(e.into()))?,
        )
    } else {
        None
    };
    // The source is a trusted, live descriptor in procfs, not a guest path.
    // linkat is no-replace: a racing destination never gets overwritten.
    let source = format!("/proc/self/fd/{}", file.as_raw_fd());
    match rustix::fs::linkat(
        rustix::fs::CWD,
        source,
        directory,
        name,
        AtFlags::SYMLINK_FOLLOW,
    ) {
        Ok(()) => {}
        Err(e) if e == rustix::io::Errno::EXIST && r.disposition == 3 => {
            let selected = selected_name(directory, name, r.insensitive != 0)?
                .ok_or(Error::Guest(NAME_NOT_FOUND))?;
            return regular_file(directory, Some(&selected), name, r, rights);
        }
        Err(e) => return Err(os_error(e.into())),
    }
    if let Some(parent_sync) = parent_sync {
        // A durability error is a real failure; the published file may persist.
        // Never roll back by unlinking a name another host process could replace.
        rustix::fs::fsync(parent_sync).map_err(|e| os_error(e.into()))?;
    }
    Ok((file, 2))
}
#[cfg(windows)]
fn regular_file(
    directory: &Dir,
    selected: Option<&std::ffi::OsStr>,
    name: &str,
    r: CreateRequest,
    _rights: u32,
) -> Result<(File, u32)> {
    use std::os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    };
    #[repr(C)]
    struct Unicode {
        length: u16,
        capacity: u16,
        buffer: *mut u16,
    }
    #[repr(C)]
    struct Attributes {
        length: u32,
        root: *mut c_void,
        name: *mut Unicode,
        flags: u32,
        security: *mut c_void,
        qos: *mut c_void,
    }
    #[repr(C)]
    struct IoStatus {
        status: usize,
        information: usize,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtCreateFile(
            handle: *mut *mut c_void,
            access: u32,
            attributes: *mut Attributes,
            status: *mut IoStatus,
            allocation: *const i64,
            file_attributes: u32,
            share: u32,
            disposition: u32,
            options: u32,
            ea: *const c_void,
            ea_bytes: u32,
        ) -> i32;
    }
    let mut text: Vec<u16> = selected
        .unwrap_or_else(|| std::ffi::OsStr::new(name))
        .encode_wide()
        .collect();
    let mut unicode = Unicode {
        length: (text.len() * 2) as u16,
        capacity: (text.len() * 2) as u16,
        buffer: text.as_mut_ptr(),
    };
    let mut attributes = Attributes {
        length: std::mem::size_of::<Attributes>() as u32,
        root: directory.as_raw_handle(),
        name: &mut unicode,
        flags: 0,
        security: std::ptr::null_mut(),
        qos: std::ptr::null_mut(),
    };
    let mut status = IoStatus {
        status: 0,
        information: 0,
    };
    let mut handle = std::ptr::null_mut();
    // Native flags persist the attributes and WRITE_THROUGH behavior. Open the
    // reparse point itself so a replaced leaf never redirects outside the root.
    let rc = unsafe {
        NtCreateFile(
            &mut handle,
            r.access | 0x80,
            &mut attributes,
            &mut status,
            std::ptr::null(),
            r.attributes,
            r.share,
            r.disposition,
            r.options | 0x200040,
            std::ptr::null(),
            0,
        )
    };
    if rc < 0 {
        return Err(Error::Guest(rc as u32));
    }
    if handle.is_null() {
        return Err(Error::Host(-1));
    }
    let file = File::from_std(unsafe { std::fs::File::from_raw_handle(handle) });
    if rc != 0 || !matches!(status.information, 1 | 2) {
        return Err(Error::Host(-1));
    }
    Ok((file, status.information as u32))
}
#[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
fn regular_file(
    _: &Dir,
    _: Option<&std::ffi::OsStr>,
    _: &str,
    _: CreateRequest,
    _: u32,
) -> Result<(File, u32)> {
    Err(Error::Host(-2))
}
#[cfg(unix)]
fn capacity(directory: &impl std::os::fd::AsFd) -> Result<(u128, u128)> {
    let s = rustix::fs::fstatvfs(directory).map_err(|_| Error::Guest(UNSUCCESSFUL))?;
    let unit = u128::from(s.f_frsize);
    if unit == 0 || s.f_bavail > s.f_blocks {
        return Err(Error::Host(-1));
    }
    Ok((u128::from(s.f_blocks) * unit, u128::from(s.f_bavail) * unit))
}
#[cfg(windows)]
fn capacity(directory: &impl std::os::windows::io::AsRawHandle) -> Result<(u128, u128)> {
    use std::ffi::c_void;
    #[repr(C)]
    struct IoStatus {
        status: usize,
        information: usize,
    }
    #[repr(C)]
    struct Size {
        total: i64,
        available: i64,
        sectors: u32,
        bytes: u32,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryVolumeInformationFile(
            file: *mut c_void,
            status: *mut IoStatus,
            info: *mut c_void,
            length: u32,
            class: u32,
        ) -> i32;
        fn NtWaitForSingleObject(handle: *mut c_void, alert: u8, timeout: *const i64) -> i32;
    }
    let mut status = IoStatus {
        status: 0,
        information: 0,
    };
    let mut size = Size {
        total: 0,
        available: 0,
        sectors: 0,
        bytes: 0,
    };
    // The capability is an actual opened directory. It remains live while any
    // pending native query is waited to completion; no guest fault can unwind it.
    let mut rc = unsafe {
        NtQueryVolumeInformationFile(
            directory.as_raw_handle(),
            &mut status,
            std::ptr::from_mut(&mut size).cast(),
            std::mem::size_of::<Size>() as u32,
            3,
        )
    };
    if rc == 0x103 {
        let waited =
            unsafe { NtWaitForSingleObject(directory.as_raw_handle(), 0, std::ptr::null()) };
        if waited < 0 {
            return Err(Error::Guest(waited as u32));
        }
        rc = status.status as u32 as i32;
    }
    if rc < 0 {
        return Err(Error::Guest(rc as u32));
    }
    if status.information != std::mem::size_of::<Size>()
        || size.total < 0
        || size.available < 0
        || size.available > size.total
        || !size.sectors.is_power_of_two()
        || !size.bytes.is_power_of_two()
    {
        return Err(Error::Host(-1));
    }
    let unit = u128::from(size.sectors) * u128::from(size.bytes);
    Ok((size.total as u128 * unit, size.available as u128 * unit))
}
#[cfg(not(any(unix, windows)))]
fn capacity<T>(_: &T) -> Result<(u128, u128)> {
    Err(Error::Host(-2))
}
fn native_buffer(p: *const u8, len: usize) -> bool {
    len == 0 || (!p.is_null() && (p as usize).checked_add(len - 1).is_some())
}
unsafe fn callback<const N: usize>(
    context: *mut c_void,
    out: *mut u32,
    f: impl FnOnce(&Context) -> Result<[u32; N]>,
) -> i32 {
    if context.is_null()
        || out.is_null()
        || (out as usize) % std::mem::align_of::<u32>() != 0
        || !native_buffer(out.cast(), N * 4)
    {
        return -1;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f(unsafe { &*context.cast::<Context>() })
    }));
    let words = match result {
        Ok(Ok(words)) => words,
        Ok(Err(Error::Guest(status))) => {
            let mut words = [0; N];
            words[0] = status;
            words
        }
        Ok(Err(Error::Host(status))) => return status,
        Err(_) => return -1,
    };
    unsafe { std::ptr::copy_nonoverlapping(words.as_ptr(), out, N) };
    0
}
unsafe fn allocation<'a>(context: *mut c_void, buffer: *mut Buffer) -> Result<&'a mut Allocation> {
    if buffer.is_null() || (buffer as usize) % std::mem::align_of::<Buffer>() != 0 {
        return Err(Error::Host(-1));
    }
    let buffer = unsafe { &*buffer };
    if buffer.owner.is_null() || (buffer.owner as usize) % std::mem::align_of::<Allocation>() != 0 {
        return Err(Error::Host(-1));
    }
    // Only a live owner returned by stage may enter this host-only API. C must
    // keep Buffer/header/data disjoint and exclusively borrowed until unstage.
    let a = unsafe { &mut *buffer.owner.cast::<Allocation>() };
    if a.context != context.cast::<Context>()
        || a.bytes.len() != buffer.length as usize
        || a.bytes.as_mut_ptr() != buffer.data
    {
        return Err(Error::Host(-1));
    }
    Ok(a)
}
unsafe extern "C" fn stage(
    context: *mut c_void,
    length: u32,
    buffer: *mut Buffer,
    status: *mut u32,
) -> i32 {
    unsafe {
        callback(context, status, |c| {
            if buffer.is_null() || (buffer as usize) % std::mem::align_of::<Buffer>() != 0 {
                return Err(Error::Host(-1));
            }
            let memory = c
                .budget
                .reserve(u64::from(length) + std::mem::size_of::<Allocation>() as u64 + 64)
                .map_err(|_| Error::Guest(NO_MEMORY))?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length as usize)
                .map_err(|_| Error::Guest(NO_MEMORY))?;
            bytes.resize(length as usize, 0);
            let mut a = Box::new(Allocation {
                context: std::ptr::from_ref(c),
                bytes,
                _memory: memory,
            });
            let data = a.bytes.as_mut_ptr();
            let owner = Box::into_raw(a).cast();
            buffer.write(Buffer {
                owner,
                data,
                length,
            });
            Ok([0])
        })
    }
}
unsafe extern "C" fn unstage(context: *mut c_void, buffer: *mut Buffer) -> i32 {
    let mut status = 0;
    unsafe {
        callback(context, &mut status, |_| {
            let _ = allocation(context, buffer)?;
            let owner = (*buffer).owner;
            buffer.write(Buffer {
                owner: std::ptr::null_mut(),
                data: std::ptr::null_mut(),
                length: 0,
            });
            drop(Box::from_raw(owner.cast::<Allocation>()));
            Ok([0])
        })
    }
}
unsafe extern "C" fn transfer(
    context: *mut c_void,
    low: u32,
    high: u32,
    buffer: *mut Buffer,
    request: *const u32,
    out: *mut u32,
) -> i32 {
    unsafe {
        callback(context, out, |c| {
            if request.is_null()
                || (request as usize) % std::mem::align_of::<u32>() != 0
                || !native_buffer(request.cast(), 16)
            {
                return Err(Error::Host(-1));
            }
            let r = std::slice::from_raw_parts(request, 4);
            if r[3] > 1 {
                return Err(Error::Host(-1));
            }
            let a = allocation(context, buffer)?;
            let offset = (r[3] != 0).then_some((u64::from(r[1]) | (u64::from(r[2]) << 32)) as i64);
            c.transfer(
                u64::from(low) | (u64::from(high) << 32),
                &mut a.bytes,
                r[0],
                offset,
            )
        })
    }
}
unsafe extern "C" fn open(
    context: *mut c_void,
    low: u32,
    high: u32,
    name: *const u8,
    length: u32,
    access: u32,
    share: u32,
    options: u32,
    insensitive: u32,
    out: *mut u32,
) -> i32 {
    unsafe {
        callback(context, out, |c| {
            if length > 65535 || !native_buffer(name, length as usize) {
                return Err(Error::Host(-1));
            }
            let name = if length == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(name, length as usize)
            };
            let id = c.open(
                u64::from(low) | (u64::from(high) << 32),
                name,
                access,
                share,
                options,
                insensitive,
            )?;
            Ok([0, id as u32, (id >> 32) as u32, 1])
        })
    }
}
unsafe extern "C" fn create(
    context: *mut c_void,
    low: u32,
    high: u32,
    name: *const u8,
    length: u32,
    request: *const u32,
    out: *mut u32,
) -> i32 {
    unsafe {
        callback(context, out, |c| {
            if length > 65535
                || !native_buffer(name, length as usize)
                || request.is_null()
                || (request as usize) % std::mem::align_of::<u32>() != 0
                || !native_buffer(request.cast(), 36)
            {
                return Err(Error::Host(-1));
            }
            let r = std::slice::from_raw_parts(request, 9);
            if r[8] > 1 {
                return Err(Error::Host(-1));
            }
            let name = if length == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(name, length as usize)
            };
            let (id, kind, information) = c.create(
                name,
                CreateRequest {
                    parent: u64::from(low) | (u64::from(high) << 32),
                    access: r[0],
                    share: r[1],
                    options: r[2],
                    insensitive: r[3],
                    disposition: r[4],
                    attributes: r[5],
                    allocation: if r[8] == 0 {
                        None
                    } else {
                        Some(u64::from(r[6]) | (u64::from(r[7]) << 32))
                    },
                },
            )?;
            Ok([0, id as u32, (id >> 32) as u32, kind, information])
        })
    }
}
unsafe extern "C" fn volume(context: *mut c_void, low: u32, high: u32, out: *mut u32) -> i32 {
    unsafe {
        callback(context, out, |c| {
            c.volume(u64::from(low) | (u64::from(high) << 32))
        })
    }
}
unsafe extern "C" fn query(
    context: *mut c_void,
    low: u32,
    high: u32,
    class: u32,
    out: *mut u32,
) -> i32 {
    unsafe {
        callback(context, out, |c| {
            c.query(u64::from(low) | (u64::from(high) << 32), class)
        })
    }
}
unsafe extern "C" fn close(
    context: *mut c_void,
    low: u32,
    high: u32,
    terminal: u32,
    out: *mut u32,
) -> i32 {
    unsafe {
        callback(context, out, |c| {
            c.close(u64::from(low) | (u64::from(high) << 32), terminal)?;
            Ok([0])
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn backend(root: &std::path::Path, writable: bool) -> Backend {
        Backend::new(
            Dir::open_ambient_dir(root, cap_std::ambient_authority()).unwrap(),
            Policy {
                writable,
                bytes_per_sector: 512,
                sectors_per_allocation_unit: 32,
            },
            Budget::new(2 * 1024 * 1024),
        )
        .unwrap()
    }
    fn expect<T>(result: Result<T>, status: u32) {
        match result {
            Err(Error::Guest(s)) => assert_eq!(s, status),
            _ => panic!("expected guest status {status:08x}"),
        }
    }
    fn create_request(disposition: u32) -> CreateRequest {
        CreateRequest {
            parent: 0,
            access: 0x100001,
            share: 3,
            options: 0x4021,
            insensitive: 1,
            disposition,
            attributes: 0x80,
            allocation: None,
        }
    }
    fn file_request(disposition: u32) -> CreateRequest {
        CreateRequest {
            access: 0x40100000,
            share: 1,
            options: 0x22,
            attributes: 4,
            ..create_request(disposition)
        }
    }
    #[test]
    fn staging_is_budgeted_zero_initialized_and_exclusively_owned() {
        let root = tempfile::tempdir().unwrap();
        let b = backend(root.path(), true);
        let other = backend(root.path(), true);
        let d = unsafe { b.descriptor() };
        let before = b.context.budget.used();
        for length in [0, 1, 65537] {
            let mut buffer = Buffer {
                owner: std::ptr::null_mut(),
                data: std::ptr::null_mut(),
                length: 0,
            };
            let mut status = 0xabababab;
            assert_eq!(
                unsafe { d.stage.unwrap()(d.context, length, &mut buffer, &mut status) },
                0
            );
            assert_eq!(status, 0);
            assert!(!buffer.owner.is_null());
            assert_eq!(buffer.length, length);
            assert!(b.context.budget.used() >= before + u64::from(length));
            assert!(
                unsafe { std::slice::from_raw_parts(buffer.data, length as usize) }
                    .iter()
                    .all(|&b| b == 0)
            );
            let o = unsafe { other.descriptor() };
            assert_eq!(unsafe { o.unstage.unwrap()(o.context, &mut buffer) }, -1);
            assert!(!buffer.owner.is_null());
            assert_eq!(unsafe { d.unstage.unwrap()(d.context, &mut buffer) }, 0);
            assert!(buffer.owner.is_null() && buffer.data.is_null() && buffer.length == 0);
            assert_eq!(b.context.budget.used(), before);
            assert_eq!(unsafe { d.unstage.unwrap()(d.context, &mut buffer) }, -1);
        }
        let mut buffer = Buffer {
            owner: std::ptr::null_mut(),
            data: std::ptr::null_mut(),
            length: 0,
        };
        let mut status = 0;
        assert_eq!(
            unsafe { d.stage.unwrap()(d.context, u32::MAX, &mut buffer, &mut status) },
            0
        );
        assert_eq!(status, NO_MEMORY);
        assert!(buffer.owner.is_null());
        assert_eq!(b.context.budget.used(), before);
    }
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    #[test]
    fn staged_ffi_roundtrip_is_full_width_and_releases_its_budget() {
        let root = tempfile::tempdir().unwrap();
        let b = backend(root.path(), true);
        let (id, _, _) = b
            .context
            .create(
                b"Roundtrip",
                CreateRequest {
                    access: 0xc0100000,
                    share: 7,
                    ..file_request(3)
                },
            )
            .unwrap();
        let d = unsafe { b.descriptor() };
        let before = b.context.budget.used();
        let mut buffer = Buffer {
            owner: std::ptr::null_mut(),
            data: std::ptr::null_mut(),
            length: 0,
        };
        let mut status = 0;
        let mut out = [0xabababab; 3];
        assert_eq!(
            unsafe { d.stage.unwrap()(d.context, 65537, &mut buffer, &mut status) },
            0
        );
        assert_eq!(status, 0);
        for (i, value) in
            unsafe { std::slice::from_raw_parts_mut(buffer.data, buffer.length as usize) }
                .iter_mut()
                .enumerate()
        {
            *value = (i.wrapping_mul(17) + 3) as u8;
        }
        assert_eq!(
            unsafe {
                d.transfer.unwrap()(
                    d.context,
                    id as u32,
                    (id >> 32) as u32,
                    &mut buffer,
                    [1, 0, 0, 0].as_ptr(),
                    out.as_mut_ptr(),
                )
            },
            0
        );
        assert_eq!(out, [0, 65537, 1]);
        assert_eq!(unsafe { d.unstage.unwrap()(d.context, &mut buffer) }, 0);
        assert_eq!(b.context.budget.used(), before);
        assert_eq!(
            unsafe { d.stage.unwrap()(d.context, 65550, &mut buffer, &mut status) },
            0
        );
        assert_eq!(
            unsafe {
                d.transfer.unwrap()(
                    d.context,
                    id as u32,
                    (id >> 32) as u32,
                    &mut buffer,
                    [0, 0, 0, 1].as_ptr(),
                    out.as_mut_ptr(),
                )
            },
            0
        );
        assert_eq!(out, [0, 65537, 1]);
        for (i, &value) in
            unsafe { std::slice::from_raw_parts(buffer.data, buffer.length as usize) }
                .iter()
                .enumerate()
        {
            assert_eq!(
                value,
                if i < 65537 {
                    (i.wrapping_mul(17) + 3) as u8
                } else {
                    0
                }
            );
        }
        out = [0xabababab; 3];
        assert_eq!(
            unsafe {
                d.transfer.unwrap()(
                    d.context,
                    id as u32,
                    (id >> 32) as u32,
                    &mut buffer,
                    [0, 0, 0, 2].as_ptr(),
                    out.as_mut_ptr(),
                )
            },
            -1
        );
        assert_eq!(out, [0xabababab; 3]);
        assert_eq!(unsafe { d.unstage.unwrap()(d.context, &mut buffer) }, 0);
        assert_eq!(b.context.budget.used(), before);
        b.context.close(id, 0).unwrap();
        assert_eq!(
            std::fs::metadata(root.path().join("Roundtrip"))
                .unwrap()
                .len(),
            65537
        );
        assert_eq!(b.open_handles().unwrap(), 0);
    }
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    #[test]
    fn synchronous_transfers_match_measured_native_offset_append_and_eof_cases() {
        let root = tempfile::tempdir().unwrap();
        let b = backend(root.path(), true);
        let mut cases = 0;
        for mode in 0..3 {
            for write in 0..2 {
                for length in [3usize, 0] {
                    for offset in [
                        None,
                        Some(-1),
                        Some(-2),
                        Some(-3),
                        Some(0),
                        Some(8),
                        Some(10),
                        Some(12),
                    ] {
                        let name = format!("Transfer{cases}");
                        let (seed, _, _) = b
                            .context
                            .create(
                                name.as_bytes(),
                                CreateRequest {
                                    access: 0xc0100000,
                                    share: 7,
                                    ..file_request(3)
                                },
                            )
                            .unwrap();
                        assert_eq!(
                            b.context
                                .transfer(seed, &mut b"0123456789".to_vec(), 1, None)
                                .unwrap(),
                            [0, 10, 1]
                        );
                        b.context.close(seed, 0).unwrap();
                        let access = match mode {
                            0 => 0xc0100000,
                            1 => 0x40100000,
                            _ => 0x100004,
                        };
                        let (id, _, _) = b
                            .context
                            .create(
                                name.as_bytes(),
                                CreateRequest {
                                    access,
                                    share: 7,
                                    ..file_request(3)
                                },
                            )
                            .unwrap();
                        {
                            let mut state = b.context.state.lock().unwrap();
                            let h = state.handles.get_mut(&id).unwrap();
                            h.position = 3;
                            let Object::File(file) = &mut h.object else {
                                panic!("file required")
                            };
                            file.seek(SeekFrom::Start(3)).unwrap();
                        }
                        let mut bytes = [0xab; 8];
                        if write != 0 {
                            bytes[..3].copy_from_slice(b"XYZ");
                        }
                        let result = b.context.transfer(id, &mut bytes[..length], write, offset);
                        let append = write != 0 && (mode == 2 || offset == Some(-1));
                        let denied = write == 0 && mode != 0;
                        let invalid = !append && offset.is_some_and(|n| n < 0 && n != -2);
                        let expected_position = if denied || invalid {
                            expect(
                                result,
                                if denied {
                                    ACCESS_DENIED
                                } else {
                                    INVALID_PARAMETER
                                },
                            );
                            3
                        } else {
                            let target = if append {
                                10
                            } else {
                                offset.filter(|&n| n >= 0).unwrap_or(3) as u64
                            };
                            let count = if write != 0 {
                                length as u32
                            } else {
                                (10u64.saturating_sub(target)).min(length as u64) as u32
                            };
                            let status = if write == 0 && length != 0 && count == 0 {
                                0xc0000011
                            } else {
                                0
                            };
                            assert_eq!(result.unwrap(), [status, count, 1], "case{cases}");
                            if write == 0 && count != 0 {
                                assert_eq!(
                                    &bytes[..count as usize],
                                    &b"0123456789"
                                        [target as usize..target as usize + count as usize]
                                );
                            }
                            target + u64::from(count)
                        };
                        assert_eq!(
                            b.context.state.lock().unwrap().handles[&id].position,
                            expected_position,
                            "case{cases}"
                        );
                        b.context.close(id, 0).unwrap();
                        let mut expected = b"0123456789".to_vec();
                        if !denied && !invalid && write != 0 && length != 0 {
                            let start = if append {
                                10
                            } else {
                                offset.filter(|&n| n >= 0).unwrap_or(3) as usize
                            };
                            expected.resize(expected.len().max(start + length), 0);
                            expected[start..start + length].copy_from_slice(&b"XYZ"[..length]);
                        }
                        assert_eq!(
                            std::fs::read(root.path().join(&name)).unwrap(),
                            expected,
                            "case{cases}"
                        );
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 96);
        assert_eq!(b.open_handles().unwrap(), 0);
        println!("Synchronous native-profile transfers:96 offset/access/append/zero/EOF cases");
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn nt_metadata_times_preserve_epoch_fraction_and_reject_unrepresentable_dates() {
        const EPOCH: u64 = 116_444_736_000_000_000;
        assert_eq!(nt_ticks(0, 0).unwrap(), EPOCH);
        assert_eq!(nt_ticks(1, 123_456_789).unwrap(), EPOCH + 11_234_567);
        assert_eq!(nt_ticks(-1, 999_999_999).unwrap(), EPOCH - 1);
        assert_eq!(nt_ticks(-11_644_473_600, 0).unwrap(), 0);
        assert!(matches!(nt_ticks(-11_644_473_601, 0), Err(Error::Host(-2))));
        assert!(matches!(nt_ticks(i128::MAX, 0), Err(Error::Host(-2))));
        assert!(matches!(nt_ticks(0, 1_000_000_000), Err(Error::Host(-1))));
        for n in [1, 99, 100, 999_999_999] {
            let time = std::time::UNIX_EPOCH - std::time::Duration::from_nanos(n);
            assert_eq!(
                creation_ticks(cap_std::time::SystemTime::from_std(time)).unwrap(),
                EPOCH - n.div_ceil(100)
            );
        }
    }
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    #[test]
    fn network_information_uses_open_file_metadata_and_virtual_allocation_geometry() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        let b = backend(root.path(), true);
        let (id, _, _) = b.context.create(b"Metadata", file_request(3)).unwrap();
        let word64 =
            |out: &[u32; 14], at: usize| u64::from(out[at]) | (u64::from(out[at + 1]) << 32);
        for length in [0, 1, 16383, 16384, 16385, 65537] {
            {
                let mut state = b.context.state.lock().unwrap();
                let Object::File(file) = &mut state.handles.get_mut(&id).unwrap().object else {
                    panic!("file required")
                };
                file.set_len(length).unwrap();
                if length > 0 {
                    file.write_all(b"x").unwrap();
                }
                file.sync_all().unwrap();
            }
            let out = b.context.query(id, 34).unwrap();
            assert_eq!(out[0], 0);
            assert_eq!(out[13] & 0x24, 0x24);
            assert_eq!(word64(&out, 11), length);
            assert_eq!(word64(&out, 9), length.div_ceil(16384) * 16384);
            assert!(word64(&out, 1) > 116_444_736_000_000_000);
            let state = b.context.state.lock().unwrap();
            let Object::File(file) = &state.handles[&id].object else {
                panic!("file required")
            };
            let metadata = file.metadata().unwrap();
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                use cap_std::fs::MetadataExt as _;
                assert_eq!(
                    word64(&out, 1),
                    creation_ticks(metadata.created().unwrap()).unwrap()
                );
                assert_eq!(
                    word64(&out, 3),
                    nt_ticks(i128::from(metadata.atime()), metadata.atime_nsec() as u32).unwrap()
                );
                assert_eq!(
                    word64(&out, 5),
                    nt_ticks(i128::from(metadata.mtime()), metadata.mtime_nsec() as u32).unwrap()
                );
                assert_eq!(
                    word64(&out, 7),
                    nt_ticks(i128::from(metadata.ctime()), metadata.ctime_nsec() as u32).unwrap()
                );
            }
            #[cfg(windows)]
            {
                use cap_std::fs::MetadataExt as _;
                assert_eq!(word64(&out, 1), metadata.creation_time());
                assert_eq!(word64(&out, 3), metadata.last_access_time());
                assert_eq!(word64(&out, 5), metadata.last_write_time());
            }
        }
        let d = unsafe { b.descriptor() };
        let mut out = [0xabababab; 14];
        assert_eq!(
            unsafe {
                d.query.unwrap()(
                    d.context,
                    id as u32,
                    (id >> 32) as u32,
                    34,
                    out.as_mut_ptr(),
                )
            },
            0
        );
        assert_eq!(word64(&out, 11), 65537);
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let state = b.context.state.lock().unwrap();
            let Object::File(file) = &state.handles[&id].object else {
                panic!("file required")
            };
            rustix::fs::fsetxattr(
                file,
                ATTRIBUTE_XATTR,
                &[1, 2, 3],
                rustix::fs::XattrFlags::REPLACE,
            )
            .unwrap();
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        assert!(matches!(b.context.query(id, 34), Err(Error::Host(-2))));
        assert!(matches!(b.context.query(id, 999), Err(Error::Host(-2))));
        b.context.close(id, 0).unwrap();
        expect(b.context.query(id, 34), INVALID_HANDLE);
        let (dir, _) = b
            .context
            .create_directory(b"Directory", create_request(3))
            .unwrap();
        let out = b.context.query(dir, 34).unwrap();
        assert_eq!(out[13] & 0x10, 0x10);
        assert_eq!(word64(&out, 9), 0);
        assert_eq!(word64(&out, 11), 0);
        b.context.close(dir, 0).unwrap();
        assert_eq!(b.open_handles().unwrap(), 0);
    }
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    #[test]
    fn regular_create_persists_system_attribute_and_reopens_without_truncation() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("UDATA/4541005b")).unwrap();
        let name = b"UDATA\\4541005b\\TitleMeta.xbx";
        let b = backend(root.path(), true);
        let (id, kind, info) = b.context.create(name, file_request(3)).unwrap();
        assert_eq!((kind, info), (2, 2));
        {
            let mut state = b.context.state.lock().unwrap();
            let Object::File(file) = &mut state.handles.get_mut(&id).unwrap().object else {
                panic!("file capability required")
            };
            file.write_all(b"preserved metadata").unwrap();
            file.sync_all().unwrap();
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let mut attributes = [0u8; 4];
                assert_eq!(
                    rustix::fs::fgetxattr(&*file, ATTRIBUTE_XATTR, &mut attributes).unwrap(),
                    4
                );
                assert_eq!(u32::from_le_bytes(attributes), 0x24);
                use std::os::fd::AsFd;
                assert!(rustix::fs::fcntl_getfl(file.as_fd())
                    .unwrap()
                    .contains(rustix::fs::OFlags::SYNC));
            }
            #[cfg(windows)]
            {
                use cap_std::fs::MetadataExt as _;
                assert_eq!(file.metadata().unwrap().file_attributes() & 0x24, 0x24);
            }
        }
        expect(b.context.create(name, file_request(3)), SHARING_VIOLATION);
        assert_eq!(b.open_handles().unwrap(), 1);
        expect(b.context.volume(id), ACCESS_DENIED);
        expect(
            b.context.create(
                b"Child",
                CreateRequest {
                    parent: id,
                    ..file_request(3)
                },
            ),
            NOT_DIRECTORY,
        );
        b.context.close(id, 0).unwrap();
        drop(b);
        let b = backend(root.path(), true);
        let (id, kind, info) = b
            .context
            .create(b"udata\\4541005B\\titlemeta.XBX", file_request(3))
            .unwrap();
        assert_eq!((kind, info), (2, 1));
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let state = b.context.state.lock().unwrap();
            let Object::File(file) = &state.handles[&id].object else {
                panic!("file capability required")
            };
            let mut attributes = [0u8; 4];
            assert_eq!(
                rustix::fs::fgetxattr(file, ATTRIBUTE_XATTR, &mut attributes).unwrap(),
                4
            );
            assert_eq!(u32::from_le_bytes(attributes), 0x24);
        }
        b.context.close(id, 1).unwrap();
        assert_eq!(
            std::fs::read(root.path().join("UDATA/4541005b/TitleMeta.xbx")).unwrap(),
            b"preserved metadata"
        );
        assert_eq!(b.open_handles().unwrap(), 0);
    }
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    #[test]
    fn regular_create_ffi_kind_dispositions_and_policy_are_real() {
        let root = tempfile::tempdir().unwrap();
        let b = backend(root.path(), true);
        let d = unsafe { b.descriptor() };
        let request = [0x40100000, 1, 0x22, 1, 3, 4, 0, 0, 0];
        let mut out = [0xabababab; 5];
        assert_eq!(
            unsafe {
                d.create.unwrap()(
                    d.context,
                    0,
                    0,
                    b"Meta.xbx".as_ptr(),
                    8,
                    request.as_ptr(),
                    out.as_mut_ptr(),
                )
            },
            0
        );
        assert_eq!(out[0], 0);
        assert_eq!(&out[3..], &[2, 2]);
        b.context
            .close(u64::from(out[1]) | (u64::from(out[2]) << 32), 0)
            .unwrap();
        expect(
            b.context.create(b"Meta.xbx", file_request(2)),
            NAME_COLLISION,
        );
        expect(
            b.context.create(b"Missing", file_request(1)),
            NAME_NOT_FOUND,
        );
        std::fs::create_dir(root.path().join("Directory")).unwrap();
        assert!(matches!(
            b.context.create(b"Directory", file_request(3)),
            Err(Error::Host(-2))
        ));
        expect(
            b.context.create(
                b"Directory",
                CreateRequest {
                    options: 0x62,
                    ..file_request(3)
                },
            ),
            IS_DIRECTORY,
        );
        assert!(matches!(
            b.context.create(b"", file_request(3)),
            Err(Error::Host(-2))
        ));
        expect(
            b.context.create(
                b"",
                CreateRequest {
                    options: 0x62,
                    ..file_request(3)
                },
            ),
            IS_DIRECTORY,
        );
        for name in [
            b"../Escape".as_slice(),
            b"..\\Escape",
            b"Name:Stream",
            b"Name.",
            b"Name ",
            b"Name\0Rest",
        ] {
            expect(b.context.create(name, file_request(3)), NAME_INVALID);
        }
        for r in [
            CreateRequest {
                allocation: Some(1),
                ..file_request(3)
            },
            CreateRequest {
                attributes: 0x800,
                ..file_request(3)
            },
            file_request(4),
        ] {
            assert!(matches!(
                b.context.create(b"Unsupported", r),
                Err(Error::Host(-2))
            ));
        }
        let ro = backend(root.path(), false);
        expect(
            ro.context.create(b"Missing", file_request(3)),
            ACCESS_DENIED,
        );
        expect(
            ro.context.create(
                b"Missing",
                CreateRequest {
                    access: 0x80100000,
                    ..file_request(3)
                },
            ),
            ACCESS_DENIED,
        );
        let (id, kind, info) = ro
            .context
            .create(
                b"Meta.xbx",
                CreateRequest {
                    access: 0x80100000,
                    ..file_request(3)
                },
            )
            .unwrap();
        assert_eq!((kind, info), (2, 1));
        ro.context.close(id, 0).unwrap();
        assert!(!root.path().join("Unsupported").exists());
        assert!(!root.path().join("Missing").exists());
        let low = Backend::new(
            Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap(),
            Policy {
                writable: true,
                bytes_per_sector: 512,
                sectors_per_allocation_unit: 32,
            },
            Budget::new(4096),
        )
        .unwrap();
        expect(low.context.create(b"NoBudget", file_request(3)), NO_MEMORY);
        assert!(!root.path().join("NoBudget").exists());
        assert_eq!(b.open_handles().unwrap(), 0);
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn regular_files_reject_links_special_files_and_publication_collisions() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("Keep"), b"outside").unwrap();
        symlink(outside.path(), root.path().join("LinkDir")).unwrap();
        symlink(outside.path().join("Keep"), root.path().join("LinkFile")).unwrap();
        std::fs::hard_link(outside.path().join("Keep"), root.path().join("HardLink")).unwrap();
        rustix::fs::mknodat(
            rustix::fs::CWD,
            root.path().join("Pipe"),
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        let b = backend(root.path(), true);
        for name in [
            b"LinkDir\\Escape".as_slice(),
            b"LinkFile",
            b"HardLink",
            b"Pipe",
        ] {
            expect(b.context.create(name, file_request(3)), ACCESS_DENIED);
        }
        assert_eq!(b.open_handles().unwrap(), 0);
        assert_eq!(
            std::fs::read(outside.path().join("Keep")).unwrap(),
            b"outside"
        );
        assert!(!outside.path().join("Escape").exists());
        // Simulate a name created after the initial enumeration. The unnamed
        // temporary file must never replace it or leak another visible entry.
        std::fs::write(root.path().join("Raced"), b"keep").unwrap();
        expect(
            regular_file(&b.context.root, None, "Raced", file_request(2), 0x120116),
            NAME_COLLISION,
        );
        assert_eq!(std::fs::read(root.path().join("Raced")).unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 5);
    }
    #[cfg(windows)]
    #[test]
    fn directory_creation_matches_native_windows_create_dispositions() {
        use std::os::windows::io::AsRawHandle;
        #[repr(C)]
        struct Unicode {
            length: u16,
            capacity: u16,
            buffer: *mut u16,
        }
        #[repr(C)]
        struct Attributes {
            length: u32,
            root: *mut c_void,
            name: *mut Unicode,
            flags: u32,
            security: *mut c_void,
            qos: *mut c_void,
        }
        #[repr(C)]
        struct IoStatus {
            status: usize,
            information: usize,
        }
        #[link(name = "ntdll")]
        unsafe extern "system" {
            fn NtCreateFile(
                handle: *mut *mut c_void,
                access: u32,
                attributes: *mut Attributes,
                status: *mut IoStatus,
                allocation: *const i64,
                file_attributes: u32,
                share: u32,
                disposition: u32,
                options: u32,
                ea: *const c_void,
                ea_bytes: u32,
            ) -> i32;
            fn NtClose(handle: *mut c_void) -> i32;
        }
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Native")).unwrap();
        std::fs::create_dir(root.path().join("Virtual")).unwrap();
        let native =
            Dir::open_ambient_dir(root.path().join("Native"), cap_std::ambient_authority())
                .unwrap();
        let b = backend(&root.path().join("Virtual"), true);
        let mut cases = 0;
        for existing in [false, true] {
            for disposition in 1..=3 {
                for zero_allocation in [false, true] {
                    for file_attributes in [0, 0x80] {
                        let name = format!("Case{cases}");
                        if existing {
                            native.create_dir(&name).unwrap();
                            std::fs::create_dir(root.path().join("Virtual").join(&name)).unwrap();
                        }
                        let mut text: Vec<u16> =
                            name.encode_utf16().chain(std::iter::once(0)).collect();
                        let mut unicode = Unicode {
                            length: ((text.len() - 1) * 2) as u16,
                            capacity: (text.len() * 2) as u16,
                            buffer: text.as_mut_ptr(),
                        };
                        let mut attributes = Attributes {
                            length: std::mem::size_of::<Attributes>() as u32,
                            root: native.as_raw_handle(),
                            name: &mut unicode,
                            flags: 0x40,
                            security: std::ptr::null_mut(),
                            qos: std::ptr::null_mut(),
                        };
                        let mut status = IoStatus {
                            status: 0x11223344,
                            information: 0x55667788,
                        };
                        let mut handle = std::ptr::null_mut();
                        let zero = 0i64;
                        let native_status = unsafe {
                            NtCreateFile(
                                &mut handle,
                                0x100001,
                                &mut attributes,
                                &mut status,
                                if zero_allocation {
                                    &zero
                                } else {
                                    std::ptr::null()
                                },
                                file_attributes,
                                3,
                                disposition,
                                0x4021,
                                std::ptr::null(),
                                0,
                            )
                        };
                        let result = b.context.create_directory(
                            name.as_bytes(),
                            CreateRequest {
                                attributes: file_attributes,
                                allocation: zero_allocation.then_some(0),
                                ..create_request(disposition)
                            },
                        );
                        match result {
                            Ok((id, information)) => {
                                assert_eq!(native_status, 0, "case{cases}");
                                assert_eq!(status.status, 0);
                                assert_eq!(information as usize, status.information, "case{cases}");
                                assert!(!handle.is_null());
                                b.context.close(id, 0).unwrap();
                                assert_eq!(unsafe { NtClose(handle) }, 0);
                            }
                            Err(Error::Guest(code)) => {
                                assert!(native_status < 0, "case{cases}");
                                assert_eq!(native_status as u32, code, "case{cases}");
                                assert!(handle.is_null());
                            }
                            Err(Error::Host(code)) => {
                                panic!("unexpected host result{code} case{cases}")
                            }
                        }
                        assert_eq!(
                            native.symlink_metadata(&name).is_ok(),
                            root.path().join("Virtual").join(&name).exists()
                        );
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(b.open_handles().unwrap(), 0);
        println!("Native Windows directory creation oracle:{cases} disposition/allocation/attribute cases matched");
    }
    #[test]
    fn create_directory_ffi_reports_real_creation_and_open_information() {
        let root = tempfile::tempdir().unwrap();
        let b = backend(root.path(), true);
        let d = unsafe { b.descriptor() };
        let request = [0x100001, 3, 0x4021, 1, 3, 0x80, 0, 0, 0];
        let mut out = [0; 5];
        assert_eq!(
            unsafe {
                d.create.unwrap()(
                    d.context,
                    0,
                    0,
                    b"TDATA".as_ptr(),
                    5,
                    request.as_ptr(),
                    out.as_mut_ptr(),
                )
            },
            0
        );
        assert_eq!(out[0], 0);
        assert_eq!(out[3..], [1, 2]);
        assert!(root.path().join("TDATA").is_dir());
        let first = u64::from(out[1]) | (u64::from(out[2]) << 32);
        let (second, info) = b
            .context
            .create_directory(b"tdata", create_request(3))
            .unwrap();
        assert_eq!(info, 1);
        {
            let state = b.context.state.lock().unwrap();
            assert_eq!(
                state.handles[&first].identity,
                state.handles[&second].identity
            );
        }
        expect(
            b.context.create_directory(b"TDATA", create_request(2)),
            NAME_COLLISION,
        );
        expect(
            b.context.create_directory(b"Missing", create_request(1)),
            NAME_NOT_FOUND,
        );
        assert!(!root.path().join("Missing").exists());
        let child = b
            .context
            .create_directory(
                b"Title",
                CreateRequest {
                    parent: first,
                    ..create_request(2)
                },
            )
            .unwrap();
        assert_eq!(child.1, 2);
        assert!(root.path().join("TDATA/Title").is_dir());
        for id in [first, second, child.0] {
            b.context.close(id, 0).unwrap();
        }
        assert_eq!(b.open_handles().unwrap(), 0);
    }
    #[test]
    fn directory_creation_respects_policy_budget_paths_and_sharing() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Present")).unwrap();
        let ro = backend(root.path(), false);
        let (id, info) = ro
            .context
            .create_directory(b"Present", create_request(3))
            .unwrap();
        assert_eq!(info, 1);
        ro.context.close(id, 0).unwrap();
        expect(
            ro.context.create_directory(b"Missing", create_request(3)),
            ACCESS_DENIED,
        );
        assert!(!root.path().join("Missing").exists());
        let b = backend(root.path(), true);
        for name in [
            b"..".as_slice(),
            b"A\\..",
            b"A/B",
            b"A:B",
            b"A\0B",
            b"A.",
            b"A ",
        ] {
            expect(
                b.context.create_directory(name, create_request(3)),
                NAME_INVALID,
            );
        }
        expect(
            b.context
                .create_directory(b"Absent\\Child", create_request(3)),
            PATH_NOT_FOUND,
        );
        for disposition in [0, 4, 5, 6] {
            expect(
                b.context
                    .create_directory(b"New", create_request(disposition)),
                INVALID_PARAMETER,
            );
        }
        assert!(matches!(
            b.context.create_directory(
                b"New",
                CreateRequest {
                    allocation: Some(1),
                    ..create_request(3)
                }
            ),
            Err(Error::Host(-2))
        ));
        assert!(matches!(
            b.context.create_directory(
                b"New",
                CreateRequest {
                    attributes: 2,
                    ..create_request(3)
                }
            ),
            Err(Error::Host(-2))
        ));
        assert!(!root.path().join("New").exists());
        let (id, _) = b
            .context
            .create_directory(
                b"Present",
                CreateRequest {
                    share: 0,
                    ..create_request(1)
                },
            )
            .unwrap();
        expect(
            b.context.create_directory(b"Present", create_request(3)),
            SHARING_VIOLATION,
        );
        b.context.close(id, 0).unwrap();
        let budget = Budget::new(4096);
        let low = Backend::new(
            Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap(),
            Policy {
                writable: true,
                bytes_per_sector: 512,
                sectors_per_allocation_unit: 32,
            },
            budget,
        )
        .unwrap();
        expect(
            low.context.create_directory(b"NoBudget", create_request(3)),
            NO_MEMORY,
        );
        assert!(!root.path().join("NoBudget").exists());
    }
    #[test]
    fn simultaneous_casefold_open_if_creates_one_directory() {
        let root = tempfile::tempdir().unwrap();
        let b = backend(root.path(), true);
        std::thread::scope(|scope| {
            let mut workers = Vec::new();
            for i in 0..8 {
                let context = &b.context;
                workers.push(scope.spawn(move || {
                    context
                        .create_directory(
                            if i % 2 == 0 { b"SAVE" } else { b"save" },
                            create_request(3),
                        )
                        .unwrap()
                }));
            }
            let values: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
            assert_eq!(values.iter().filter(|v| v.1 == 2).count(), 1);
            assert_eq!(values.iter().filter(|v| v.1 == 1).count(), 7);
            for (id, _) in values {
                b.context.close(id, 0).unwrap();
            }
        });
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        assert_eq!(b.open_handles().unwrap(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn creation_pins_renamed_roots_and_rejects_swapped_symlink_parents() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let moved = temp.path().join("moved");
        std::fs::create_dir(&root).unwrap();
        let b = backend(&root, true);
        std::fs::rename(&root, &moved).unwrap();
        std::fs::create_dir(&root).unwrap();
        let (id, info) = b
            .context
            .create_directory(b"Pinned", create_request(3))
            .unwrap();
        assert_eq!(info, 2);
        b.context.close(id, 0).unwrap();
        assert!(moved.join("Pinned").is_dir());
        assert!(!root.join("Pinned").exists());
        let race = temp.path().join("race");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(race.join("candidate")).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let b = backend(&race, true);
        let parent = b.context.root.open_dir("candidate").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let outside_check = outside.clone();
        let attacker = std::thread::spawn(move || {
            while !signal.load(Ordering::Acquire) {
                std::fs::rename(race.join("candidate"), race.join("held")).unwrap();
                std::os::unix::fs::symlink(&outside, race.join("candidate")).unwrap();
                std::fs::remove_file(race.join("candidate")).unwrap();
                std::fs::rename(race.join("held"), race.join("candidate")).unwrap();
                std::thread::yield_now();
            }
        });
        let mut created = 0;
        for i in 0..1000 {
            let name = format!("Save{i}");
            let path = format!("candidate\\{name}");
            if let Ok((id, info)) = b
                .context
                .create_directory(path.as_bytes(), create_request(3))
            {
                assert_eq!(info, 2);
                let expected = parent.open_dir(&name).unwrap().dir_metadata().unwrap();
                assert_eq!(
                    b.context.state.lock().unwrap().handles[&id].identity,
                    Identity(expected.dev(), expected.ino())
                );
                b.context.close(id, 0).unwrap();
                created += 1;
            }
        }
        stop.store(true, Ordering::Release);
        attacker.join().unwrap();
        assert_eq!(std::fs::read_dir(outside_check).unwrap().count(), 0);
        assert_eq!(b.open_handles().unwrap(), 0);
        println!(
            "Pinned-parent directory-create symlink race:1000 attempts,{created} safe creates"
        );
    }
    #[cfg(unix)]
    #[test]
    fn create_directory_never_follows_symlink_parents_or_leaves() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), root.path().join("Link")).unwrap();
        let b = backend(root.path(), true);
        expect(
            b.context
                .create_directory(b"Link\\Escape", create_request(3)),
            ACCESS_DENIED,
        );
        expect(
            b.context.create_directory(b"Link", create_request(3)),
            ACCESS_DENIED,
        );
        expect(
            b.context.create_directory(b"Link", create_request(2)),
            NAME_COLLISION,
        );
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
        assert_eq!(b.open_handles().unwrap(), 0);
    }
    #[test]
    fn actual_directory_capabilities_and_volume_queries_are_live() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Cache")).unwrap();
        let b = backend(root.path(), true);
        let d = unsafe { b.descriptor() };
        assert_eq!(
            std::mem::size_of::<Descriptor>(),
            9 * std::mem::size_of::<usize>()
        );
        let mut out = [0u32; 4];
        assert_eq!(
            unsafe {
                d.open.unwrap()(
                    d.context,
                    0,
                    0,
                    std::ptr::null(),
                    0,
                    0x100001,
                    3,
                    0x800021,
                    1,
                    out.as_mut_ptr(),
                )
            },
            0
        );
        assert_eq!(out[0], 0);
        assert_eq!(out[3], 1);
        let id = u64::from(out[1]) | (u64::from(out[2]) << 32);
        assert_ne!(id, 0);
        assert_eq!(b.open_handles().unwrap(), 1);
        let mut volume = [0u32; 7];
        assert_eq!(
            unsafe { d.volume.unwrap()(d.context, out[1], out[2], volume.as_mut_ptr()) },
            0
        );
        assert_eq!(volume[0], 0);
        assert_eq!(&volume[5..], &[512, 32]);
        let total = u64::from(volume[1]) | (u64::from(volume[2]) << 32);
        let available = u64::from(volume[3]) | (u64::from(volume[4]) << 32);
        assert!(total > 0 && available <= total);
        let child = b.context.open(id, b"cache", 0x100001, 7, 0x21, 1).unwrap();
        assert_eq!(b.open_handles().unwrap(), 2);
        assert_eq!(
            b.context.state.lock().unwrap().handles[&child].identity,
            Identity(
                b.context
                    .root
                    .open_dir("Cache")
                    .unwrap()
                    .dir_metadata()
                    .unwrap()
                    .dev(),
                b.context
                    .root
                    .open_dir("Cache")
                    .unwrap()
                    .dir_metadata()
                    .unwrap()
                    .ino()
            )
        );
        b.context.close(child, 0).unwrap();
        let mut status = u32::MAX;
        assert_eq!(
            unsafe { d.close.unwrap()(d.context, out[1], out[2], 0, &mut status) },
            0
        );
        assert_eq!(status, 0);
        assert_eq!(b.open_handles().unwrap(), 0);
        expect(b.context.close(id, 0), INVALID_HANDLE);
        b.context.state.lock().unwrap().next = 0x12345678_abcdef01;
        let mut high = [0; 4];
        assert_eq!(
            unsafe {
                d.open.unwrap()(
                    d.context,
                    0,
                    0,
                    std::ptr::null(),
                    0,
                    0x100001,
                    7,
                    0x21,
                    1,
                    high.as_mut_ptr(),
                )
            },
            0
        );
        assert_eq!(&high[1..3], &[0xabcdef01, 0x12345678]);
        assert_eq!(
            unsafe { d.close.unwrap()(d.context, high[1], high[2], 1, &mut status) },
            0
        );
        assert_eq!(b.open_handles().unwrap(), 0);
    }
    #[test]
    fn native_token_namespace_wraps_and_skips_live_tokens() {
        let root = tempfile::tempdir().unwrap();
        let b = backend(root.path(), true);
        let first = b.context.open(0, b"", 0x100001, 7, 0x21, 1).unwrap();
        assert_eq!(first, 1);
        b.context.state.lock().unwrap().next = u64::MAX;
        let last = b.context.open(0, b"", 0x100001, 7, 0x21, 1).unwrap();
        assert_eq!(last, u64::MAX);
        let wrapped = b.context.open(0, b"", 0x100001, 7, 0x21, 1).unwrap();
        assert_eq!(wrapped, 2);
        for id in [first, last, wrapped] {
            b.context.close(id, 0).unwrap();
        }
        assert_eq!(b.open_handles().unwrap(), 0);
    }
    #[test]
    fn malformed_and_unimplemented_paths_do_not_open_objects() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Cache")).unwrap();
        std::fs::write(root.path().join("file.bin"), b"private data").unwrap();
        let b = backend(root.path(), true);
        for name in [
            b"..".as_slice(),
            b".",
            b"Cache\\..",
            b"Cache\\.\\nested",
            b"\\Cache",
            b"Cache\\\\nested",
            b"Cache/../other",
            b"C:\\Windows",
            b"file:stream",
            b"GLOBALROOT/Device",
            b"Cache\0tail",
            b"Cache.",
            b"Cache ",
            b"*",
            b"?",
            b"a\x7f",
            b"a\nb",
        ] {
            expect(b.context.open(0, name, 0x100001, 7, 0x21, 1), NAME_INVALID);
            assert_eq!(b.open_handles().unwrap(), 0);
        }
        expect(
            b.context.open(0, b"missing", 0x100001, 7, 0x21, 1),
            NAME_NOT_FOUND,
        );
        expect(
            b.context.open(0, b"missing\\child", 0x100001, 7, 0x21, 1),
            PATH_NOT_FOUND,
        );
        expect(
            b.context.open(0, b"file.bin", 0x100001, 7, 0x21, 1),
            NOT_DIRECTORY,
        );
        assert!(matches!(
            b.context.open(0, b"file.bin", 0x100001, 7, 0x20, 1),
            Err(Error::Host(-2))
        ));
        assert!(matches!(
            b.context.open(0, b"caf\xe9", 0x100001, 7, 0x21, 1),
            Err(Error::Host(-2))
        ));
        expect(
            b.context.open(0, b"Cache", 0x100001, 7, 0x61, 1),
            INVALID_PARAMETER,
        );
        expect(
            b.context.open(0, b"Cache", 0x100001, 7, 0x60, 1),
            IS_DIRECTORY,
        );
        expect(
            b.context.open(99, b"", 0x100001, 7, 0x21, 1),
            INVALID_HANDLE,
        );
        expect(
            b.context.open(0, b"Cache", 1, 7, 0x21, 1),
            INVALID_PARAMETER,
        );
        expect(
            b.context.open(0, b"Cache", 0x100001, 8, 0x21, 1),
            INVALID_PARAMETER,
        );
        assert_eq!(b.open_handles().unwrap(), 0);
        let d = unsafe { b.descriptor() };
        let mut out = [0x55; 4];
        assert_eq!(
            unsafe {
                d.open.unwrap()(
                    d.context,
                    0,
                    0,
                    std::ptr::null(),
                    1,
                    0x100001,
                    7,
                    0x21,
                    1,
                    out.as_mut_ptr(),
                )
            },
            -1
        );
        assert_eq!(out, [0x55; 4]);
    }
    #[test]
    fn sharing_rights_metadata_only_and_close_are_enforced() {
        let root = tempfile::tempdir().unwrap();
        let b = backend(root.path(), true);
        let read = b.context.open(0, b"", 0x100001, 1, 0x21, 1).unwrap();
        expect(
            b.context.open(0, b"", 0x100002, 3, 0x21, 1),
            SHARING_VIOLATION,
        );
        let another = b.context.open(0, b"", 0x100001, 3, 0x21, 1).unwrap();
        expect(
            b.context.open(0, b"", 0x100001, 0, 0x21, 1),
            SHARING_VIOLATION,
        );
        b.context.close(another, 0).unwrap();
        b.context.close(read, 0).unwrap();
        let metadata = b.context.open(0, b"", 0x100080, 0, 0x21, 1).unwrap();
        let reader = b.context.open(0, b"", 0x100001, 0, 0x21, 1).unwrap();
        b.context.close(metadata, 0).unwrap();
        b.context.close(reader, 0).unwrap();
        let deletion = b.context.open(0, b"", 0x110000, 7, 0x21, 1).unwrap();
        expect(
            b.context.open(0, b"", 0x100001, 3, 0x21, 1),
            SHARING_VIOLATION,
        );
        let shared = b.context.open(0, b"", 0x100001, 7, 0x21, 1).unwrap();
        b.context.close(shared, 0).unwrap();
        b.context.close(deletion, 1).unwrap();
        let readonly = backend(root.path(), false);
        expect(
            readonly.context.open(0, b"", 0x100002, 7, 0x21, 1),
            ACCESS_DENIED,
        );
        expect(
            readonly.context.open(0, b"", 0x110000, 7, 0x21, 1),
            ACCESS_DENIED,
        );
        let id = readonly
            .context
            .open(0, b"", 0x80000000, 7, 0x21, 1)
            .unwrap();
        readonly.context.close(id, 0).unwrap();
        let low = Backend::new(
            Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap(),
            Policy {
                writable: true,
                bytes_per_sector: 512,
                sectors_per_allocation_unit: 32,
            },
            Budget::new(4096),
        )
        .unwrap();
        expect(low.context.open(0, b"", 0x100001, 7, 0x21, 1), NO_MEMORY);
        assert_eq!(low.open_handles().unwrap(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_case_collisions_and_root_renames_do_not_escape() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(root.join("Cache")).unwrap();
        std::fs::create_dir_all(outside.join("Secret")).unwrap();
        let b = backend(&root, true);
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
        expect(
            b.context.open(0, b"escape\\Secret", 0x100001, 7, 0x21, 1),
            ACCESS_DENIED,
        );
        std::fs::create_dir(root.join("cache")).unwrap();
        expect(
            b.context.open(0, b"CACHE", 0x100001, 7, 0x21, 1),
            NAME_COLLISION,
        );
        std::fs::remove_dir(root.join("cache")).unwrap();
        std::fs::rename(&root, temp.path().join("moved")).unwrap();
        std::fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("Cache")).unwrap();
        let id = b.context.open(0, b"Cache", 0x100001, 7, 0x21, 1).unwrap();
        let expected = b
            .context
            .root
            .open_dir("Cache")
            .unwrap()
            .dir_metadata()
            .unwrap();
        assert_eq!(
            b.context.state.lock().unwrap().handles[&id].identity,
            Identity(expected.dev(), expected.ino())
        );
        b.context.close(id, 0).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn swapped_symlink_components_never_open_outside_directory() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(root.join("candidate")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let b = backend(&root, true);
        let expected = b
            .context
            .root
            .open_dir("candidate")
            .unwrap()
            .dir_metadata()
            .unwrap();
        let identity = Identity(expected.dev(), expected.ino());
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let attacker = std::thread::spawn(move || {
            while !signal.load(Ordering::Acquire) {
                std::fs::rename(root.join("candidate"), root.join("held")).unwrap();
                std::os::unix::fs::symlink(&outside, root.join("candidate")).unwrap();
                std::fs::remove_file(root.join("candidate")).unwrap();
                std::fs::rename(root.join("held"), root.join("candidate")).unwrap();
            }
        });
        let mut opened = 0;
        for _ in 0..1000 {
            if let Ok(id) = b.context.open(0, b"candidate", 0x100001, 7, 0x21, 1) {
                assert_eq!(
                    b.context.state.lock().unwrap().handles[&id].identity,
                    identity
                );
                b.context.close(id, 0).unwrap();
                opened += 1;
            }
        }
        stop.store(true, Ordering::Release);
        attacker.join().unwrap();
        assert_eq!(b.open_handles().unwrap(), 0);
        println!("Pinned-directory symlink-swap race:1000 attempts,{opened} safe opens");
    }
    #[cfg(windows)]
    #[test]
    fn directory_shares_match_native_windows_open_semantics() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Virtual")).unwrap();
        std::fs::create_dir(root.path().join("Native")).unwrap();
        let b = backend(root.path(), true);
        let mask = |class: u32| {
            0x100000
                | if class & 1 != 0 { 1 } else { 0 }
                | if class & 2 != 0 { 2 } else { 0 }
                | if class & 4 != 0 { 0x10000 } else { 0 }
        };
        let native = |access, share| {
            let mut o = std::fs::OpenOptions::new();
            o.access_mode(access)
                .share_mode(share)
                .custom_flags(0x02000000);
            o.open(root.path().join("Native"))
        };
        let mut cases = 0;
        for ac in 0..8 {
            for ash in 0..8 {
                for bc in 0..8 {
                    for bsh in 0..8 {
                        let a = native(mask(ac), ash).unwrap();
                        let ours = b
                            .context
                            .open(0, b"Virtual", mask(ac), ash, 0x21, 1)
                            .unwrap();
                        let reference = native(mask(bc), bsh);
                        let got = b.context.open(0, b"Virtual", mask(bc), bsh, 0x21, 1);
                        assert_eq!(got.is_ok(),reference.is_ok(),"share case {ac}/{ash} then {bc}/{bsh}; native={reference:?}; backend={got:?}");
                        if let Ok(id) = got {
                            b.context.close(id, 0).unwrap();
                        } else {
                            expect(got, SHARING_VIOLATION);
                            assert_eq!(reference.as_ref().unwrap_err().raw_os_error(), Some(32));
                        }
                        drop(reference);
                        b.context.close(ours, 0).unwrap();
                        drop(a);
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 4096);
        assert_eq!(b.open_handles().unwrap(), 0);
        println!("Native Windows directory share oracle:{cases} cases matched");
    }
}
