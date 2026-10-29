//! Private inherited worker channels with cumulative deadlines. Ordinary
//! anonymous Windows pipes cannot cancel a blocked write, so the parent uses
//! overlapped named pipes; the child's endpoint remains ordinary stdio.
use super::{invalid, wide};
use std::{
    ffi::OsStr,
    fs::File,
    io,
    mem::zeroed,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        GetLastError, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE,
        INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG},
    Storage::FileSystem::{
        CreateFileW, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
        OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
    },
    System::{
        Pipes::{ConnectNamedPipe, CreateNamedPipeW, PIPE_REJECT_REMOTE_CLIENTS},
        Threading::{CreateEventW, WaitForSingleObject, INFINITE},
        IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
    },
};

struct Operation {
    pipe: HANDLE,
    state: Box<OVERLAPPED>,
    _event: OwnedHandle,
    pending: bool,
}
impl Operation {
    fn new(pipe: HANDLE) -> io::Result<Self> {
        // Private noninheritable manual-reset event, initially clear.
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        let event = unsafe { OwnedHandle::from_raw_handle(event) };
        let mut state: Box<OVERLAPPED> = Box::new(unsafe { zeroed() });
        state.hEvent = event.as_raw_handle();
        Ok(Self {
            pipe,
            state,
            _event: event,
            pending: false,
        })
    }
    fn complete(&mut self, error: Option<u32>, deadline: Option<Instant>) -> io::Result<usize> {
        if let Some(error) = error {
            if error != ERROR_IO_PENDING {
                return Err(io::Error::from_raw_os_error(error as i32));
            }
            self.pending = true;
            loop {
                let millis = match deadline {
                    None => INFINITE,
                    Some(end) => {
                        let left = end.saturating_duration_since(Instant::now());
                        if left.is_zero() {
                            return Err(timed_out());
                        }
                        left.as_millis().clamp(1, (u32::MAX - 1) as u128) as u32
                    }
                };
                match unsafe { WaitForSingleObject(self.state.hEvent, millis) } {
                    WAIT_OBJECT_0 => break,
                    WAIT_TIMEOUT => continue,
                    _ => return Err(io::Error::last_os_error()),
                }
            }
        }
        let mut transferred = 0;
        let ok = unsafe { GetOverlappedResult(self.pipe, &*self.state, &mut transferred, 0) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        self.pending = false;
        if deadline.is_some_and(|end| Instant::now() >= end) {
            return Err(timed_out());
        }
        Ok(transferred as usize)
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        if self.pending {
            // CancelIoEx only requests cancellation. Keep BOTH the OVERLAPPED
            // and caller's buffer alive until completion, even on timeout/error.
            // https://learn.microsoft.com/windows/win32/api/ioapiset/nf-ioapiset-cancelioex
            unsafe {
                CancelIoEx(self.pipe, &*self.state);
                let mut ignored = 0;
                GetOverlappedResult(self.pipe, &*self.state, &mut ignored, 1);
            }
        }
    }
}
fn timed_out() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "native worker I/O deadline exceeded",
    )
}

/// Parent endpoint of a single private byte stream. A failed/timed-out operation
/// closes it: a partial packet must never be reused as a fresh framed message.
pub struct WorkerPipe {
    handle: Option<OwnedHandle>,
}
impl WorkerPipe {
    pub(crate) fn pair() -> io::Result<(Self, File)> {
        let mut random = [0u8; 32];
        let status = unsafe {
            BCryptGenRandom(
                null_mut(),
                random.as_mut_ptr(),
                random.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status < 0 {
            return Err(io::Error::other(format!(
                "worker pipe random identity failed: {status:#x}"
            )));
        }
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let name = wide(OsStr::new(&format!(
            r"\\.\pipe\LOCAL\GamePKG.worker.{suffix}"
        )))?;
        let raw = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_REJECT_REMOTE_CLIENTS,
                1,
                65536,
                65536,
                0,
                null(),
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        // Noninheritable until the launcher duplicates only its allowed stdio
        // rights. No endpoint name is provided to the worker.
        let raw = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let peer = unsafe { File::from_raw_handle(raw) };
        let mut connect = Operation::new(handle.as_raw_handle())?;
        let ok = unsafe { ConnectNamedPipe(handle.as_raw_handle(), &mut *connect.state) };
        let error = (ok == 0).then(|| unsafe { GetLastError() });
        if error.is_some_and(|value| value != ERROR_PIPE_CONNECTED) {
            connect.complete(error, Some(Instant::now() + Duration::from_secs(5)))?;
        }
        drop(connect);
        Ok((
            Self {
                handle: Some(handle),
            },
            peer,
        ))
    }
    fn operation(
        &mut self,
        bytes: *mut u8,
        length: usize,
        write: bool,
        deadline: Option<Instant>,
    ) -> io::Result<usize> {
        let result = (|| {
            let handle = self.handle.as_ref().ok_or_else(|| {
                io::Error::new(io::ErrorKind::BrokenPipe, "native worker channel is closed")
            })?;
            if deadline.is_some_and(|end| Instant::now() >= end) {
                return Err(timed_out());
            }
            let length = u32::try_from(length)
                .map_err(|_| invalid("native worker operation is too large"))?;
            let mut operation = Operation::new(handle.as_raw_handle())?;
            // The buffer stays borrowed through Operation::drop, including
            // cancellation completion. No pending operation escapes this call.
            let ok = unsafe {
                if write {
                    WriteFile(
                        handle.as_raw_handle(),
                        bytes.cast(),
                        length,
                        null_mut(),
                        &mut *operation.state,
                    )
                } else {
                    ReadFile(
                        handle.as_raw_handle(),
                        bytes,
                        length,
                        null_mut(),
                        &mut *operation.state,
                    )
                }
            };
            let error = (ok == 0).then(|| unsafe { GetLastError() });
            operation.complete(error, deadline)
        })();
        if result.is_err() {
            self.handle.take();
        }
        result
    }
    pub fn read_exact(&mut self, mut bytes: &mut [u8], deadline: Instant) -> io::Result<()> {
        while !bytes.is_empty() {
            let n = self.operation(bytes.as_mut_ptr(), bytes.len(), false, Some(deadline))?;
            if n == 0 {
                self.handle.take();
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "native worker stopped",
                ));
            }
            bytes = &mut bytes[n..];
        }
        Ok(())
    }
    /// Bounded reads also serve compiler stdout, whose text is not framed.
    pub fn read_some(&mut self, bytes: &mut [u8], deadline: Instant) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.operation(bytes.as_mut_ptr(), bytes.len(), false, Some(deadline))
    }
    pub fn write_all(&mut self, mut bytes: &[u8], deadline: Instant) -> io::Result<()> {
        while !bytes.is_empty() {
            let n = self.operation(bytes.as_ptr().cast_mut(), bytes.len(), true, Some(deadline))?;
            if n == 0 {
                self.handle.take();
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "native worker stopped reading",
                ));
            }
            bytes = &bytes[n..];
        }
        Ok(())
    }
}

/// Stderr only. A dedicated tail-bounded log drainer may block until job shutdown
/// closes its writer. The request channel deliberately does not implement Read
/// or Write, so it cannot accidentally bypass its cumulative deadline methods.
pub struct WorkerLog(WorkerPipe);
impl WorkerLog {
    pub(crate) fn new(pipe: WorkerPipe) -> Self {
        Self(pipe)
    }
}
impl io::Read for WorkerLog {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.0
            .operation(bytes.as_mut_ptr(), bytes.len(), false, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn duplex_worker_stream_preserves_fragmented_binary_payload() {
        let (mut pipe, mut peer) = WorkerPipe::pair().unwrap();
        let thread = std::thread::spawn(move || {
            let mut request = [0; 7];
            peer.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"\0\n\r\xffabc");
            for fragment in [b"one".as_slice(), b"\0\xfftwo"] {
                peer.write_all(fragment).unwrap();
            }
        });
        let end = Instant::now() + Duration::from_secs(3);
        pipe.write_all(b"\0\n\r\xffabc", end).unwrap();
        let mut reply = [0; 8];
        let n = pipe.read_some(&mut reply, end).unwrap();
        assert!((1..=reply.len()).contains(&n));
        pipe.read_exact(&mut reply[n..], end).unwrap();
        assert_eq!(&reply, b"one\0\xfftwo");
        thread.join().unwrap();
    }
    #[test]
    fn stalled_read_cancels_before_buffer_reuse_and_closes_channel() {
        let (mut pipe, _peer) = WorkerPipe::pair().unwrap();
        let mut bytes = [0; 4096];
        assert_eq!(
            pipe.read_exact(&mut bytes, Instant::now() + Duration::from_millis(30))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        bytes.fill(0x7e);
        std::thread::sleep(Duration::from_millis(20));
        assert!(bytes.iter().all(|b| *b == 0x7e));
        assert_eq!(
            pipe.write_all(b"next", Instant::now() + Duration::from_secs(1))
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
    #[test]
    fn blocked_large_write_is_cancelled_and_not_reused() {
        let (mut pipe, _peer) = WorkerPipe::pair().unwrap();
        let mut bytes = vec![0xa5; 8 * 1048576];
        let began = Instant::now();
        assert_eq!(
            pipe.write_all(&bytes, began + Duration::from_millis(30))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(began.elapsed() < Duration::from_secs(3));
        bytes.fill(0);
        assert_eq!(
            pipe.write_all(b"next", Instant::now() + Duration::from_secs(1))
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
