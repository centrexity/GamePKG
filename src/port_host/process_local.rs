//! Linux native executable launch. Runtime services stay in the executable.
use super::{
    history::{drain_local_log, read_game_output, record_history, HistoryEvent},
    reaper, Artifact, Bootstrap, NativeLaunch, Permit, Process, Target,
};
use anyhow::{ensure, Context, Result};
use std::{
    collections::VecDeque,
    fs, io,
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
};

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
        if let Some(runtime) = runtime {
            for endpoint in ["pipewire-0", "pulse/native"] {
                let path = runtime.join(endpoint);
                if path.exists() {
                    socket(command, &path, &inside.join(endpoint))?;
                }
            }
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
    /// namespace. The runtime receives read-only ROMs and a
    /// capability mount for an approved private save scope.
    pub fn start_in_process(
        artifact: &Artifact,
        library: &Path,
        rom: Option<fs::File>,
        save: Option<(&Path, u64)>,
        options: NativeLaunch,
    ) -> Result<Self> {
        Self::start_in_process_with_assets(
            artifact,
            library,
            rom.into_iter().collect(),
            save,
            None,
            options,
        )
    }

    pub fn start_in_process_with_assets(
        artifact: &Artifact,
        library: &Path,
        roms: Vec<fs::File>,
        save: Option<(&Path, u64)>,
        assets: Option<&crate::port_host::owner::VerifiedAssets>,
        options: NativeLaunch,
    ) -> Result<Self> {
        Self::start_in_process_with_data(artifact, library, roms, save, assets, vec![], options)
    }

    pub(crate) fn start_in_process_with_data(
        artifact: &Artifact,
        library: &Path,
        roms: Vec<fs::File>,
        save: Option<(&Path, u64)>,
        assets: Option<&crate::port_host::owner::VerifiedAssets>,
        game_data: Vec<cap_std::fs::Dir>,
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
            manifest.required_rom_count() == roms.len(),
            "wrong number of selected ROMs"
        );
        ensure!(
            manifest.required_game_data_directory_count() == game_data.len(),
            "wrong number of selected game-data directories"
        );
        for rom in &roms {
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
        let previous_save = if let Some((root, _)) = save {
            match crate::saves::previous_blob(root, manifest) {
                Ok(Some(blob)) => {
                    let snapshot = directory.path().join("previous-save");
                    fs::create_dir(&snapshot)?;
                    fs::write(snapshot.join("legacy-save.blob"), blob)?;
                    Some(snapshot)
                }
                Ok(None) => None,
                Err(error) => {
                    eprintln!("GamePKG could not read the previous save: {error:#}");
                    None
                }
            }
        } else {
            None
        };
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
        if manifest.permissions.audio && !options.mute_audio {
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
        desktop(
            &mut command,
            manifest.permissions.audio && !options.mute_audio,
        )?;
        if let Some(assets) = assets {
            command
                .arg("--ro-bind")
                .arg(assets.verified_directory(&artifact.receipt().game.source_kit)?)
                .arg("/run/gamepkg-assets")
                .args(["--setenv", "GAMEPKG_ASSET_GRANT", "1"]);
            if let Some(audio) = assets.recorded_audio() {
                command
                    .args(["--ro-bind"])
                    .arg(audio)
                    .arg("/run/gamepkg-audio");
            }
        }
        let mut inherited_mounts = Vec::new();
        for (index, rom) in roms.iter().enumerate() {
            let fd = rom.as_raw_fd();
            command
                .arg("--ro-bind-fd")
                .arg(fd.to_string())
                .arg(format!("/run/gamepkg-rom/{index}"));
            inherited_mounts.push(fd);
        }
        for (index, directory) in game_data.iter().enumerate() {
            let fd = directory.as_raw_fd();
            command
                .arg("--ro-bind-fd")
                .arg(fd.to_string())
                .arg(format!("/run/gamepkg-data/{index}"));
            inherited_mounts.push(fd);
        }
        if let Some((scope, quota)) = &save_mount {
            let fd = scope.as_raw_fd();
            command
                .arg("--bind-fd")
                .arg(fd.to_string())
                .arg("/run/gamepkg-save")
                .args(["--setenv", "GAMEPKG_SAVE_QUOTA"])
                .arg(quota.to_string());
            inherited_mounts.push(fd);
        }
        if let Some(snapshot) = previous_save {
            command
                .arg("--ro-bind")
                .arg(snapshot)
                .arg("/run/gamepkg-previous-save");
        }
        let file_picker =
            manifest.native_tick && options.visible && !manifest.permissions.files.is_empty();
        let (game_input, parent_picker) = if file_picker {
            use std::os::fd::OwnedFd;
            let (parent, child) = std::os::unix::net::UnixStream::pair()?;
            command.args(["--setenv", "GAMEPKG_FILE_PICKER", "1"]);
            (Stdio::from(OwnedFd::from(child)), Some(parent))
        } else {
            (Stdio::null(), None)
        };
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
            .stdin(game_input)
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
                for &fd in &inherited_mounts {
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
        let achievements = manifest
            .achievements
            .iter()
            .map(|achievement| achievement.id.clone())
            .collect();
        let history = std::thread::Builder::new()
            .name("gamepkg-play-history".into())
            .spawn(move || record_history(history, readiness, achievements))?;
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
        drop(roms);
        drop(game_data);
        drop(save_mount);
        if let Some(socket) = parent_picker {
            let manifest = manifest.clone();
            std::thread::Builder::new()
                .name("gamepkg-launcher-picker".into())
                .spawn(move || {
                    if let Err(error) = crate::port_host::tick_picker::serve(socket, manifest) {
                        if error.kind() != io::ErrorKind::BrokenPipe {
                            eprintln!("GamePKG file picker: {error}");
                        }
                    }
                })?;
        }
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
            read_game_output(stdout, &output_log, marker.as_bytes(), |event| {
                if event == HistoryEvent::Presented {
                    presented.store(true, Ordering::Release);
                }
                let _ = ready.send(event);
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
}
