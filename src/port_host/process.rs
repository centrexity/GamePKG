//! Private service transport for checked native artifacts on Linux/Windows.
//! Children receive no ISO/save/device/display/audio grants at OS level.
use super::{
    broker::Connection,
    preparation::{Artifact, Bootstrap, Target},
};
#[cfg(all(
    feature = "gui",
    any(windows, all(target_os = "linux", target_arch = "x86_64"))
))]
#[path = "process_history.rs"]
mod history;
#[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
#[path = "process_local.rs"]
mod local;
#[cfg(all(feature = "gui", windows))]
#[path = "process_windows.rs"]
mod windows;
use anyhow::{ensure, Context, Result};
#[cfg(windows)]
use std::io::Write;
#[cfg(target_os = "linux")]
use std::path::Path;
use std::{
    collections::VecDeque,
    fs,
    io::Read,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};
#[cfg(all(
    feature = "gui",
    any(windows, all(target_os = "linux", target_arch = "x86_64"))
))]
/// Trusted launch arguments shared by the launcher and linked executable.
#[derive(Clone, Copy, Debug, clap::Parser)]
pub struct NativeLaunch {
    #[command(flatten)]
    pub graphics: crate::launch_options::LaunchArgs,
    #[arg(long)]
    pub visible: bool,
    #[arg(long)]
    pub mute_audio: bool,
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u8).range(0..=100))]
    pub volume: u8,
}

#[cfg(all(
    feature = "gui",
    any(windows, all(target_os = "linux", target_arch = "x86_64"))
))]
impl NativeLaunch {
    pub(super) fn arguments(self, manifest: &crate::manifest::Manifest) -> Result<Vec<String>> {
        ensure!(self.volume <= 100, "audio volume must be 0–100");
        let mut args = self.graphics.resolved(manifest.launch_options)?.arguments();
        args.extend(["--volume".into(), self.volume.to_string()]);
        if self.visible {
            args.push("--visible".into());
        }
        if self.mute_audio {
            args.push("--mute-audio".into());
        }
        Ok(args)
    }
}

/// Windows desktop compatibility currently admits offline, image-based services.
#[cfg(all(
    feature = "gui",
    any(windows, all(target_os = "linux", target_arch = "x86_64"))
))]
pub fn require_windows_compatibility(manifest: &crate::manifest::Manifest) -> Result<()> {
    ensure!(
        manifest.uses_service_runtime()
            && !manifest.permissions.network
            && !manifest.permissions.microphone
            && manifest.permissions.files.is_empty()
            && manifest.shared_assets.is_empty()
            && manifest.native_graphics.is_none(),
        "Windows desktop compatibility requires offline native services with the image renderer"
    );
    Ok(())
}

const LOG: usize = 128 * 1024;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, OnceLock,
};
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
struct Permit;
impl Permit {
    fn acquire() -> Result<Self> {
        ACTIVE
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 8).then_some(n + 1)
            })
            .map_err(|_| anyhow::anyhow!("native session limit reached"))?;
        Ok(Self)
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
    }
}
#[cfg(target_os = "linux")]
type NativeChild = std::process::Child;
#[cfg(windows)]
type NativeChild = gamepkg_takedawn_windows_runner::Process;
#[cfg(not(any(windows, target_os = "linux")))]
type NativeChild = ();
struct Reap {
    child: NativeChild,
    directory: tempfile::TempDir,
    _permit: Permit,
    #[cfg(target_os = "linux")]
    _launcher: Option<mpsc::Sender<()>>,
    #[cfg(all(
        feature = "gui",
        any(windows, all(target_os = "linux", target_arch = "x86_64"))
    ))]
    history: Option<JoinHandle<Result<()>>>,
}
fn reaper() -> Result<&'static mpsc::SyncSender<Reap>> {
    static REAPER: OnceLock<std::result::Result<mpsc::SyncSender<Reap>, String>> = OnceLock::new();
    REAPER
        .get_or_init(|| {
            // Each started session reserves one of eight permits BEFORE spawning;
            // therefore its teardown always fits this bounded queue without wait.
            let (send, receive) = mpsc::sync_channel::<Reap>(8);
            std::thread::Builder::new()
                .name("gamepkg-reaper".into())
                .spawn(move || {
                    while let Ok(job) = receive.recv() {
                        #[cfg(any(target_os = "linux", all(windows, feature = "gui")))]
                        let mut job = job;
                        #[cfg(target_os = "linux")]
                        let _ = job.child.wait();
                        #[cfg(windows)]
                        let _ = job.child.stop();
                        #[cfg(all(
                            feature = "gui",
                            any(windows, all(target_os = "linux", target_arch = "x86_64"))
                        ))]
                        if let Some(history) = job.history.take() {
                            match history.join() {
                                Ok(Ok(())) => {}
                                Ok(Err(error)) => eprintln!("Unable to save play history: {error}"),
                                Err(_) => {
                                    eprintln!("Native play history worker stopped unexpectedly")
                                }
                            }
                        }
                        drop(job.child);
                        drop(job.directory);
                        drop(job._permit);
                    }
                })
                .map(|_| send)
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!("native reaper unavailable: {e}"))
}
pub struct Process {
    #[cfg(target_os = "linux")]
    child: Option<std::process::Child>,
    #[cfg(windows)]
    child: Option<gamepkg_takedawn_windows_runner::Process>,
    #[cfg(windows)]
    _sandbox: gamepkg_takedawn_windows_runner::Sandbox,
    directory: Option<tempfile::TempDir>,
    log: Arc<Mutex<VecDeque<u8>>>,
    transport: Option<JoinHandle<std::io::Result<()>>>,
    clock: Option<super::timing::Clock>,
    #[cfg(target_os = "linux")]
    launcher: Option<mpsc::Sender<()>>,
    #[cfg(all(
        feature = "gui",
        any(windows, all(target_os = "linux", target_arch = "x86_64"))
    ))]
    history: Option<JoinHandle<Result<()>>>,
    #[cfg(all(
        feature = "gui",
        any(windows, all(target_os = "linux", target_arch = "x86_64"))
    ))]
    game_frame: Option<Arc<std::sync::atomic::AtomicBool>>,
    permit: Option<Permit>,
}
fn tail(log: &Arc<Mutex<VecDeque<u8>>>, bytes: &[u8]) {
    if let Ok(mut log) = log.lock() {
        let bytes = &bytes[bytes.len().saturating_sub(LOG)..];
        let discard = (log.len() + bytes.len()).saturating_sub(LOG);
        log.drain(..discard);
        log.extend(bytes);
    }
}
impl Process {
    pub fn start(artifact: &Artifact, connection: Connection, memory: u64) -> Result<Self> {
        ensure!(
            artifact.receipt().bootstrap == Bootstrap::HistoricalPipe,
            "in-process artifact requires its own launcher"
        );
        let permit = Permit::acquire()?;
        let _ = reaper()?;
        let clock = connection
            .clock()
            .context("native launch requires session-owned worker services")?;
        let target = artifact.receipt().plan.target();
        ensure!(
            matches!(target, Target::Linux64 | Target::Linux32) == cfg!(target_os = "linux"),
            "service artifact belongs to another operating system"
        );
        ensure!(
            (16 * 1024 * 1024..=16 * 1024 * 1024 * 1024).contains(&memory),
            "invalid native worker memory budget"
        );
        let source = artifact.executable()?;
        let bytes = crate::files::read_regular(&source, 256 * 1024 * 1024, false)?;
        ensure!(
            crate::verify::digest(&bytes) == artifact.receipt().binary,
            "service artifact changed before launch"
        );
        let directory = tempfile::tempdir()?;
        let executable = directory
            .path()
            .join(if cfg!(windows) { "game.exe" } else { "game" });
        fs::write(&executable, &bytes)?;
        let log = Arc::new(Mutex::new(VecDeque::new()));
        #[cfg(target_os = "linux")]
        {
            use std::{
                os::unix::{fs::PermissionsExt, process::CommandExt},
                process::{Command, Stdio},
            };
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o500))?;
            let mut command = Command::new("prlimit");
            command.arg(format!("--as={memory}")).args([
                "--core=0",
                "--fsize=0",
                "--",
                "bwrap",
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                "--clearenv",
            ]);
            for path in [
                "/lib",
                "/lib64",
                "/lib32",
                "/libx32",
                "/usr/lib",
                "/usr/lib64",
                "/etc/ld.so.cache",
            ] {
                if Path::new(path).exists() {
                    command.args(["--ro-bind", path, path]);
                }
            }
            command
                .arg("--ro-bind")
                .arg(directory.path())
                .arg("/runtime")
                .args([
                    "--proc",
                    "/proc",
                    "--dev",
                    "/dev",
                    "--tmpfs",
                    "/tmp",
                    "--chdir",
                    "/runtime",
                    "--",
                    "/runtime/game",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let parent = unsafe { libc::getpid() };
            unsafe {
                command.pre_exec(move || {
                    // Retain ONLY stdio across exec, including when a driver
                    // opened a descriptor without CLOEXEC in the GUI process.
                    if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::getppid() != parent {
                        return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
                    }
                    Ok(())
                });
            }
            let mut child = command
                .spawn()
                .context("service games require Linux process confinement")?;
            let input = child.stdout.take().unwrap();
            let output = child.stdin.take().unwrap();
            let mut errors = child.stderr.take().unwrap();
            let error_log = log.clone();
            std::thread::spawn(move || {
                let mut bytes = [0; 4096];
                while let Ok(n) = errors.read(&mut bytes) {
                    if n == 0 {
                        break;
                    }
                    tail(&error_log, &bytes[..n]);
                }
            });
            let transport = std::thread::Builder::new()
                .name("gamepkg-services".into())
                .spawn(move || connection.serve(input, output));
            match transport {
                Ok(worker) => Ok(Self {
                    child: Some(child),
                    directory: Some(directory),
                    clock: Some(clock),
                    launcher: None,
                    #[cfg(all(feature = "gui", target_arch = "x86_64"))]
                    history: None,
                    #[cfg(all(feature = "gui", target_arch = "x86_64"))]
                    game_frame: None,
                    permit: Some(permit),
                    log,
                    transport: Some(worker),
                }),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    Err(error.into())
                }
            }
        }
        #[cfg(windows)]
        {
            use gamepkg_takedawn_windows_runner::{Sandbox, WorkerProcess};
            let identity = crate::verify::digest(directory.path().as_os_str().as_encoded_bytes());
            let sandbox = Sandbox::open(&format!("GamePKG.Services.{}", &identity[..32]))?;
            sandbox.grant_runtime(directory.path())?;
            let values = vec![
                (
                    "SystemRoot".into(),
                    std::env::var_os("SystemRoot")
                        .context("Windows system directory unavailable")?,
                ),
                (
                    "LOCALAPPDATA".into(),
                    std::env::var_os("LOCALAPPDATA").context("Windows profile unavailable")?,
                ),
            ];
            let worker = WorkerProcess::spawn(
                &executable,
                &[],
                directory.path(),
                &values,
                &sandbox,
                memory,
            )?;
            let (child, pipe, mut errors) = worker.into_parts();
            let error_log = log.clone();
            std::thread::spawn(move || {
                let mut bytes = [0; 4096];
                while let Ok(n) = errors.read(&mut bytes) {
                    if n == 0 {
                        break;
                    }
                    tail(&error_log, &bytes[..n]);
                }
            });
            #[derive(Clone)]
            struct Io(Arc<Mutex<gamepkg_takedawn_windows_runner::ServiceIo>>);
            impl Read for Io {
                fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                    self.0
                        .lock()
                        .map_err(|_| std::io::Error::other("service pipe lock"))?
                        .read(bytes)
                }
            }
            impl Write for Io {
                fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                    self.0
                        .lock()
                        .map_err(|_| std::io::Error::other("service pipe lock"))?
                        .write(bytes)
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }
            let io = Io(Arc::new(Mutex::new(pipe.into_service_io())));
            let transport = std::thread::Builder::new()
                .name("gamepkg-services".into())
                .spawn(move || connection.serve(io.clone(), io))?;
            Ok(Self {
                child: Some(child),
                _sandbox: sandbox,
                #[cfg(feature = "gui")]
                history: None,
                #[cfg(feature = "gui")]
                game_frame: None,
                directory: Some(directory),
                clock: Some(clock),
                permit: Some(permit),
                log,
                transport: Some(transport),
            })
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        anyhow::bail!("native service process confinement is unavailable on this platform")
    }
    pub fn poll(&mut self) -> Result<Option<u32>> {
        if self.permit.is_none() {
            return Ok(Some(1));
        }
        if self.transport.as_ref().is_some_and(JoinHandle::is_finished) {
            let result = self
                .transport
                .take()
                .unwrap()
                .join()
                .map_err(|_| anyhow::anyhow!("service worker panicked"));
            if let Err(error) = result {
                self.stop()?;
                return Err(error);
            }
            if let Err(error) = result.unwrap() {
                self.stop()?;
                return Err(error.into());
            }
        }
        #[cfg(target_os = "linux")]
        let status = {
            use std::os::unix::process::ExitStatusExt;
            self.child.as_mut().unwrap().try_wait()?.map(|s| {
                s.code()
                    .map(|v| v as u32)
                    .unwrap_or(128 + s.signal().unwrap_or(0) as u32)
            })
        };
        #[cfg(windows)]
        let status = self.child.as_ref().unwrap().poll()?;
        #[cfg(not(any(windows, target_os = "linux")))]
        let status = None;
        #[cfg(all(
            feature = "gui",
            any(windows, all(target_os = "linux", target_arch = "x86_64"))
        ))]
        if status.is_some() {
            if let Some(history) = &self.history {
                if !history.is_finished() {
                    return Ok(None);
                }
                self.history.take().unwrap().join().map_err(|_| {
                    anyhow::anyhow!("native play history worker stopped unexpectedly")
                })??;
            }
        }
        Ok(status)
    }
    #[cfg(all(
        feature = "gui",
        any(windows, all(target_os = "linux", target_arch = "x86_64"))
    ))]
    pub fn first_frame_presented(&self) -> bool {
        self.game_frame
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
    }
    pub fn diagnostics(&self) -> String {
        self.log
            .lock()
            .map(|bytes| {
                String::from_utf8_lossy(&bytes.iter().copied().collect::<Vec<_>>()).into_owned()
            })
            .unwrap_or_default()
    }
    pub fn stop(&mut self) -> Result<()> {
        if let Some(clock) = &self.clock {
            clock.close();
        }
        if self.permit.is_none() {
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        if self.child.as_mut().unwrap().try_wait()?.is_none() {
            self.child.as_mut().unwrap().kill()?;
        }
        #[cfg(windows)]
        self.child.as_ref().unwrap().request_stop()?;
        let job = Reap {
            child: self.child.take().unwrap(),
            directory: self.directory.take().unwrap(),
            _permit: self.permit.take().unwrap(),
            #[cfg(target_os = "linux")]
            _launcher: self.launcher.take(),
            #[cfg(all(
                feature = "gui",
                any(windows, all(target_os = "linux", target_arch = "x86_64"))
            ))]
            history: self.history.take(),
        };
        reaper()?
            .send(job)
            .map_err(|_| anyhow::anyhow!("native reaper stopped"))?;
        Ok(())
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
