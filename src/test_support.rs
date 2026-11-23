//! Test-only helpers for large virtual fixtures.
//!
//! NTFS does not make `File::set_len` sparse on its own. Marking the fixture
//! sparse keeps multi-gigabyte offset tests from consuming the builder's disk.
#[cfg(windows)]
pub(crate) fn mark_sparse(file: &std::fs::File) {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};
    #[link(name = "kernel32")]
    extern "system" {
        fn DeviceIoControl(
            handle: *mut c_void,
            code: u32,
            input: *const c_void,
            input_len: u32,
            output: *mut c_void,
            output_len: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
    }
    let mut returned = 0;
    assert_ne!(
        unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                0x0009_00c4,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        },
        0,
        "Windows fixtures require a sparse-capable filesystem"
    );
}

#[cfg(not(windows))]
pub(crate) fn mark_sparse(_: &std::fs::File) {}
