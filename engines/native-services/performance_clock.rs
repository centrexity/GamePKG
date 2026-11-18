//! Audited nonblocking performance reads shared by the broker and trusted child.
//! Never staged as game source. No calendar, device handle or pointer is exposed.
use super::{Error, Result};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Raw {
    pub counter: u64,
    pub frequency: u64,
    pub uptime_ms: u64,
}
#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    any(
        all(target_arch = "x86", target_pointer_width = "32"),
        all(target_arch = "x86_64", target_pointer_width = "64")
    )
))]
pub(crate) fn sample() -> Result<Raw> {
    // Linux clock_gettime's timespec uses C long on both qualified GNU targets.
    // Keep this trusted source dependency-free for the separately linked runtime.
    #[repr(C)]
    struct Timespec {
        seconds: std::ffi::c_long,
        nanos: std::ffi::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: i32, output: *mut Timespec) -> i32;
    }
    let mut time = Timespec {
        seconds: 0,
        nanos: 0,
    };
    // SAFETY: CLOCK_MONOTONIC, with a valid writable native timespec.
    if unsafe { clock_gettime(1, &mut time) } != 0
        || time.seconds < 0
        || !(0..1_000_000_000).contains(&time.nanos)
    {
        return Err(Error::Failed);
    }
    let counter = (time.seconds as u64)
        .checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(time.nanos as u64))
        .ok_or(Error::Failed)?;
    if counter > i64::MAX as u64 {
        return Err(Error::Failed);
    }
    Ok(Raw {
        counter,
        frequency: 1_000_000_000,
        uptime_ms: counter / 1_000_000,
    })
}
#[cfg(windows)]
pub(crate) fn sample() -> Result<Raw> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn QueryPerformanceCounter(value: *mut i64) -> i32;
        fn QueryPerformanceFrequency(value: *mut i64) -> i32;
        fn GetTickCount64() -> u64;
    }
    let mut counter = 0i64;
    let mut frequency = 0i64;
    // SAFETY: both output pointers reference live, exclusively borrowed i64s.
    if unsafe { QueryPerformanceFrequency(&mut frequency) } == 0
        || unsafe { QueryPerformanceCounter(&mut counter) } == 0
        || counter < 0
        || frequency <= 0
    {
        return Err(Error::Failed);
    }
    Ok(Raw {
        counter: counter as u64,
        frequency: frequency as u64,
        // SAFETY: this read-only kernel function has no pointer arguments.
        uptime_ms: unsafe { GetTickCount64() },
    })
}
#[cfg(not(any(
    all(
        target_os = "linux",
        target_env = "gnu",
        any(
            all(target_arch = "x86", target_pointer_width = "32"),
            all(target_arch = "x86_64", target_pointer_width = "64")
        )
    ),
    windows
)))]
pub(crate) fn sample() -> Result<Raw> {
    Err(Error::Unsupported)
}
