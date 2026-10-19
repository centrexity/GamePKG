//! Publish a verified staging directory without replacing any destination.
//! Never fall back to a check followed by an overwriting rename.
use std::{io, path::Path};

#[cfg(target_os = "linux")]
pub(super) fn publish(staging: &Path, target: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(staging.as_os_str().as_bytes())?;
    let target = std::ffi::CString::new(target.as_os_str().as_bytes())?;
    // Both terminated paths remain alive; flags request atomic no-replace.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
pub(super) fn publish(staging: &Path, target: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(staging.as_os_str().as_bytes())?;
    let target = std::ffi::CString::new(target.as_os_str().as_bytes())?;
    // Darwin's exclusive rename rejects existing files, links and directories.
    // A filesystem that cannot provide this guarantee returns an error.
    let result = unsafe { libc::renamex_np(source.as_ptr(), target.as_ptr(), libc::RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
pub(super) fn publish(staging: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(source: *const u16, target: *const u16, flags: u32) -> i32;
    }
    fn wide(path: &Path) -> io::Result<Vec<u16>> {
        if path.as_os_str().encode_wide().any(|unit| unit == 0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains a NUL",
            ));
        }
        let path = std::path::absolute(path)?;
        let mut value = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if !value.starts_with(&[92, 92, 63, 92]) {
            let (prefix, skip) = if value.starts_with(&[92, 92]) {
                (r"\\?\UNC\", 2)
            } else {
                (r"\\?\", 0)
            };
            let mut extended = prefix.encode_utf16().collect::<Vec<_>>();
            extended.extend_from_slice(&value[skip..]);
            value = extended;
        }
        value.push(0);
        Ok(value)
    }
    // Normalize absolute Win32 paths without following the destination leaf,
    // then use extended disk/UNC paths to retain long-path support.
    let source = wide(staging)?;
    let target = wide(target)?;
    // Both UTF-16 buffers remain alive through the synchronous call. Zero
    // flags exclude replace/copy/reboot behavior and leave existing targets alone.
    let result = unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), 0) };
    if result != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(super) fn publish(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "exclusive app publication is unavailable on this platform",
    ))
}
