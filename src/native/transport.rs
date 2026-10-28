//! Platform-specific worker ownership and bounded I/O. The session's protocol,
//! permissions, frame/audio limits and service implementation remain shared.
#[cfg(target_os = "linux")]
use anyhow::bail;
use anyhow::{ensure, Context, Result};
use std::{
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
pub(super) fn append_engine_log(log: &mut Vec<u8>, bytes: &[u8]) {
    const MAX: usize = 4096;
    if bytes.len() >= MAX {
        log.clear();
        log.extend_from_slice(&bytes[bytes.len() - MAX..]);
        return;
    }
    if log.len() + bytes.len() > MAX {
        log.drain(..log.len() + bytes.len() - MAX);
    }
    log.extend_from_slice(bytes);
}

struct Log {
    bytes: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    done: std::sync::mpsc::Receiver<()>,
}
impl Log {
    fn start(mut source: impl Read + Send + 'static) -> Self {
        let bytes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = bytes.clone();
        let (send, done) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 2048];
            loop {
                match source.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let Ok(mut log) = sink.lock() else {
                            break;
                        };
                        append_engine_log(&mut log, &buffer[..n]);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            let _ = send.send(());
        });
        Self { bytes, done }
    }
    fn message(&self) -> String {
        let _ = self.done.recv_timeout(Duration::from_millis(100));
        self.bytes
            .lock()
            .ok()
            .map(|log| String::from_utf8_lossy(&log).trim().to_owned())
            .unwrap_or_default()
    }
}
#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::{
        io::Write,
        os::fd::{AsRawFd, RawFd},
        process::{Child, ChildStdin, ChildStdout, Stdio},
    };
    fn ready(fd: RawFd, events: i16, deadline: Instant) -> Result<()> {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            ensure!(!left.is_zero(), "native game exceeded its time limit");
            let mut p = libc::pollfd {
                fd,
                events,
                revents: 0,
            };
            let n = unsafe {
                libc::poll(
                    &mut p,
                    1,
                    left.as_millis().min(i32::MAX as u128).max(1) as i32,
                )
            };
            if n < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e.into());
            }
            ensure!(n > 0, "native game exceeded its time limit");
            ensure!(p.revents & libc::POLLNVAL == 0, "native worker pipe closed");
            return Ok(());
        }
    }
    fn transfer(pipe: &mut impl Read, fd: RawFd, mut out: &mut [u8], end: Instant) -> Result<()> {
        while !out.is_empty() {
            ready(fd, libc::POLLIN, end)?;
            match pipe.read(out) {
                Ok(0) => bail!("native game stopped"),
                Ok(n) => out = &mut out[n..],
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    pub(in crate::native) struct Worker {
        child: Child,
        _parent_thread: std::sync::mpsc::Sender<()>,
        input: ChildStdin,
        output: ChildStdout,
        log: Log,
    }
    impl Worker {
        pub(in crate::native) fn spawn(
            runtime: &Path,
            memory: u64,
            _identity: &str,
        ) -> Result<Self> {
            let mut command = std::process::Command::new("prlimit");
            command.arg(format!("--as={memory}")).args([
                "--core=0",
                "--fsize=0",
                "--",
                "bwrap",
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                "--clearenv",
                "--ro-bind",
                "/usr",
                "/usr",
            ]);
            for p in ["/lib", "/lib64", "/lib32"] {
                if Path::new(p).exists() {
                    command.args(["--ro-bind", p, p]);
                }
            }
            command
                .arg("--ro-bind")
                .arg(runtime)
                .arg("/game")
                .args([
                    "--proc",
                    "/proc",
                    "--dev",
                    "/dev",
                    "--tmpfs",
                    "/tmp",
                    "--chdir",
                    "/game",
                    "--",
                    "/game/game",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            // Linux parent-death signals follow the spawning thread. Keep that thread
            // alive when an asynchronous load hands the game to the UI thread.
            let (spawn_send, spawn_receive) = std::sync::mpsc::sync_channel(1);
            let (parent_thread, parent_lifetime) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = spawn_send.send(command.spawn());
                let _ = parent_lifetime.recv();
            });
            let mut child = spawn_receive
                .recv()
                .context("native launcher thread stopped")?
                .context(
                    "native playback requires bubblewrap and prlimit; isolation is mandatory",
                )?;
            let log = Log::start(child.stderr.take().unwrap());
            let input = child.stdin.take().unwrap();
            let output = child.stdout.take().unwrap();
            for fd in [input.as_raw_fd(), output.as_raw_fd()] {
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                if flags < 0
                    || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            Ok(Self {
                child,
                _parent_thread: parent_thread,
                input,
                output,
                log,
            })
        }
        pub(in crate::native) fn read_exact(
            &mut self,
            bytes: &mut [u8],
            end: Instant,
        ) -> Result<()> {
            let fd = self.output.as_raw_fd();
            transfer(&mut self.output, fd, bytes, end)
        }
        pub(in crate::native) fn write_all(
            &mut self,
            mut bytes: &[u8],
            end: Instant,
        ) -> Result<()> {
            while !bytes.is_empty() {
                ready(self.input.as_raw_fd(), libc::POLLOUT, end)?;
                match self.input.write(bytes) {
                    Ok(0) => bail!("native worker stopped reading"),
                    Ok(n) => bytes = &bytes[n..],
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(e) => return Err(e.into()),
                }
            }
            Ok(())
        }
        pub(in crate::native) fn stop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        pub(in crate::native) fn message(&self) -> String {
            self.log.message()
        }
    }
    impl Drop for Worker {
        fn drop(&mut self) {
            self.stop();
        }
    }
}
#[cfg(windows)]
mod platform {
    use super::*;
    use gamepkg_takedawn_windows_runner::{Process, Sandbox, WorkerPipe, WorkerProcess};
    pub(in crate::native) struct Worker {
        process: Process,
        io: WorkerPipe,
        log: Log,
    }
    impl Worker {
        pub(in crate::native) fn spawn(
            runtime: &Path,
            memory: u64,
            identity: &str,
        ) -> Result<Self> {
            ensure!(
                identity.len() == 64 && identity.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid native worker identity"
            );
            let sandbox = Sandbox::open(&format!("GamePKG.Native.{}", &identity[..32]))?;
            sandbox.grant_runtime(runtime)?;
            let system =
                std::env::var_os("SystemRoot").context("Windows system directory unavailable")?;
            let local = std::env::var_os("LOCALAPPDATA")
                .context("Windows local application data unavailable")?;
            let worker = WorkerProcess::spawn(
                &runtime.join("game.exe"),
                &[],
                runtime,
                &[
                    ("SystemRoot".into(), system),
                    ("LOCALAPPDATA".into(), local),
                ],
                &sandbox,
                memory,
            )
            .context("native playback requires Windows AppContainer isolation")?;
            let (process, io, errors) = worker.into_parts();
            // CreateProcess has copied the capability SID into the child's token. The
            // persistent profile/ACL remains; no SID pointer crosses the launch call.
            Ok(Self {
                process,
                io,
                log: Log::start(errors),
            })
        }
        pub(in crate::native) fn read_exact(
            &mut self,
            bytes: &mut [u8],
            end: Instant,
        ) -> Result<()> {
            self.io.read_exact(bytes, end).map_err(Into::into)
        }
        pub(in crate::native) fn write_all(&mut self, bytes: &[u8], end: Instant) -> Result<()> {
            self.io.write_all(bytes, end).map_err(Into::into)
        }
        pub(in crate::native) fn stop(&mut self) {
            let _ = self.process.stop();
        }
        pub(in crate::native) fn message(&self) -> String {
            self.log.message()
        }
    }
    impl Drop for Worker {
        fn drop(&mut self) {
            self.stop();
        }
    }
}

pub(super) use platform::Worker;
