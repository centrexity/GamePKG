extern "C" {
    fn gamepkg_native_stdin() -> *mut FILE;
    fn gamepkg_native_stdout() -> *mut FILE;
    fn gamepkg_native_stderr() -> *mut FILE;
    fn gamepkg_native_fflush(__stream: *mut FILE) -> ::core::ffi::c_int;
    fn gamepkg_native_fprintf(
        __stream: *mut FILE,
        __format: *const ::core::ffi::c_char,
        ...
    ) -> ::core::ffi::c_int;
}
pub type __off_t = ::core::ffi::c_long;
pub type __off64_t = ::core::ffi::c_long;
pub type int64_t = i64;
pub type uint32_t = u32;
pub type uint64_t = u64;
pub type uintptr_t = usize;
pub type size_t = usize;
#[repr(C)]
pub struct _IO_FILE {
    _private: [u8; 0],
}
pub type _IO_lock_t = ();
pub type FILE = _IO_FILE;
pub const INT64_MIN: i64 = i64::MIN;
#[no_mangle]
pub unsafe extern "C" fn port_stdin() -> uintptr_t {
    return gamepkg_native_stdin().expose_provenance() as uintptr_t;
}
#[no_mangle]
pub unsafe extern "C" fn port_stdout() -> uintptr_t {
    return gamepkg_native_stdout().expose_provenance() as uintptr_t;
}
#[no_mangle]
pub unsafe extern "C" fn port_stderr() -> uintptr_t {
    return gamepkg_native_stderr().expose_provenance() as uintptr_t;
}
#[no_mangle]
pub unsafe extern "C" fn port_int64_min() -> int64_t {
    return INT64_MIN as int64_t;
}
#[no_mangle]
pub unsafe extern "C" fn port_log(
    mut tag: uint32_t,
    mut count: uint64_t,
) -> ::core::ffi::c_int {
    let mut n: ::core::ffi::c_int = gamepkg_native_fprintf(
        gamepkg_native_stderr(),
        b"native-stream:%08x:%llu\n\0".as_ptr() as *const ::core::ffi::c_char,
        tag,
        count as ::core::ffi::c_ulonglong,
    );
    return if gamepkg_native_fflush(gamepkg_native_stderr()) != 0 {
        -1 as ::core::ffi::c_int
    } else {
        n
    };
}
#[repr(C)]
pub struct _IO_wide_data {
    _private: [u8; 0],
}
#[repr(C)]
pub struct _IO_codecvt {
    _private: [u8; 0],
}
#[repr(C)]
pub struct _IO_marker {
    _private: [u8; 0],
}
