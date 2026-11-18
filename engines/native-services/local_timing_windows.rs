//! Trusted high-resolution waiting. No handle or callback crosses the SDK.
use super::super::{Error, Result};
use std::{ffi::c_void, ptr::NonNull};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> *mut c_void;
    fn CreateWaitableTimerExW(
        attributes: *const c_void,
        name: *const u16,
        flags: u32,
        access: u32,
    ) -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn SetEvent(handle: *mut c_void) -> i32;
    fn ResetEvent(handle: *mut c_void) -> i32;
    fn SetWaitableTimer(
        timer: *mut c_void,
        due: *const i64,
        period: i32,
        completion: *const c_void,
        argument: *const c_void,
        resume: i32,
    ) -> i32;
    fn CancelWaitableTimer(timer: *mut c_void) -> i32;
    fn WaitForMultipleObjects(
        count: u32,
        objects: *const *mut c_void,
        all: i32,
        milliseconds: u32,
    ) -> u32;
    fn GetLastError() -> u32;
}
struct Handle(NonNull<c_void>);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: this unique owner closes a handle created by the kernel.
        unsafe {
            CloseHandle(self.0.as_ptr());
        }
    }
}
pub(super) struct Native {
    event: Handle,
    timer: Handle,
}
// Kernel wait/event APIs support concurrent signal/wait. The clock lock
// serializes resets/arms; one waiter owns an Arc for the entire native wait.
unsafe impl Send for Native {}
unsafe impl Sync for Native {}
impl Native {
    pub(super) fn new() -> Result<Self> {
        // SAFETY: unnamed private event, no security descriptor or game pointer.
        let event = NonNull::new(unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) })
            .ok_or(Error::Failed)?;
        let event = Handle(event);
        // SAFETY: private auto-reset high-resolution timer; request only modify
        // and synchronization rights. No APC, name, inherited handle or fallback.
        let timer = NonNull::new(unsafe {
            CreateWaitableTimerExW(std::ptr::null(), std::ptr::null(), 2, 0x00100002)
        });
        let Some(timer) = timer else {
            // SAFETY: GetLastError has no arguments and follows the failed call.
            return Err(match unsafe { GetLastError() } {
                1 | 50 | 87 => Error::Unsupported,
                5 => Error::Denied,
                8 | 14 | 1450 => Error::Busy,
                _ => Error::Failed,
            });
        };
        Ok(Self {
            event,
            timer: Handle(timer),
        })
    }
    pub(super) fn reset(&self) -> Result<()> {
        // SAFETY: Arc keeps the private event live; caller holds the clock lock.
        if unsafe { ResetEvent(self.event.0.as_ptr()) } == 0 {
            Err(Error::Failed)
        } else {
            Ok(())
        }
    }
    pub(super) fn signal(&self) -> Result<()> {
        // SAFETY: Arc keeps the event live; SetEvent is safe against native wait.
        if unsafe { SetEvent(self.event.0.as_ptr()) } == 0 {
            Err(Error::Failed)
        } else {
            Ok(())
        }
    }
    pub(super) fn wait(&self, nanoseconds: u64) -> Result<()> {
        if !(1..=1_000_000_000).contains(&nanoseconds) {
            return Err(Error::Invalid);
        }
        let due = -(nanoseconds.div_ceil(100) as i64);
        // SAFETY: valid timer and exclusive live due-time pointer; no callback.
        if unsafe {
            SetWaitableTimer(
                self.timer.0.as_ptr(),
                &due,
                0,
                std::ptr::null(),
                std::ptr::null(),
                0,
            )
        } == 0
        {
            return Err(Error::Failed);
        }
        let handles = [self.event.0.as_ptr(), self.timer.0.as_ptr()];
        // SAFETY: both handles stay alive through this Arc-owned wait. A bounded
        // guard detects an OS timer failure rather than silently using Sleep.
        let reason = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, 1100) };
        // SAFETY: no other waiter arms this timer; cancel even on wait failure.
        let cancelled = unsafe { CancelWaitableTimer(self.timer.0.as_ptr()) };
        if reason > 1 || cancelled == 0 {
            Err(Error::Failed)
        } else {
            Ok(())
        }
    }
}
