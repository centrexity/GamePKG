//! Native Takedawn process ownership. Only host-reviewed cached builds may use
//! this entry point; it does not register an engine or accept package recipes.
use anyhow::{ensure, Context, Result};
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, OpenOptionsExt},
        unix::process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex},
};

pub const MEMORY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_BINARY: u64 = 128 * 1048576;
const MAX_DISC: u64 = 16 * 1024 * 1024 * 1024;
const LOG_BYTES: usize = 128 * 1024;
const STARTUP_BYTES: usize = 16 * 1024;

#[derive(Default)]
struct Diagnostics {
    startup: Vec<u8>,
    recent: VecDeque<u8>,
    omitted: bool,
    admitted: bool,
    window: bool,
    frames: u32,
}
impl Diagnostics {
    fn observe(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        self.admitted |= text.contains("verified disc admitted in place");
        self.window |= text.contains("[PRESENT] window up:");
        for (index, _) in text.match_indices("[GPU] flip ") {
            let number = text[index + "[GPU] flip ".len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>();
            if let Ok(frame) = number.parse() {
                self.frames = self.frames.max(frame);
            }
        }
    }
    fn append(&mut self, bytes: &[u8]) {
        let first = bytes.len().min(STARTUP_BYTES - self.startup.len());
        self.startup.extend_from_slice(&bytes[..first]);
        let bytes = &bytes[first..];
        let capacity = LOG_BYTES - STARTUP_BYTES;
        let discard = (self.recent.len() + bytes.len()).saturating_sub(capacity);
        self.omitted |= discard != 0;
        let old = discard.min(self.recent.len());
        self.recent.drain(..old);
        self.recent.extend(&bytes[discard - old..]);
    }
    fn text(&self) -> String {
        let mut bytes = self.startup.clone();
        if self.omitted {
            bytes.extend_from_slice(b"\n[GamePKG: earlier runtime output omitted]\n");
        }
        bytes.extend(self.recent.iter());
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// The native renderer owns its window. Headless is only for maintainer gates.
#[derive(Clone, Copy)]
pub struct Options {
    pub headless: bool,
    pub mute_audio: bool,
    pub audio_volume: u8,
    pub watchdog_seconds: u16,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            headless: false,
            mute_audio: false,
            audio_volume: 100,
            watchdog_seconds: 0,
        }
    }
}

fn directory(path: &Path) -> Result<File> {
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .or_else(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(error)
            }
        })?;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .context("game data must be a real directory")
}

fn scope_path(root: &Path, id: &str, creator: &str, schema: u32) -> PathBuf {
    crate::takedawn::data_path(root, id, creator, schema)
}

fn drain(
    mut pipe: impl Read + Send + 'static,
    log: Arc<Mutex<Diagnostics>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut bytes = [0; 4096];
        let mut overlap = Vec::new();
        loop {
            match pipe.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => {
                    let mut log = log.lock().unwrap_or_else(|e| e.into_inner());
                    overlap.extend_from_slice(&bytes[..n]);
                    log.observe(&overlap);
                    let keep = overlap.len().saturating_sub(64);
                    overlap.drain(..keep);
                    log.append(&bytes[..n]);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    })
}

fn safe_socket_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}

// Expose only selected desktop sockets, never the user's home or runtime folder.
fn desktop(command: &mut Command) -> Result<()> {
    let uid = unsafe { libc::geteuid() };
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let inside = format!("/run/user/{uid}");
    command.args([
        "--dir",
        "/run/user",
        "--dir",
        &inside,
        "--setenv",
        "XDG_RUNTIME_DIR",
        &inside,
    ]);
    if let (Some(runtime), Ok(display)) = (&runtime, std::env::var("WAYLAND_DISPLAY")) {
        ensure!(
            safe_socket_name(&display),
            "unsupported Wayland display name"
        );
        let socket = runtime.join(&display);
        ensure!(socket.exists(), "Wayland display is unavailable");
        command
            .arg("--ro-bind")
            .arg(socket)
            .arg(format!("{inside}/{display}"))
            .args([
                "--setenv",
                "WAYLAND_DISPLAY",
                &display,
                "--setenv",
                "SDL_VIDEODRIVER",
                "wayland",
            ]);
    } else if let Ok(display) = std::env::var("DISPLAY") {
        let number = display
            .strip_prefix(':')
            .context("a local desktop display is required")?
            .split('.')
            .next()
            .unwrap_or_default();
        ensure!(
            !number.is_empty() && number.len() <= 4 && number.bytes().all(|c| c.is_ascii_digit()),
            "invalid local display"
        );
        let socket = format!("/tmp/.X11-unix/X{number}");
        ensure!(Path::new(&socket).exists(), "X11 display is unavailable");
        command.args([
            "--dir",
            "/tmp/.X11-unix",
            "--ro-bind",
            &socket,
            &socket,
            "--setenv",
            "DISPLAY",
            &display,
            "--setenv",
            "SDL_VIDEODRIVER",
            "x11",
        ]);
        if let Some(auth) = std::env::var_os("XAUTHORITY") {
            command.arg("--ro-bind").arg(auth).arg("/xauthority").args([
                "--setenv",
                "XAUTHORITY",
                "/xauthority",
            ]);
        }
    } else {
        anyhow::bail!("no desktop display is available");
    }
    if let Some(runtime) = runtime {
        let pulse = runtime.join("pulse/native");
        if pulse.exists() {
            command
                .args(["--dir", "/run/pulse"])
                .arg("--ro-bind")
                .arg(pulse)
                .arg("/run/pulse/native")
                .args([
                    "--setenv",
                    "PULSE_SERVER",
                    "unix:/run/pulse/native",
                    "--setenv",
                    "SDL_AUDIODRIVER",
                    "pulseaudio",
                ]);
        }
    }
    Ok(())
}

/// Owns the process tree, its immutable executable snapshot and play-history
/// session. Persistent files are scoped to game, creator and save schema.
pub struct Session {
    child: Child,
    unit: String,
    parent: Option<std::sync::mpsc::Sender<()>>,
    _binary: tempfile::TempDir,
    history: crate::progress::Session,
    log: Arc<Mutex<Diagnostics>>,
    finished: bool,
    headless: bool,
    started: bool,
    data_lock: Option<File>,
}
impl Session {
    pub fn start_cached(
        root: &Path,
        id: &str,
        creator: &str,
        schema: u32,
        executable: &Path,
        iso: &Path,
        options: Options,
    ) -> Result<Self> {
        ensure!(options.audio_volume <= 100, "invalid game audio volume");
        ensure!(
            !id.is_empty()
                && id.len() <= 200
                && id != "."
                && id != ".."
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
                && !creator.is_empty()
                && creator.len() <= 256,
            "invalid native game identity"
        );
        let bytes = crate::files::read_regular(executable, MAX_BINARY, false)
            .context("build the reviewed native game before playing")?;
        crate::integrity::verify_build(executable, &bytes)?;
        crate::native::validate_executable(&bytes)?;
        let disc = crate::files::open_regular(iso, MAX_DISC, true)?;
        ensure!(
            disc.metadata()?.len() >= 33 * 2048,
            "select a supported Xbox ISO"
        );
        // Keep an open capability to the selected ISO. Neither replacing its
        // pathname nor selecting a large image copies it into the library.
        let _root = directory(root)?;
        let _parent = directory(&root.join("takedawn-data"))?;
        let data = directory(&scope_path(root, id, creator, schema))?;
        let _locks = directory(&root.join("takedawn-session-locks"))?;
        let key = scope_path(root, id, creator, schema)
            .file_name()
            .unwrap()
            .to_owned();
        let data_lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join("takedawn-session-locks").join(key))?;
        data_lock.try_lock().map_err(|error| {
            anyhow::anyhow!("This game is already running or its save folder is busy: {error}")
        })?;
        let snapshot = tempfile::Builder::new()
            .prefix("gamepkg-takedawn-native-")
            .tempdir()?;
        let binary = snapshot.path().join("engine");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o500)
            .open(&binary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        let unit = format!(
            "gamepkg-takedawn-{}.scope",
            &crate::verify::digest(snapshot.path().as_os_str().as_encoded_bytes())[..24]
        );
        let mut command = Command::new("/usr/bin/systemd-run");
        command.args([
            "--user",
            "--scope",
            "--quiet",
            "--collect",
            "--unit",
            &unit,
            "--property=MemoryMax=4294967296",
            "--property=MemorySwapMax=0",
            "--",
            "/usr/bin/bwrap",
            "--unshare-all",
            "--die-with-parent",
            "--new-session",
            "--clearenv",
            "--ro-bind",
            "/usr",
            "/usr",
        ]);
        for path in ["/lib", "/lib64"] {
            if Path::new(path).exists() {
                command.args(["--ro-bind", path, path]);
            }
        }
        command.args([
            "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp", "--dir", "/run",
        ]);
        for path in ["/sys", "/etc/ld.so.cache", "/etc/fonts", "/run/udev"] {
            if Path::new(path).exists() {
                command.args(["--ro-bind", path, path]);
            }
        }
        for path in ["/dev/dri", "/dev/snd", "/dev/input"] {
            if Path::new(path).exists() {
                command.args(["--dev-bind", path, path]);
            }
        }
        command
            .arg("--ro-bind")
            .arg(&binary)
            .arg("/engine")
            .arg("--ro-bind-fd")
            .arg(disc.as_raw_fd().to_string())
            .arg("/disc.iso")
            .arg("--bind-fd")
            .arg(data.as_raw_fd().to_string())
            .arg("/state")
            .args([
                "--chdir",
                "/state",
                "--setenv",
                "PATH",
                "/usr/bin:/bin",
                "--setenv",
                "LANG",
                "C.UTF-8",
                "--setenv",
                "HOME",
                "/state",
                "--setenv",
                "XDG_DATA_HOME",
                "/state/data",
                "--setenv",
                "XDG_CACHE_HOME",
                "/state/cache",
                "--setenv",
                "GAMEPKG_TAKEDAWN_DISC",
                "/disc.iso",
                "--setenv",
                "RECOMP_SAVE_DIR",
                "/state/saves",
                "--setenv",
                "RECOMP_RENDER_SHADER_CACHE",
                "/state/shader-cache",
                "--setenv",
                "RECOMP_PB_BACKEND",
                "vulkan",
            ]);
        command.args([
            "--setenv",
            "RECOMP_AUDIO_MUTE",
            if options.mute_audio { "1" } else { "0" },
        ]);
        command.args([
            "--setenv",
            "RECOMP_AUDIO_VOLUME",
            &format!("{:.2}", f64::from(options.audio_volume) / 100.0),
        ]);
        command.args([
            "--setenv",
            "RECOMP_HANG_WATCHDOG",
            &options.watchdog_seconds.to_string(),
        ]);
        if options.headless {
            command.args([
                "--setenv",
                "RECOMP_HEADLESS",
                "1",
                "--setenv",
                "SDL_AUDIODRIVER",
                "dummy",
                "--setenv",
                "RECOMP_FLIP_MARK",
                "1",
            ]);
        } else {
            desktop(&mut command)?;
        }
        command.args(["--remount-ro", "/", "--", "/engine"]);
        if options.headless {
            command.arg("--headless");
        }
        command
            .arg("/gamepkg-disc/default.xbe")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let fds = [disc.as_raw_fd(), data.as_raw_fd()];
        unsafe {
            command.pre_exec(move || {
                for fd in fds {
                    let flags = libc::fcntl(fd, libc::F_GETFD);
                    if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let history = crate::progress::Session::open(root, id, creator)?;
        // A live owner thread is needed because bwrap tracks its spawning thread.
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let (parent, lifetime) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(command.spawn());
            let _ = lifetime.recv();
        });
        let mut child = receiver
            .recv()
            .context("native launcher thread stopped")??;
        let log = Arc::new(Mutex::new(Diagnostics::default()));
        drain(
            child.stdout.take().context("missing game output")?,
            log.clone(),
        );
        drain(
            child.stderr.take().context("missing game diagnostics")?,
            log.clone(),
        );
        Ok(Self {
            child,
            unit,
            parent: Some(parent),
            _binary: snapshot,
            history,
            log,
            finished: false,
            headless: options.headless,
            started: false,
            data_lock: Some(data_lock),
        })
    }
    pub fn diagnostics(&self) -> String {
        let log = self.log.lock().unwrap_or_else(|e| e.into_inner());
        log.text()
    }
    pub fn ready(&self) -> bool {
        let log = self.log.lock().unwrap_or_else(|e| e.into_inner());
        log.admitted
            && if self.headless {
                log.frames > 0
            } else {
                log.window
            }
    }
    pub fn disc_admitted(&self) -> bool {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).admitted
    }
    pub fn frame_count(&self) -> u32 {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).frames
    }
    pub fn poll(&mut self) -> Result<Option<ExitStatus>> {
        if !self.started && self.ready() {
            self.history.start();
            self.started = true;
        }
        let status = self.child.try_wait()?;
        if status.is_some() && !self.finished {
            self.history.finish()?;
            self.finished = true;
            self.parent.take();
            self.data_lock.take();
        } else if !self.finished && self.started {
            self.history.tick(&[])?;
        }
        Ok(status)
    }
    pub fn stop(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_none() {
            let _ = Command::new("/usr/bin/systemctl")
                .args(["--user", "stop", &self.unit])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = self.child.kill();
            self.child.wait()?;
        }
        self.parent.take();
        self.history.finish()?;
        self.finished = true;
        self.data_lock.take();
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn data_is_scoped_and_directory_links_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let first = scope_path(root.path(), "game", "creator", 1);
        assert_ne!(first, scope_path(root.path(), "game", "other", 1));
        assert_ne!(first, scope_path(root.path(), "game", "creator", 2));
        assert_ne!(first, scope_path(root.path(), "other", "creator", 1));
        let link = root.path().join("link");
        std::os::unix::fs::symlink(root.path(), &link).unwrap();
        assert!(directory(&link).is_err());
        assert!(!safe_socket_name("../wayland-0"));
        assert!(!safe_socket_name("/tmp/socket"));
        assert!(safe_socket_name("wayland-0"));
    }
    #[test]
    fn noisy_runtime_retains_startup_and_last_error_with_bounded_storage() {
        for chunks in [1, 4096, LOG_BYTES * 2] {
            let mut log = Diagnostics::default();
            log.append(b"verified disc admitted in place\n");
            let noise = vec![b'x'; LOG_BYTES * 3];
            for chunk in noise.chunks(chunks) {
                log.append(chunk);
            }
            log.append(b"\nlast runtime error\n");
            assert!(log.startup.len() + log.recent.len() <= LOG_BYTES);
            let text = log.text();
            assert!(text.starts_with("verified disc admitted in place\n"));
            assert!(text.ends_with("\nlast runtime error\n"));
            assert!(text.contains("earlier runtime output omitted"));
        }
    }
    #[test]
    fn fragmented_ready_messages_survive_verbose_output_and_frame_reordering() {
        struct Fragmented {
            data: std::io::Cursor<Vec<u8>>,
            chunk: usize,
        }
        impl Read for Fragmented {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                let count = out.len().min(self.chunk);
                self.data.read(&mut out[..count])
            }
        }
        for chunk in [1, 7, 4096] {
            let mut bytes=b"GamePKG: verified disc admitted in place\n[PRESENT] window up: Vulkan swapchain\n[GPU] flip 12345 999 ms\n".to_vec();
            bytes.extend(vec![b'x'; LOG_BYTES * 2]);
            bytes.extend(b"\n[GPU] flip 7 1000 ms\n");
            let log = Arc::new(Mutex::new(Diagnostics::default()));
            drain(
                Fragmented {
                    data: std::io::Cursor::new(bytes),
                    chunk,
                },
                log.clone(),
            )
            .join()
            .unwrap();
            let log = log.lock().unwrap();
            assert!(log.admitted && log.window);
            assert_eq!(log.frames, 12345);
            assert!(log.startup.len() + log.recent.len() <= LOG_BYTES);
        }
    }
    #[test]
    #[ignore = "requires a privately built reviewed native game, owned ISO, Vulkan and systemd user scope"]
    fn private_native_launch_tracks_time_and_closes() {
        let executable = PathBuf::from(std::env::var_os("GAMEPKG_TAKEDAWN_EXECUTABLE").unwrap());
        let iso = PathBuf::from(std::env::var_os("GAMEPKG_TAKEDAWN_ISO").unwrap());
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let bytes = crate::files::read_regular(&executable, MAX_BINARY, false).unwrap();
        fs::write(&cache, &bytes).unwrap();
        crate::integrity::record_build(&cache, &bytes).unwrap();
        let headless = std::env::var_os("GAMEPKG_TAKEDAWN_WINDOW_TEST").is_none();
        let mut game = Session::start_cached(
            root.path(),
            "test-game",
            "test-creator",
            1,
            &cache,
            &iso,
            Options {
                headless,
                ..Default::default()
            },
        )
        .unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(45);
        loop {
            assert!(game.poll().unwrap().is_none(), "{}", game.diagnostics());
            let ready = if headless {
                "[GPU] flip 120 "
            } else {
                "[PRESENT] window up: Vulkan swapchain (zero-copy)"
            };
            if game.diagnostics().contains(ready) {
                break;
            }
            assert!(std::time::Instant::now() < end, "{}", game.diagnostics());
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !headless {
            // Leave time for original frames to reach the actual native window.
            let end = std::time::Instant::now() + std::time::Duration::from_secs(25);
            while std::time::Instant::now() < end {
                assert!(game.poll().unwrap().is_none(), "{}", game.diagnostics());
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        assert!(game
            .diagnostics()
            .contains("verified disc admitted in place"));
        assert!(game.ready());
        assert!(Session::start_cached(
            root.path(),
            "test-game",
            "test-creator",
            1,
            &cache,
            &iso,
            Options {
                headless,
                ..Default::default()
            }
        )
        .err()
        .unwrap()
        .to_string()
        .contains("already running"));
        game.stop().unwrap();
        assert!(game.poll().unwrap().is_some());
        assert!(
            crate::progress::load(root.path(), "test-game", "test-creator")
                .unwrap()
                .played_ms
                > 1000
        );
        assert_eq!(
            crate::progress::load(root.path(), "test-game", "other-creator")
                .unwrap()
                .played_ms,
            0
        );
        assert!(!scope_path(root.path(), "test-game", "test-creator", 1)
            .join("default.xbe")
            .exists());
    }
}
