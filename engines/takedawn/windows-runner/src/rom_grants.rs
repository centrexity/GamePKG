//! Ordered read-only files passed only between the launcher and trusted bootstrap.
use super::{duplicate, duplicate_with_inheritance, invalid, FILE_GENERIC_READ};
use std::{
    fs::File,
    io,
    os::windows::io::{AsRawHandle, OwnedHandle},
};
use windows_sys::Win32::{
    Foundation::{
        GetHandleInformation, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT,
        HANDLE_FLAG_PROTECT_FROM_CLOSE,
    },
    Storage::FileSystem::{GetFileType, FILE_ATTRIBUTE_DIRECTORY, FILE_TYPE_DISK},
};

pub(super) const ENV: &str = "GAMEPKG_NATIVE_ROM_HANDLES";
const MAX_ROMS: usize = 8;

pub(super) fn prepare(files: &[File]) -> io::Result<Vec<OwnedHandle>> {
    if files.len() > MAX_ROMS {
        return Err(invalid("too many selected ROMs"));
    }
    files
        .iter()
        .map(|file| {
            if !file.metadata()?.is_file() {
                return Err(invalid("selected ROM must be a regular file"));
            }
            duplicate(file.as_raw_handle(), FILE_GENERIC_READ)
        })
        .collect()
}

/// Open private, non-inheritable read-only copies in signed manifest order.
/// Call from the trusted bootstrap after the process containment check. Treat
/// incoming numeric handles as borrowed even if startup did not come from
/// the launcher. Only handles returned by DuplicateHandle become Rust-owned.
pub fn open_inherited_roms(expected: usize) -> io::Result<Vec<File>> {
    let encoded = std::env::var(ENV).or_else(|error| match error {
        std::env::VarError::NotPresent => Ok(String::new()),
        _ => Err(invalid("invalid inherited ROM handle list")),
    })?;
    let handles = parse_handles(&encoded, expected)?;
    let stdio = [
        std::io::stdin().as_raw_handle(),
        std::io::stdout().as_raw_handle(),
        std::io::stderr().as_raw_handle(),
    ];
    for &value in &handles {
        let handle = value as HANDLE;
        let mut flags = 0;
        if stdio.contains(&handle)
            || unsafe { GetHandleInformation(handle, &mut flags) } == 0
            || flags & HANDLE_FLAG_INHERIT == 0
            || flags & HANDLE_FLAG_PROTECT_FROM_CLOSE != 0
            || unsafe { GetFileType(handle) } != FILE_TYPE_DISK
        {
            return Err(invalid("invalid inherited ROM handle"));
        }
    }
    let mut roms = Vec::with_capacity(handles.len());
    for value in handles {
        let inherited = value as HANDLE;
        let file = File::from(duplicate_with_inheritance(
            inherited,
            FILE_GENERIC_READ,
            false,
        )?);
        if unsafe { SetHandleInformation(inherited, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        use std::os::windows::fs::MetadataExt;
        let metadata = file.metadata()?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
            return Err(invalid("inherited ROM must be a regular file"));
        }
        roms.push(file);
    }
    Ok(roms)
}

fn parse_handles(encoded: &str, expected: usize) -> io::Result<Vec<usize>> {
    // 20 decimal digits and one separator accommodate each 64-bit HANDLE.
    if expected > MAX_ROMS || encoded.len() > MAX_ROMS * 21 {
        return Err(invalid("invalid inherited ROM handle list"));
    }
    if encoded.is_empty() && expected == 0 {
        return Ok(Vec::new());
    }
    let mut handles = Vec::with_capacity(expected);
    for part in encoded.split(',') {
        let value = if part.bytes().all(|byte| byte.is_ascii_digit()) {
            part.parse::<usize>().ok()
        } else {
            None
        }
        .filter(|value| *value != 0 && *value <= isize::MAX as usize)
        .ok_or_else(|| invalid("invalid inherited ROM handle"))?;
        if handles.len() >= expected || handles.contains(&value) {
            return Err(invalid("unexpected or repeated inherited ROM handle"));
        }
        handles.push(value);
    }
    if handles.len() != expected {
        return Err(invalid("inherited ROM count differs from signed manifest"));
    }
    Ok(handles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_list_requires_manifest_order_and_exact_count() {
        assert_eq!(parse_handles("", 0).unwrap(), []);
        assert_eq!(parse_handles("31,16,51", 3).unwrap(), [31, 16, 51]);
        assert_eq!(
            parse_handles("1,2,3,4,5,6,7,8", 8).unwrap(),
            [1, 2, 3, 4, 5, 6, 7, 8]
        );
        for (text, count) in [
            ("", 1),
            ("11", 0),
            ("11", 2),
            ("11,11", 2),
            ("0", 1),
            ("-1", 1),
            (" 32", 1),
            ("11,", 2),
            ("18446744073709551615", 1),
            ("1,2,3,4,5,6,7,8,9", 9),
        ] {
            assert!(parse_handles(text, count).is_err(), "{text:?}/{count}");
        }
    }

    #[test]
    fn caller_cannot_override_the_rom_handoff_in_the_environment() {
        let values = [("GamePkg_Native_Rom_Handles".into(), "42".into())];
        assert!(crate::environment_block(&values, None, None, &[]).is_err());
    }
}
