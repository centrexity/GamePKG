//! Host-only Windows write-through rename of validated leaves under pinned
//! directory capabilities. Mirrors cap-primitives' Windows handle-to-path
//! rename approach; requires exclusive private save roots, as does the caller.
use std::{
    ffi::{c_void, OsString},
    fs::{File, OpenOptions},
    io,
    os::windows::{ffi::OsStringExt, fs::OpenOptionsExt, io::AsRawHandle},
};
#[derive(Default)]
#[repr(C)]
struct Information {
    attributes: u32,
    creation: [u32; 2],
    access: [u32; 2],
    write: [u32; 2],
    volume: u32,
    size_high: u32,
    size_low: u32,
    links: u32,
    index_high: u32,
    index_low: u32,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetFinalPathNameByHandleW(file: *mut c_void, path: *mut u16, length: u32, flags: u32)
        -> u32;
    fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
    fn GetFileInformationByHandle(file: *mut c_void, information: *mut Information) -> i32;
}
fn identity(file: &File) -> io::Result<(u32, u32, u32)> {
    let mut information = Information::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if information.attributes & 0x400 != 0 || information.attributes & 0x10 == 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    if information.index_high == 0 && information.index_low == 0 {
        return Err(io::ErrorKind::Unsupported.into());
    }
    Ok((
        information.volume,
        information.index_high,
        information.index_low,
    ))
}
fn leaf(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.len() > 255
        || matches!(name, "." | "..")
        || name.contains(['\\', '/', ':', '<', '>', '"', '|', '?', '*'])
        || name.chars().any(char::is_control)
        || name.ends_with([' ', '.'])
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let stem = name
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
        .is_some_and(|v| {
            matches!(
                v,
                "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(())
}
fn beneath(directory: &File, name: &str) -> io::Result<(Vec<u16>, Vec<File>)> {
    leaf(name)?;
    if !directory.metadata()?.is_dir() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // Keep the caller's parent handle open throughout rename. No game-chosen
    // absolute path or current-directory resolution enters this function.
    let mut path = vec![0u16; 512];
    for _ in 0..3 {
        let length = unsafe {
            GetFinalPathNameByHandleW(
                directory.as_raw_handle(),
                path.as_mut_ptr(),
                path.len() as u32,
                0,
            )
        } as usize;
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if length >= 32767 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if length >= path.len() {
            path.resize(length + 1, 0);
            continue;
        }
        path.truncate(length);
        // Restrict the durable adapter to local drive namespaces. No volume
        // copying, UNC fallback or ambiguous device-relative path is accepted.
        let prefix: Vec<u16> = "\\\\?\\".encode_utf16().collect();
        if !path.starts_with(&prefix)
            || path.len() < 7
            || !((path[4] as u8).is_ascii_alphabetic())
            || path[4] > 127
            || path[5] != b':' as u16
            || path[6] != b'\\' as u16
        {
            return Err(io::ErrorKind::Unsupported.into());
        }
        let mut ends = vec![7];
        ends.extend((7..path.len()).filter(|i| path[*i] == b'\\' as u16));
        if path.len() > 7 && path.last() != Some(&(b'\\' as u16)) {
            ends.push(path.len());
        }
        if ends.len() > 256 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut anchors = Vec::new();
        // Open each actual parent without FILE_SHARE_DELETE. On Windows this
        // prevents its name being replaced/renamed during the path-based move.
        // Root-down locking plus final file-ID comparison also rejects stale
        // GetFinalPathName results and reparse-point substitutions.
        for end in ends {
            let anchor = OpenOptions::new()
                .access_mode(0x00100080)
                .share_mode(3)
                .custom_flags(0x02000000 | 0x00200000)
                .open(OsString::from_wide(&path[..end]))?;
            identity(&anchor)?;
            anchors.push(anchor);
        }
        if identity(anchors.last().ok_or(io::ErrorKind::InvalidInput)?)? != identity(directory)? {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        if path.last() != Some(&(b'\\' as u16)) {
            path.push(b'\\' as u16);
        }
        path.extend(name.encode_utf16());
        if path.len() >= 32767 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        path.push(0);
        return Ok((path, anchors));
    }
    Err(io::ErrorKind::Interrupted.into())
}
/// Same-volume atomic replacement only: COPY_ALLOWED and deferred operations
/// are deliberately absent. The caller must flush/validate the source first.
pub(super) fn replace(
    from: &File,
    source: &str,
    to: &File,
    destination: &str,
    write_through: bool,
) -> io::Result<()> {
    let (source, _source_anchors) = beneath(from, source)?;
    let (destination, _destination_anchors) = beneath(to, destination)?;
    let flags = 1 | if write_through { 8 } else { 0 };
    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "gamepkg-save-replace-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn open(&self) -> File {
            OpenOptions::new()
                .read(true)
                .share_mode(7)
                .custom_flags(0x02000000)
                .open(&self.0)
                .unwrap()
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn windows_replace_flush_retry_and_unicode() {
        let root = Root::new();
        let directory = root.open();
        for durable in [false, true] {
            fs::write(root.0.join("temporary ü"), b"complete save").unwrap();
            fs::write(root.0.join("save"), b"old save").unwrap();
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(root.0.join("temporary ü"))
                .unwrap()
                .sync_all()
                .unwrap();
            let reader = OpenOptions::new()
                .read(true)
                .share_mode(3)
                .open(root.0.join("save"))
                .unwrap();
            assert!(replace(&directory, "temporary ü", &directory, "save", durable).is_err());
            assert_eq!(fs::read(root.0.join("save")).unwrap(), b"old save");
            assert_eq!(
                fs::read(root.0.join("temporary ü")).unwrap(),
                b"complete save"
            );
            drop(reader);
            replace(&directory, "temporary ü", &directory, "save", durable).unwrap();
            assert_eq!(fs::read(root.0.join("save")).unwrap(), b"complete save");
            assert!(!root.0.join("temporary ü").exists());
        }
    }

    #[test]
    fn windows_replace_rejects_ambiguous_leaves_without_mutation() {
        let root = Root::new();
        let directory = root.open();
        fs::write(root.0.join("temporary"), b"keep source").unwrap();
        fs::write(root.0.join("save"), b"keep save").unwrap();
        for name in [
            "../outside",
            "/outside",
            "nested/name",
            "a\\b",
            "C:save",
            "save:stream",
            "NUL",
            "COM1.txt",
            "COM¹.txt",
            "save.",
            "save ",
            "",
            "..",
            "a\0b",
        ] {
            assert!(replace(&directory, "temporary", &directory, name, true).is_err());
            assert!(replace(&directory, name, &directory, "save", true).is_err());
        }
        assert_eq!(fs::read(root.0.join("temporary")).unwrap(), b"keep source");
        assert_eq!(fs::read(root.0.join("save")).unwrap(), b"keep save");
    }

    #[test]
    fn windows_replace_moved_parent_never_uses_recreated_old_path() {
        let root = Root::new();
        let directory = root.open();
        fs::write(root.0.join("temporary"), b"private source").unwrap();
        fs::write(root.0.join("save"), b"private save").unwrap();
        let moved = Root(root.0.with_extension("moved"));
        if let Err(error) = fs::rename(&root.0, &moved.0) {
            // Some Windows filesystems deny moving an open directory outright.
            // This is a safe refusal, but does not exercise stale-name handling.
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            eprintln!(
                "moved-parent subcase unavailable: filesystem denies moving an open directory"
            );
            return;
        }
        fs::create_dir(&root.0).unwrap();
        fs::write(root.0.join("temporary"), b"outside source").unwrap();
        fs::write(root.0.join("save"), b"outside save").unwrap();
        let result = replace(&directory, "temporary", &directory, "save", true);
        assert_eq!(
            fs::read(root.0.join("temporary")).unwrap(),
            b"outside source"
        );
        assert_eq!(fs::read(root.0.join("save")).unwrap(), b"outside save");
        if result.is_ok() {
            assert_eq!(fs::read(moved.0.join("save")).unwrap(), b"private source");
        } else {
            assert_eq!(
                fs::read(moved.0.join("temporary")).unwrap(),
                b"private source"
            );
            assert_eq!(fs::read(moved.0.join("save")).unwrap(), b"private save");
        }
        drop(directory);
    }
}
