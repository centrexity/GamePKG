//! Host-owned handle admission. A duplicate keeps only read access and never
//! inherits into another child. The supplied original remains the caller's.
use std::{
    ffi::c_void,
    fs::File,
    os::windows::io::{FromRawHandle, OwnedHandle},
};

const EBADF: i32 = -9;
const FILE_GENERIC_READ: u32 = 0x0012_0089;
const FILE_TYPE_DISK: u32 = 1;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn DuplicateHandle(
        source_process: *mut c_void,
        source: *mut c_void,
        target_process: *mut c_void,
        target: *mut *mut c_void,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
    fn GetFileType(handle: *mut c_void) -> u32;
}

pub fn duplicate_read_file(raw: usize) -> Result<File, i32> {
    if raw == 0 || raw == usize::MAX {
        return Err(EBADF);
    }
    let mut copy = std::ptr::null_mut();
    // SAFETY: Win32 validates the source handle. Only a successful duplicate
    // becomes owned here; no close-source or same-access option is supplied.
    let result = unsafe {
        let process = GetCurrentProcess();
        DuplicateHandle(
            process,
            raw as *mut c_void,
            process,
            &mut copy,
            FILE_GENERIC_READ,
            0,
            0,
        )
    };
    if result == 0 {
        return Err(EBADF);
    }
    // SAFETY: DuplicateHandle returned this unique, valid owned handle.
    let owned = unsafe { OwnedHandle::from_raw_handle(copy) };
    // SAFETY: The owned duplicate stays live during this type query.
    if unsafe { GetFileType(copy) } != FILE_TYPE_DISK {
        return Err(EBADF);
    }
    Ok(File::from(owned))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Seek, SeekFrom, Write},
        os::windows::io::AsRawHandle,
    };

    #[test]
    fn duplicate_is_read_only_noninheritable_and_does_not_own_original() {
        unsafe extern "system" {
            fn GetHandleInformation(handle: *mut c_void, flags: *mut u32) -> i32;
        }
        let mut original = tempfile::tempfile().unwrap();
        original.write_all(b"original image").unwrap();
        let mut duplicate = duplicate_read_file(original.as_raw_handle() as usize).unwrap();
        let mut flags = 0;
        // SAFETY: This is a live owned duplicate and a writable output flag.
        assert_ne!(
            unsafe { GetHandleInformation(duplicate.as_raw_handle(), &mut flags) },
            0
        );
        assert_eq!(flags & 1, 0, "HANDLE_FLAG_INHERIT must be absent");
        duplicate.seek(SeekFrom::Start(0)).unwrap();
        let mut data = String::new();
        duplicate.read_to_string(&mut data).unwrap();
        assert_eq!(data, "original image");
        assert_eq!(
            duplicate.write(b"bad").unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        drop(duplicate);
        original.seek(SeekFrom::Start(0)).unwrap();
        original.write_all(b"caller owns it").unwrap();
        assert!(duplicate_read_file(original.as_raw_handle() as usize).is_ok());
    }

    #[test]
    fn invalid_and_nondisk_handles_reject_without_consuming_caller() {
        unsafe extern "system" {
            fn CreateEventW(
                security: *mut c_void,
                manual: i32,
                initial: i32,
                name: *const u16,
            ) -> *mut c_void;
            fn GetHandleInformation(handle: *mut c_void, flags: *mut u32) -> i32;
        }
        assert!(duplicate_read_file(0).is_err());
        assert!(duplicate_read_file(usize::MAX).is_err());
        // SAFETY: Null optional pointers and ordinary boolean parameters.
        let event = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
        assert!(!event.is_null());
        // SAFETY: This fresh successful event is uniquely owned.
        let event = unsafe { OwnedHandle::from_raw_handle(event) };
        assert!(duplicate_read_file(event.as_raw_handle() as usize).is_err());
        let mut flags = 0;
        // SAFETY: The caller's original event must remain valid after rejection.
        assert_ne!(
            unsafe { GetHandleInformation(event.as_raw_handle(), &mut flags) },
            0
        );
    }
}
