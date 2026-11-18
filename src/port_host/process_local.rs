//! Linux native executable launch. Runtime services stay in the executable.
use super::{reaper, tail, Artifact, Bootstrap, Permit, Process, Target};
use crate::launch_options::LaunchArgs;
use anyhow::{ensure, Context, Result};
use std::{
    collections::VecDeque,
    fs,
    io::{self, Read},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

/// Trusted launch arguments shared by the launcher and linked executable.
#[derive(Clone, Copy, Debug, clap::Parser)]
pub struct NativeLaunch {
    #[command(flatten)]
    pub graphics: LaunchArgs,
    #[arg(long)]
    pub visible: bool,
    #[arg(long)]
    pub mute_audio: bool,
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u8).range(0..=100))]
    pub volume: u8,
}

impl NativeLaunch {
    fn arguments(self, manifest: &crate::manifest::Manifest) -> Result<Vec<String>> {
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

fn read_only_system(command: &mut Command, paths: &[&str]) {
    for &path in paths {
        if Path::new(path).exists() {
            command.args(["--ro-bind", path, path]);
        }
    }
}

fn socket(command: &mut Command, source: &Path, target: &Path) -> Result<()> {
    ensure!(
        fs::symlink_metadata(source)?.file_type().is_socket(),
        "desktop endpoint is not a local socket"
    );
    command.arg("--ro-bind").arg(source).arg(target);
    Ok(())
}

fn desktop(command: &mut Command, audio: bool) -> Result<()> {
    let uid = unsafe { libc::geteuid() };
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let inside = PathBuf::from(format!("/run/user/{uid}"));
    command
        .args(["--perms", "0700", "--dir"])
        .arg(&inside)
        .args(["--setenv", "XDG_RUNTIME_DIR"])
        .arg(&inside);
    if let Some(display) = std::env::var_os("WAYLAND_DISPLAY").filter(|d| !d.is_empty()) {
        let runtime = runtime
            .as_ref()
            .context("Wayland runtime directory is missing")?;
        let display = display.to_str().context("invalid Wayland display")?;
        ensure!(
            !matches!(display, "." | "..")
                && display.len() <= 128
                && display
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c)),
            "invalid local Wayland display name"
        );
        socket(command, &runtime.join(display), &inside.join(display))?;
        command.args(["--setenv", "WAYLAND_DISPLAY", display]);
    } else {
        let display = std::env::var("DISPLAY").context("a local desktop display is required")?;
        let suffix = display
            .strip_prefix(':')
            .context("a local X11 display is required")?;
        let (number, screen) = suffix.split_once('.').unwrap_or((suffix, "0"));
        ensure!(
            !number.is_empty()
                && number.len() <= 4
                && number.bytes().all(|c| c.is_ascii_digit())
                && !screen.is_empty()
                && screen.len() <= 4
                && screen.bytes().all(|c| c.is_ascii_digit()),
            "invalid local X11 display"
        );
        let endpoint = PathBuf::from(format!("/tmp/.X11-unix/X{number}"));
        socket(command, &endpoint, &endpoint)?;
        command.args(["--setenv", "DISPLAY", &display]);
        let auth = std::env::var_os("XAUTHORITY")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".Xauthority"))
            });
        if let Some(auth) = auth.filter(|path| path.is_file()) {
            command
                .arg("--ro-bind")
                .arg(auth)
                .arg("/run/gamepkg-xauthority")
                .args(["--setenv", "XAUTHORITY", "/run/gamepkg-xauthority"]);
        }
    }
    if audio {
        if let Some(pulse) = runtime
            .as_ref()
            .map(|path| path.join("pulse/native"))
            .filter(|p| p.exists())
        {
            socket(command, &pulse, &inside.join("pulse/native"))?;
        }
    }
    Ok(())
}

fn devices(
    command: &mut Command,
    dir: &str,
    permitted: impl Fn(&str, &fs::Metadata) -> bool,
) -> Result<()> {
    if !Path::new(dir).exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_char_device()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| permitted(name, &metadata))
        {
            command
                .arg("--dev-bind")
                .arg(entry.path())
                .arg(entry.path());
        }
    }
    Ok(())
}

fn controller(name: &str, metadata: &fs::Metadata) -> bool {
    if !name.starts_with("event") {
        return false;
    }
    let device = metadata.rdev();
    let extra = format!(
        "/run/udev/data/c{}:{}",
        libc::major(device),
        libc::minor(device)
    );
    fs::read_to_string(extra)
        .ok()
        .is_some_and(|info| info.lines().any(|line| line == "E:ID_INPUT_JOYSTICK=1"))
}

impl Process {
    /// Start a receipt-verified Linux game in its own memory scope and
    /// namespace. The runtime receives an optional read-only ROM and a
    /// capability mount for an approved private save scope.
    pub fn start_in_process(
        artifact: &Artifact,
        library: &Path,
        rom: Option<fs::File>,
        save: Option<(&Path, u64)>,
        options: NativeLaunch,
    ) -> Result<Self> {
        ensure!(
            artifact.receipt().bootstrap == Bootstrap::InProcess
                && artifact.receipt().plan.target() == Target::Linux64,
            "Linux x64 in-process artifact required"
        );
        let manifest = artifact.manifest();
        let arguments = options.arguments(manifest)?;
        ensure!(
            manifest.required_rom_count() == usize::from(rom.is_some()),
            "wrong number of selected ROMs"
        );
        if let Some(rom) = &rom {
            let flags = unsafe { libc::fcntl(rom.as_raw_fd(), libc::F_GETFL) };
            ensure!(
                flags >= 0
                    && flags & (libc::O_ACCMODE | libc::O_PATH) == libc::O_RDONLY
                    && rom.metadata()?.is_file(),
                "selected ROM must be a read-only regular file"
            );
        }
        let save_mount = save
            .map(|(root, quota)| {
                crate::port_host::owner::save_scope(root, manifest).map(|dir| (dir, quota))
            })
            .transpose()?;
        let history = crate::progress::Session::open(library, &manifest.id, &manifest.creator_key)?;
        let permit = Permit::acquire()?;
        reaper()?;
        let source = artifact.executable()?;
        let bytes = crate::files::read_regular(&source, 256 * 1024 * 1024, false)?;
        ensure!(
            crate::verify::digest(&bytes) == artifact.receipt().binary,
            "service executable changed before launch"
        );
        let directory = tempfile::tempdir()?;
        let executable = directory.path().join("game");
        fs::write(&executable, bytes)?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o500))?;
        let unit = format!(
            "gamepkg-native-{}.scope",
            &crate::verify::digest(directory.path().as_os_str().as_encoded_bytes())[..24]
        );
        let memory = manifest.limits.hard_memory_bytes;
        let host_netns = fs::metadata("/proc/self/ns/net")?.ino().to_string();
        let mut command = Command::new("/usr/bin/systemd-run");
        command
            .args(["--user", "--scope", "--quiet", "--collect", "--unit", &unit])
            .arg(format!("--property=MemoryMax={memory}"))
            .args([
                "--property=MemorySwapMax=0",
                "--",
                "/usr/bin/bwrap",
                "--unshare-user",
                "--unshare-pid",
                "--unshare-ipc",
                "--unshare-uts",
                "--disable-userns",
                "--die-with-parent",
                "--new-session",
                "--clearenv",
            ]);
        if manifest.permissions.network {
            read_only_system(
                &mut command,
                &[
                    "/etc/resolv.conf",
                    "/etc/hosts",
                    "/etc/nsswitch.conf",
                    "/etc/ssl/certs",
                ],
            );
        } else {
            command.arg("--unshare-net");
        }
        read_only_system(
            &mut command,
            &[
                "/lib",
                "/lib64",
                "/usr/lib",
                "/usr/lib64",
                "/usr/share",
                "/etc/ld.so.cache",
                "/etc/vulkan",
                "/etc/fonts",
            ],
        );
        command.args([
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--dir",
            "/run",
            "--dir",
            "/home/game",
            "--setenv",
            "HOME",
            "/home/game",
            "--setenv",
            "XDG_CACHE_HOME",
            "/tmp/cache",
            "--setenv",
            "GAMEPKG_MEMORY_SCOPE",
            &unit,
            "--setenv",
            "GAMEPKG_PARENT_NETWORK_NS",
            &host_netns,
        ]);
        read_only_system(
            &mut command,
            &["/sys/dev", "/sys/devices", "/sys/class/drm"],
        );
        devices(&mut command, "/dev/dri", |name, _| {
            name.starts_with("renderD")
        })?;
        for path in crate::takedawn_player::nvidia_devices(Path::new("/dev"))? {
            command.arg("--dev-bind").arg(&path).arg(&path);
        }
        if manifest.permissions.input {
            read_only_system(&mut command, &["/run/udev/data", "/sys/class/input"]);
            devices(&mut command, "/dev/input", controller)?;
        }
        if manifest.permissions.audio {
            read_only_system(
                &mut command,
                &["/etc/alsa", "/etc/asound.conf", "/sys/class/sound"],
            );
            devices(&mut command, "/dev/snd", |name, _| {
                name.starts_with("controlC")
                    || name == "timer"
                    || (name.starts_with("pcmC") && name.ends_with('p'))
            })?;
        }
        desktop(&mut command, manifest.permissions.audio)?;
        let save_fd = save_mount.as_ref().map(|(scope, quota)| {
            let fd = scope.as_raw_fd();
            command
                .arg("--bind-fd")
                .arg(fd.to_string())
                .arg("/run/gamepkg-save")
                .args(["--setenv", "GAMEPKG_SAVE_QUOTA"])
                .arg(quota.to_string());
            fd
        });
        command.arg("--ro-bind").arg(executable).arg("/game").args([
            "--remount-ro",
            "/",
            "--chdir",
            "/home/game",
            "--",
            "/game",
        ]);
        command
            .args(arguments)
            .stdin(rom.map_or_else(Stdio::null, Stdio::from))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            command.pre_exec(move || {
                if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) != 0 {
                    return Err(io::Error::last_os_error());
                }
                let limit = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0 {
                    return Err(io::Error::last_os_error());
                }
                if let Some(fd) = save_fd {
                    let flags = libc::fcntl(fd, libc::F_GETFD);
                    if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        // Bubblewrap monitors its parent thread. Keep it alive until the
        // process is reaped even when the library caller moves between threads.
        let (send_child, receive_child) = mpsc::sync_channel(1);
        let (launcher, lifetime) = mpsc::channel();
        let (ready, readiness) = mpsc::sync_channel(1);
        let history = std::thread::Builder::new()
            .name("gamepkg-play-history".into())
            .spawn(move || record_history(history, readiness))?;
        std::thread::Builder::new()
            .name("gamepkg-native-launch".into())
            .spawn(move || {
                if send_child.send(command.spawn()).is_ok() {
                    let _ = lifetime.recv();
                }
            })?;
        let mut child = receive_child
            .recv()
            .context("native launcher thread exited")??;
        drop(save_mount);
        let log = Arc::new(Mutex::new(VecDeque::new()));
        let stdout = child.stdout.take().context("missing native game output")?;
        let stderr = child
            .stderr
            .take()
            .context("missing native game diagnostics")?;
        let game_frame = Arc::new(AtomicBool::new(false));
        let presented = game_frame.clone();
        let output_log = log.clone();
        std::thread::spawn(move || {
            let marker = format!("GamePKG presented {unit}");
            read_game_output(stdout, &output_log, marker.as_bytes(), || {
                presented.store(true, Ordering::Release);
                let _ = ready.try_send(());
            });
        });
        drain_local_log(stderr, log.clone());
        Ok(Self {
            child: Some(child),
            directory: Some(directory),
            log,
            transport: None,
            clock: None,
            launcher: Some(launcher),
            history: Some(history),
            game_frame: Some(game_frame),
            permit: Some(permit),
        })
    }

    pub fn first_frame_presented(&self) -> bool {
        self.game_frame
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
    }
}

fn record_history(mut history: crate::progress::Session, ready: mpsc::Receiver<()>) -> Result<()> {
    if ready.recv().is_err() {
        return Ok(());
    }
    history.start();
    // Preserve elapsed time across a launcher crash, with all disk I/O off
    // both the library event loop and the game window thread.
    loop {
        match ready.recv_timeout(Duration::from_secs(10)) {
            Ok(()) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => history.tick(&[])?,
            Err(mpsc::RecvTimeoutError::Disconnected) => return history.finish(),
        }
    }
}

fn read_game_output(
    mut pipe: impl Read,
    log: &Arc<Mutex<VecDeque<u8>>>,
    marker: &[u8],
    notify: impl FnOnce(),
) {
    let mut notify = Some(notify);
    let mut position = Some(0);
    let mut buffer = [0; 4096];
    loop {
        let count = match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        tail(log, &buffer[..count]);
        if notify.is_none() {
            continue;
        }
        for &byte in &buffer[..count] {
            if byte == b'\n' {
                if position == Some(marker.len()) {
                    notify.take().unwrap()();
                    break;
                }
                position = Some(0);
            } else {
                position = position
                    .filter(|&index| marker.get(index) == Some(&byte))
                    .map(|index| index + 1);
            }
        }
    }
}
fn drain_local_log(mut pipe: impl Read + Send + 'static, log: Arc<Mutex<VecDeque<u8>>>) {
    std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        while let Ok(count) = pipe.read(&mut buffer) {
            if count == 0 {
                break;
            }
            tail(&log, &buffer[..count]);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_frame_signal_requires_a_complete_exact_line() {
        let log = Arc::new(Mutex::new(VecDeque::new()));
        let mut data = vec![b'x'; 8191];
        data.extend(b"GamePKG presented expected\nGamePKG presented another\n");
        let mut signaled = false;
        read_game_output(&data[..], &log, b"GamePKG presented expected", || {
            signaled = true
        });
        assert!(!signaled);
        data.extend(b"GamePKG presented expected\n");
        read_game_output(&data[..], &log, b"GamePKG presented expected", || {
            signaled = true
        });
        assert!(signaled);
    }

    #[test]
    fn native_history_ignores_startup_exit_and_flushes_presented_play() {
        let library = tempfile::tempdir().unwrap();
        let root = library.path();
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        record_history(
            crate::progress::Session::open(root, "game", "creator").unwrap(),
            receiver,
        )
        .unwrap();
        assert_eq!(
            crate::progress::load(root, "game", "creator")
                .unwrap()
                .played_ms,
            0
        );

        let (sender, receiver) = mpsc::channel();
        let session = crate::progress::Session::open(root, "game", "creator").unwrap();
        let worker = std::thread::spawn(move || record_history(session, receiver));
        sender.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        drop(sender);
        worker.join().unwrap().unwrap();
        assert!(
            crate::progress::load(root, "game", "creator")
                .unwrap()
                .played_ms
                > 0
        );
        assert_eq!(
            crate::progress::load(root, "game", "another")
                .unwrap()
                .played_ms,
            0
        );
    }
}
