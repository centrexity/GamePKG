//! Host-owned C interface to the same verified reader used by GamePKG.
//! No alternate ISO parser, guest dependency or unverified asset read path.
#![allow(dead_code)] // Shared reader also exposes the CLI inspector API.
#![deny(unsafe_op_in_unsafe_fn)]
#[path = "../../../../src/memory_budget.rs"]
mod memory_budget;
#[path = "../../../../src/xbox_disc.rs"]
mod xbox_disc;

pub use memory_budget::{Budget, Lease};
use std::{
    collections::BTreeMap,
    io::{Read, Seek, SeekFrom},
    sync::Mutex,
};
pub use xbox_disc::{DirectoryEntry, Stream, VerifiedDisc};

// Stable POSIX errno values, not the host's native OS error representation.
const EIO: i32 = -5;
const ENOENT: i32 = -2;
const EBADF: i32 = -9;
const ENOMEM: i32 = -12;
const EBUSY: i32 = -16;
const ENODEV: i32 = -19;
const ENOTDIR: i32 = -20;
const EISDIR: i32 = -21;
const EINVAL: i32 = -22;
const EMFILE: i32 = -24;
const ENOBUFS: i32 = -105;
const MAX_HANDLES: usize = 128;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Info {
    size: u64,
    is_directory: u32,
    reserved: u32,
}
fn entry_info(entry: &DirectoryEntry) -> Info {
    Info {
        size: entry.size,
        is_directory: u32::from(entry.is_directory),
        reserved: 0,
    }
}
enum Handle {
    File(Stream),
    Directory {
        path: String,
        cursor: usize,
        _memory: Lease,
    },
}
struct Bridge {
    media: VerifiedDisc,
    handles: BTreeMap<u64, Handle>,
    budget: Budget,
    _table: Lease,
}
#[derive(Default)]
struct State {
    bridge: Option<Bridge>,
    next: u64,
}
static STATE: Mutex<State> = Mutex::new(State {
    bridge: None,
    next: 0,
});
type Result<T> = std::result::Result<T, i32>;

fn call(f: impl FnOnce(&mut State) -> Result<()>) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut state = STATE.lock().map_err(|_| EIO)?;
        f(&mut state)
    }))
    .unwrap_or(Err(EIO))
    .err()
    .unwrap_or(0)
}
fn bridge(state: &mut State) -> Result<&mut Bridge> {
    state.bridge.as_mut().ok_or(ENODEV)
}
// SAFETY: All exported functions use the live caller-buffer contract in media.h.
unsafe fn text<'a>(p: *const u8, len: usize, max: usize) -> Result<&'a str> {
    if len > max || (len != 0 && p.is_null()) {
        return Err(EINVAL);
    }
    let data = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(p, len) }
    };
    let value = std::str::from_utf8(data).map_err(|_| EINVAL)?;
    if value.contains('\0') {
        return Err(EINVAL);
    }
    Ok(value)
}
fn path(value: &str) -> Result<String> {
    if value.len() >= 512 || value.chars().any(|c| c.is_control() || c == ':') {
        return Err(EINVAL);
    }
    let value = value.replace('\\', "/");
    if !value.is_empty() && value.split('/').any(|part| matches!(part, "" | "." | "..")) {
        return Err(EINVAL);
    }
    Ok(value)
}
impl Bridge {
    fn info(&self, path: &str) -> Result<Info> {
        if self.media.directory(path).is_ok() {
            return Ok(Info {
                is_directory: 1,
                ..Info::default()
            });
        }
        let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
        let entries = self.media.directory(parent).map_err(|_| ENOENT)?;
        let entry = entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
            .ok_or(ENOENT)?;
        Ok(entry_info(entry))
    }
    fn open(&mut self, path: String) -> Result<Handle> {
        if self.handles.len() >= MAX_HANDLES {
            return Err(EMFILE);
        }
        let info = self.info(&path)?;
        if info.is_directory != 0 {
            Ok(Handle::Directory {
                path,
                cursor: 0,
                _memory: self.budget.reserve(4096).map_err(|_| ENOMEM)?,
            })
        } else {
            Ok(Handle::File(
                self.media.open_stream(&path).map_err(|_| ENOMEM)?,
            ))
        }
    }
}

/// # Safety
/// Caller buffers must meet the contract in media.h.
#[no_mangle]
pub unsafe extern "C" fn gp_media_init(iso: *const u8, length: usize, budget_bytes: u64) -> i32 {
    call(|state| {
        if state.bridge.is_some() {
            return Err(EBUSY);
        }
        let iso = unsafe { text(iso, length, 32768)? };
        if iso.is_empty() {
            return Err(EINVAL);
        }
        let budget = Budget::new(budget_bytes);
        let table = budget.reserve(65536).map_err(|_| ENOMEM)?;
        let media = VerifiedDisc::open(std::path::Path::new(iso), &budget).map_err(|_| EIO)?;
        state.bridge = Some(Bridge {
            media,
            handles: BTreeMap::new(),
            budget,
            _table: table,
        });
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn gp_media_shutdown() -> i32 {
    call(|state| {
        if !bridge(state)?.handles.is_empty() {
            return Err(EBUSY);
        }
        state.bridge = None;
        Ok(())
    })
}
/// # Safety
/// Caller buffers must meet the contract in media.h.
#[no_mangle]
pub unsafe extern "C" fn gp_media_open(p: *const u8, len: usize, output: *mut u64) -> i32 {
    call(|state| {
        if output.is_null() {
            return Err(EINVAL);
        }
        let path = path(unsafe { text(p, len, 511)? })?;
        let id = state.next.checked_add(1).ok_or(EMFILE)?;
        let bridge = bridge(state)?;
        let handle = bridge.open(path)?;
        bridge.handles.insert(id, handle);
        state.next = id;
        unsafe {
            output.write(id);
        }
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn gp_media_close(handle: u64) -> i32 {
    call(|state| {
        bridge(state)?.handles.remove(&handle).ok_or(EBADF)?;
        Ok(())
    })
}
/// # Safety
/// Caller buffers must meet the contract in media.h.
#[no_mangle]
pub unsafe extern "C" fn gp_media_stat(p: *const u8, len: usize, output: *mut Info) -> i32 {
    call(|state| {
        if output.is_null() {
            return Err(EINVAL);
        }
        let path = path(unsafe { text(p, len, 511)? })?;
        let info = bridge(state)?.info(&path)?;
        unsafe {
            output.write(info);
        }
        Ok(())
    })
}
/// # Safety
/// Caller buffers must meet the contract in media.h.
#[no_mangle]
pub unsafe extern "C" fn gp_media_fstat(handle: u64, output: *mut Info) -> i32 {
    call(|state| {
        if output.is_null() {
            return Err(EINVAL);
        }
        let info = match bridge(state)?.handles.get(&handle).ok_or(EBADF)? {
            Handle::File(stream) => Info {
                size: stream.len(),
                ..Info::default()
            },
            Handle::Directory { .. } => Info {
                is_directory: 1,
                ..Info::default()
            },
        };
        unsafe {
            output.write(info);
        }
        Ok(())
    })
}
/// # Safety
/// Caller buffers must meet the contract in media.h. Output and count cannot overlap.
#[no_mangle]
pub unsafe extern "C" fn gp_media_read(
    handle: u64,
    output: *mut u8,
    capacity: usize,
    count: *mut usize,
) -> i32 {
    call(|state| {
        if count.is_null() || (capacity != 0 && output.is_null()) || capacity > isize::MAX as usize
        {
            return Err(EINVAL);
        }
        let Handle::File(stream) = bridge(state)?.handles.get_mut(&handle).ok_or(EBADF)? else {
            return Err(EISDIR);
        };
        let size = capacity.min(65536);
        let buffer = if size == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(output, size) }
        };
        let read = stream.read(buffer).map_err(|_| EIO)?;
        unsafe {
            count.write(read);
        }
        Ok(())
    })
}
/// # Safety
/// Caller buffers must meet the contract in media.h.
#[no_mangle]
pub unsafe extern "C" fn gp_media_seek(
    handle: u64,
    offset: i64,
    whence: i32,
    output: *mut u64,
) -> i32 {
    call(|state| {
        if output.is_null() {
            return Err(EINVAL);
        }
        let seek = match whence {
            0 if offset >= 0 => SeekFrom::Start(offset as u64),
            1 => SeekFrom::Current(offset),
            2 => SeekFrom::End(offset),
            _ => return Err(EINVAL),
        };
        let Handle::File(stream) = bridge(state)?.handles.get_mut(&handle).ok_or(EBADF)? else {
            return Err(EISDIR);
        };
        let position = stream.seek(seek).map_err(|_| EINVAL)?;
        unsafe {
            output.write(position);
        }
        Ok(())
    })
}
/// # Safety
/// Caller buffers must meet the contract in media.h. Output buffers cannot overlap.
#[no_mangle]
pub unsafe extern "C" fn gp_media_readdir(
    handle: u64,
    name: *mut u8,
    capacity: usize,
    length: *mut usize,
    info: *mut Info,
) -> i32 {
    call(|state| {
        if length.is_null() || info.is_null() || (capacity != 0 && name.is_null()) {
            return Err(EINVAL);
        }
        let bridge = bridge(state)?;
        let Handle::Directory { path, cursor, .. } =
            bridge.handles.get_mut(&handle).ok_or(EBADF)?
        else {
            return Err(ENOTDIR);
        };
        let entries = bridge.media.directory(path).map_err(|_| EIO)?;
        let Some(entry) = entries.get(*cursor) else {
            unsafe {
                length.write(0);
                info.write(Info::default());
            }
            return Ok(());
        };
        if entry.name.len() > capacity {
            return Err(ENOBUFS);
        }
        unsafe {
            std::ptr::copy_nonoverlapping(entry.name.as_ptr(), name, entry.name.len());
            info.write(entry_info(entry));
            length.write(entry.name.len());
        }
        *cursor += 1;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unscoped_paths_and_invalid_calls_without_touching_outputs() {
        for bad in [
            "/default.xbe",
            "../default.xbe",
            "a/./b",
            "a//b",
            "D:\\a",
            "a\\..\\b",
            "a\n",
        ] {
            assert!(path(bad).is_err(), "{bad:?}");
        }
        assert_eq!(path("Data\\Global.txd").unwrap(), "Data/Global.txd");
        let mut handle = 99;
        unsafe {
            assert_eq!(gp_media_open(std::ptr::null(), 1, &mut handle), EINVAL);
            assert_eq!(
                gp_media_open(b"default.xbe".as_ptr(), 11, &mut handle),
                ENODEV
            );
            assert_eq!(
                gp_media_read(1, std::ptr::null_mut(), 1, std::ptr::null_mut()),
                EINVAL
            );
        }
        assert_eq!(handle, 99);
        assert_eq!(gp_media_close(1), ENODEV);
        assert_eq!(gp_media_shutdown(), ENODEV);
    }
}
