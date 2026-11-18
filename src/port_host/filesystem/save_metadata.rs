// Open-file metadata for private saves. No host paths cross the service.
#[cfg(any(target_os = "linux", target_os = "android", windows))]
use super::io;
use super::{Error, Result};
use std::fs::File;

pub(super) const SETTABLE: u32 = 0x26;
#[cfg(any(target_os = "linux", target_os = "android", windows))]
const READ_ONLY: u32 = 1;
#[cfg(any(target_os = "linux", target_os = "android", windows))]
const NORMAL: u32 = 0x80;

#[cfg(any(target_os = "linux", target_os = "android", windows))]
fn reported_attributes(bits: u32) -> u32 {
    let portable = bits & (SETTABLE | READ_ONLY);
    if portable == 0 {
        NORMAL
    } else {
        portable
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const ATTRIBUTE_NAME: &std::ffi::CStr = c"user.gamepkg.fatx.attributes.v1";

#[cfg(any(target_os = "linux", target_os = "android"))]
fn time_ticks(seconds: i128, nanos: u32) -> Result<u64> {
    if nanos >= 1_000_000_000 {
        return Err(Error::Failed);
    }
    // Floor to 100 ns only after recombining the canonical signed time.
    let ticks = (seconds + 11_644_473_600) * 10_000_000 + i128::from(nanos / 100);
    i64::try_from(ticks)
        .ok()
        .filter(|value| *value >= 0)
        .map(|value| value as u64)
        .ok_or(Error::Unsupported)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn creation_ticks(created: std::time::SystemTime) -> Result<u64> {
    match created.duration_since(std::time::UNIX_EPOCH) {
        Ok(time) => time_ticks(i128::from(time.as_secs()), time.subsec_nanos()),
        Err(error) => {
            let time = error.duration();
            if time.subsec_nanos() == 0 {
                time_ticks(-i128::from(time.as_secs()), 0)
            } else {
                time_ticks(
                    -i128::from(time.as_secs()) - 1,
                    1_000_000_000 - time.subsec_nanos(),
                )
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn unix_attributes(file: &File, readonly: bool) -> Result<u32> {
    use std::os::fd::AsRawFd;

    let mut encoded = [0; 4];
    // This fixed-size read addresses the owned descriptor, not a host name.
    let length = unsafe {
        libc::fgetxattr(
            file.as_raw_fd(),
            ATTRIBUTE_NAME.as_ptr(),
            encoded.as_mut_ptr().cast(),
            encoded.len(),
        )
    };
    let bits = match length {
        4 => {
            let stored = u32::from_le_bytes(encoded);
            if stored & !(SETTABLE | READ_ONLY) != 0 && stored != NORMAL {
                return Err(Error::Failed);
            }
            stored
        }
        -1 => match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ENODATA) => 0x20,
            Some(libc::EOPNOTSUPP) => return Err(Error::Unsupported),
            _ => return Err(Error::Failed),
        },
        _ => return Err(Error::Failed),
    };
    Ok(reported_attributes(bits | u32::from(readonly)))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) fn query(file: &File) -> Result<(u64, [u64; 4], u32)> {
    use std::os::unix::fs::MetadataExt;

    let metadata = io(file.metadata())?;
    let canonical_nanos = |value: i64| u32::try_from(value).map_err(|_| Error::Failed);
    let times = [
        creation_ticks(metadata.created().map_err(|_| Error::Unsupported)?)?,
        time_ticks(
            i128::from(metadata.atime()),
            canonical_nanos(metadata.atime_nsec())?,
        )?,
        time_ticks(
            i128::from(metadata.mtime()),
            canonical_nanos(metadata.mtime_nsec())?,
        )?,
        time_ticks(
            i128::from(metadata.ctime()),
            canonical_nanos(metadata.ctime_nsec())?,
        )?,
    ];
    Ok((
        metadata.len(),
        times,
        unix_attributes(file, metadata.permissions().readonly())?,
    ))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) fn set_attributes(file: &File, bits: u32) -> Result<()> {
    use std::os::fd::AsRawFd;

    let encoded = bits.to_le_bytes();
    let result = unsafe {
        libc::fsetxattr(
            file.as_raw_fd(),
            ATTRIBUTE_NAME.as_ptr(),
            encoded.as_ptr().cast(),
            encoded.len(),
            0,
        )
    };
    if result == 0 {
        return Ok(());
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EOPNOTSUPP) => Err(Error::Unsupported),
        _ => Err(Error::Failed),
    }
}

#[cfg(windows)]
fn windows_basic(file: &File) -> Result<windows_sys::Win32::Storage::FileSystem::FILE_BASIC_INFO> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileBasicInfo, GetFileInformationByHandleEx, FILE_BASIC_INFO,
    };

    let mut info = FILE_BASIC_INFO::default();
    // The exact Win32 layout stays inside the host. The wire uses LE words.
    let result = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileBasicInfo,
            std::ptr::from_mut(&mut info).cast(),
            std::mem::size_of::<FILE_BASIC_INFO>() as u32,
        )
    };
    if result == 0 {
        return Err(Error::Failed);
    }
    Ok(info)
}

#[cfg(windows)]
pub(super) fn query(file: &File) -> Result<(u64, [u64; 4], u32)> {
    let info = windows_basic(file)?;
    let signed = [
        info.CreationTime,
        info.LastAccessTime,
        info.LastWriteTime,
        info.ChangeTime,
    ];
    if signed.iter().any(|&time| time < 0) {
        return Err(Error::Failed);
    }
    Ok((
        io(file.metadata())?.len(),
        signed.map(|time| time as u64),
        reported_attributes(info.FileAttributes),
    ))
}

#[cfg(windows)]
pub(super) fn set_attributes(file: &File, bits: u32) -> Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileBasicInfo, SetFileInformationByHandle, FILE_BASIC_INFO,
    };

    let replacement = FILE_BASIC_INFO {
        FileAttributes: reported_attributes(
            bits | (windows_basic(file)?.FileAttributes & READ_ONLY),
        ),
        ..Default::default()
    };
    let result = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileBasicInfo,
            std::ptr::from_ref(&replacement).cast(),
            std::mem::size_of::<FILE_BASIC_INFO>() as u32,
        )
    };
    if result == 0 {
        return Err(Error::Failed);
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
pub(super) fn query(_: &File) -> Result<(u64, [u64; 4], u32)> {
    Err(Error::Unsupported)
}

#[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
pub(super) fn set_attributes(_: &File, _: u32) -> Result<()> {
    Err(Error::Unsupported)
}
