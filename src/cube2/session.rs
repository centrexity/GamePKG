//! Reviewed engine process owner with bounded pipes, deadlines and isolated data.
use super::protocol;
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, RawFd},
        unix::fs::PermissionsExt,
    },
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Stdio},
    time::{Duration, Instant},
};

pub struct Session {
    child: Child,
    _parent: std::sync::mpsc::Sender<()>,
    _binary: tempfile::TempDir,
    input: ChildStdin,
    output: ChildStdout,
    deadline: Duration,
    stopped: bool,
    exit_save: Option<Vec<u8>>,
    events: Vec<Vec<u8>>,
    _network: super::network::Worker,
    diagnostics: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    diagnostics_done: std::sync::mpsc::Receiver<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}
fn wait(fd: RawFd, events: i16, deadline: Instant) -> io::Result<()> {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Cube 2 frame deadline exceeded",
            ));
        }
        let mut descriptor = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let ready = unsafe {
            libc::poll(
                &mut descriptor,
                1,
                left.as_millis().clamp(1, i32::MAX as u128) as i32,
            )
        };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if ready == 0 {
            continue;
        }
        if descriptor.revents & libc::POLLNVAL != 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Cube 2 pipe closed",
            ));
        }
        return Ok(());
    }
}
struct Reader<'a> {
    pipe: &'a mut ChildStdout,
    deadline: Instant,
}
impl Read for Reader<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        loop {
            wait(self.pipe.as_raw_fd(), libc::POLLIN, self.deadline)?;
            match self.pipe.read(out) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue
                }
                result => return result,
            }
        }
    }
}
// Normal launch follows upstream startup and saved preferences. Only explicit
// host-approved multiplayer choices add commands; never choose a map or HUD.
fn startup_script(commands: &str) -> Option<String> {
    let commands = commands.trim().trim_start_matches(';').trim();
    (!commands.is_empty()).then(|| format!("-x{commands}"))
}
impl Session {
    /// Compile the embedded kit and start it with only selected game data visible.
    /// Developer entry point; installed playback uses authenticated cached builds.
    pub fn compile_and_start(data: &Path, memory: u64, deadline: Duration) -> Result<Self> {
        Self::compile_and_start_with_save(data, memory, deadline, &[])
    }
    pub fn compile_and_start_with_save(
        data: &Path,
        memory: u64,
        deadline: Duration,
        saved: &[u8],
    ) -> Result<Self> {
        Self::start(
            data,
            memory,
            deadline,
            saved,
            super::build::compile,
            None,
            String::new(),
        )
    }
    pub(super) fn start_cached(
        data: &Path,
        memory: u64,
        deadline: Duration,
        saved: &[u8],
        cached: &Path,
        permissions: &super::network::Config,
        commands: String,
    ) -> Result<Self> {
        Self::start(
            data,
            memory,
            deadline,
            saved,
            || {
                let bytes = crate::files::read_regular(cached, 32 * 1048576, false)
                    .context("Build Cube 2 for this computer before playing")?;
                crate::native::validate_executable(&bytes)?;
                crate::integrity::verify_build(cached, &bytes)?;
                Ok(bytes)
            },
            Some(permissions.clone()),
            commands,
        )
    }
    fn start(
        data: &Path,
        memory: u64,
        deadline: Duration,
        saved: &[u8],
        compile: impl FnOnce() -> Result<Vec<u8>>,
        permissions: Option<super::network::Config>,
        commands: String,
    ) -> Result<Self> {
        let seed = protocol::save_seed(saved)?;
        ensure!(
            (64 * 1048576..=4 * 1024 * 1048576).contains(&memory),
            "invalid Cube 2 memory ceiling"
        );
        ensure!(
            !deadline.is_zero() && deadline <= Duration::from_secs(60),
            "invalid Cube 2 frame deadline"
        );
        let directories = super::data::Directories::open(data)?;
        let bytes = compile()?;
        let permissions = permissions.unwrap_or_else(|| {
            super::network::Config::from(crate::manifest::Permissions {
                input: false,
                audio: false,
                save: false,
                network: "none".into(),
                network_hosts: Vec::new(),
                network_listen: Vec::new(),
                network_allow_local: false,
            })
        });
        let (network, socket) = super::network::start(permissions)?;
        use std::os::fd::FromRawFd;
        let fd = unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
        ensure!(fd >= 0, "cannot pin Cube 2 network channel");
        let network_descriptor = unsafe { fs::File::from_raw_fd(fd) };
        drop(socket);
        let binary = tempfile::tempdir()?;
        let path = binary.path().join("engine");
        fs::write(&path, bytes)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o500))?;
        let mut command = std::process::Command::new("/usr/bin/prlimit");
        command.env_clear().arg(format!("--as={memory}")).args([
            "--core=0",
            "--fsize=16777216",
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
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--remount-ro",
            "/dev",
            "--size",
            "16777216",
            "--tmpfs",
            "/tmp",
            "--size",
            "67108864",
            "--tmpfs",
            "/save",
            "--dir",
            "/game",
        ]);
        command
            .arg("--ro-bind-fd")
            .arg(directories.data.as_raw_fd().to_string())
            .arg("/game/data")
            .arg("--ro-bind-fd")
            .arg(directories.packages.as_raw_fd().to_string())
            .arg("/game/packages")
            .arg("--ro-bind")
            .arg(&path)
            .arg("/engine")
            .args([
                "--remount-ro",
                "/",
                "--chdir",
                "/game",
                "--setenv",
                "PATH",
                "/usr/bin",
                "--setenv",
                "HOME",
                "/save",
                "--setenv",
                "SDL_VIDEODRIVER",
                "offscreen",
                "--setenv",
                "SDL_AUDIODRIVER",
                "dummy",
                "--setenv",
                "LIBGL_ALWAYS_SOFTWARE",
                "1",
                "--setenv",
                "GALLIUM_DRIVER",
                "llvmpipe",
                "--setenv",
                "LP_NUM_THREADS",
                "2",
                "--",
                "/engine",
                "-w640",
                "-h480",
                "-t0",
                "-q/save",
            ])
            .args(startup_script(&commands))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        use std::os::unix::process::CommandExt;
        // Clear CLOEXEC only in the forked launcher, never in the GUI process.
        // Bubblewrap consumes these directory capabilities rather than resolving
        // a pathname that may have been replaced during compilation.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(network_descriptor.as_raw_fd(), 3) < 0 {
                    return Err(io::Error::last_os_error());
                }
                for fd in [
                    directories.data.as_raw_fd(),
                    directories.packages.as_raw_fd(),
                ] {
                    let flags = libc::fcntl(fd, libc::F_GETFD);
                    if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        // Keep the spawning thread alive across asynchronous handoff to the GUI.
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let (parent, lifetime) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(command.spawn());
            let _ = lifetime.recv();
        });
        let mut child = receive.recv().context("Cube 2 launcher thread stopped")??;
        let input = child.stdin.take().expect("piped stdin");
        let output = child.stdout.take().expect("piped stdout");
        for fd in [input.as_raw_fd(), output.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                let error = io::Error::last_os_error();
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        let mut stderr = child.stderr.take().expect("piped diagnostics");
        let diagnostics = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = diagnostics.clone();
        let (done, diagnostics_done) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            loop {
                match stderr.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let Ok(mut bytes) = sink.lock() else {
                            break;
                        };
                        if bytes.len() + n > 16384 {
                            let remove = bytes.len() + n - 16384;
                            bytes.drain(..remove);
                        }
                        bytes.extend_from_slice(&buffer[..n]);
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            let _ = done.send(());
        });
        let mut session = Self {
            child,
            _parent: parent,
            _binary: binary,
            input,
            output,
            deadline,
            stopped: false,
            diagnostics,
            diagnostics_done,
            exit_save: None,
            events: Vec::new(),
            _network: network,
        };
        session.send(&seed, Instant::now() + Duration::from_secs(60))?;
        Ok(session)
    }
    pub fn tick(&mut self, buttons: u32, dx: i32, dy: i32) -> Result<protocol::Frame> {
        self.tick_update(buttons, dx, dy)?
            .context("Cube 2 closed normally")
    }
    pub fn tick_update(
        &mut self,
        buttons: u32,
        dx: i32,
        dy: i32,
    ) -> Result<Option<protocol::Frame>> {
        ensure!(!self.stopped, "Cube 2 session has stopped");
        let packet = protocol::input(buttons, dx, dy)?;
        let deadline = Instant::now() + self.deadline;
        let mut loading = false;
        let result = (|| {
            for event in std::mem::take(&mut self.events) {
                self.send(&event, deadline)?;
            }
            self.send(&packet, deadline)?;
            let update = protocol::read_update(Reader {
                pipe: &mut self.output,
                deadline,
            })?;
            if matches!(update, protocol::Update::Loading) {
                loading = true;
                // One fixed allowance per tick; repeated markers cannot renew it.
                let update = protocol::read_update(Reader {
                    pipe: &mut self.output,
                    deadline: Instant::now() + Duration::from_secs(60),
                })?;
                ensure!(
                    !matches!(update, protocol::Update::Loading),
                    "Duplicate Cube 2 loading marker"
                );
                Ok(update)
            } else {
                Ok(update)
            }
        })();
        match self.complete(result, if loading { "load" } else { "render" })? {
            protocol::Update::Loading => anyhow::bail!("Unexpected Cube 2 loading marker"),
            protocol::Update::Frame(frame) => Ok(Some(frame)),
            protocol::Update::Quit(saved) => {
                self.exit_save = Some(saved);
                self.close()?;
                Ok(None)
            }
        }
    }
    pub(super) fn queue(&mut self, event: Vec<u8>) -> Result<()> {
        ensure!(
            self.events.len() < protocol::MAX_EVENTS,
            "Cube 2 input event queue exceeded its bound"
        );
        self.events.push(event);
        Ok(())
    }
    pub fn key(
        &mut self,
        scancode: u32,
        pressed: bool,
        repeat: bool,
        modifiers: u32,
    ) -> Result<()> {
        self.queue(protocol::key_event(scancode, pressed, repeat, modifiers)?)
    }
    pub fn text(&mut self, text: &str) -> Result<()> {
        self.queue(protocol::text_event(text)?)
    }
    pub fn reset_input(&mut self) {
        self.events.clear();
        self.events.push(protocol::RESET_INPUT.to_vec());
    }
    pub fn modifiers(&mut self, modifiers: u32) -> Result<()> {
        self.queue(protocol::modifiers_event(modifiers)?)
    }
    fn send(&mut self, packet: &[u8], deadline: Instant) -> Result<()> {
        let mut offset = 0;
        while offset < packet.len() {
            wait(self.input.as_raw_fd(), libc::POLLOUT, deadline)?;
            match self.input.write(&packet[offset..]) {
                Ok(0) => {
                    return Err(
                        io::Error::new(io::ErrorKind::BrokenPipe, "Cube 2 input closed").into(),
                    )
                }
                Ok(n) => offset += n,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
    pub fn set_deadline(&mut self, deadline: Duration) -> Result<()> {
        ensure!(
            !deadline.is_zero() && deadline <= Duration::from_secs(60),
            "invalid Cube 2 deadline"
        );
        self.deadline = deadline;
        Ok(())
    }
    pub fn snapshot(&mut self) -> Result<Vec<u8>> {
        if let Some(saved) = &self.exit_save {
            return Ok(saved.clone());
        }
        ensure!(!self.stopped, "Cube 2 session has stopped");
        let deadline = Instant::now() + Duration::from_secs(2);
        let result = (|| {
            self.send(&protocol::SAVE_REQUEST, deadline)?;
            protocol::read_save(Reader {
                pipe: &mut self.output,
                deadline,
            })
        })();
        self.complete(result, "save")
    }
    pub fn close(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        let result = (|| {
            if self.exit_save.is_none() {
                self.send(&protocol::CLOSE_REQUEST, deadline)?;
            }
            loop {
                if let Some(status) = self.child.try_wait()? {
                    self.stopped = true;
                    ensure!(status.success(), "Cube 2 exited unsuccessfully");
                    return Ok(());
                }
                ensure!(
                    Instant::now() < deadline,
                    "Cube 2 did not close within its deadline"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        })();
        self.complete(result, "close")
    }
    fn complete<T>(&mut self, result: Result<T>, operation: &str) -> Result<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.stop();
                let _ = self
                    .diagnostics_done
                    .recv_timeout(Duration::from_millis(50));
                let message = self
                    .diagnostics
                    .lock()
                    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                    .unwrap_or_default();
                if error.chain().any(|cause| {
                    cause
                        .downcast_ref::<io::Error>()
                        .is_some_and(|e| e.kind() == io::ErrorKind::TimedOut)
                }) {
                    let message = timeout_message(operation, self.deadline);
                    return Err(anyhow::anyhow!(message));
                }
                let lower = message.to_ascii_lowercase();
                if ["out of memory", "cannot allocate memory", "bad_alloc"]
                    .iter()
                    .any(|word| lower.contains(word))
                {
                    return Err(anyhow::anyhow!("The game ran out of its allowed memory. Increase Memory ceiling in Settings within the game's signed hard limit."));
                }
                let detail = message
                    .lines()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("");
                Err(error.context(format!(
                    "Cube 2 engine stopped: {}",
                    detail.chars().take(500).collect::<String>()
                )))
            }
        }
    }
    fn stop(&mut self) {
        if !self.stopped {
            self.stopped = true;
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn timeout_message(operation: &str, deadline: Duration) -> String {
    match operation {
        "load" => "The game exceeded its 60-second map-loading deadline.".to_owned(),
        "save" => "The game did not provide a save snapshot within its 2-second shutdown deadline. Existing saves were kept.".to_owned(),
        "close" => "The game did not close within its 2-second shutdown deadline.".to_owned(),
        _ => format!("The renderer exceeded your {} ms tick deadline. Increase Tick deadline in Settings (500 ms is recommended for software rendering).", deadline.as_millis()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_startup_keeps_original_menu_hud_and_fps_preferences() {
        assert_eq!(startup_script(""), None);
        assert_eq!(startup_script(" ; "), None);
        assert_eq!(
            startup_script("; connect \"127.0.0.1\" 28785"),
            Some("-xconnect \"127.0.0.1\" 28785".into())
        );
    }
    #[test]
    fn timeout_guidance_identifies_the_failed_operation() {
        let deadline = Duration::from_millis(100);
        assert!(timeout_message("render", deadline).contains("100 ms tick deadline"));
        assert!(timeout_message("save", deadline).contains("save snapshot"));
        assert!(timeout_message("save", deadline).contains("Existing saves were kept"));
        assert!(!timeout_message("save", deadline).contains("renderer"));
        assert!(timeout_message("close", deadline).contains("did not close"));
    }
    #[test]
    fn bad_limits_are_rejected_before_compilation() {
        for memory in [0, 1, u64::MAX] {
            assert!(Session::compile_and_start(
                Path::new("/nonexistent"),
                memory,
                Duration::from_secs(1)
            )
            .is_err());
        }
        for deadline in [Duration::ZERO, Duration::from_secs(61)] {
            assert!(
                Session::compile_and_start(Path::new("/nonexistent"), 104857600, deadline).is_err()
            );
        }
    }
    #[test]
    #[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
    fn two_original_engines_connect_through_the_host_broker() {
        two_engines(false);
    }
    #[test]
    #[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
    fn two_original_engines_join_through_the_discovered_server_list() {
        two_engines(true);
    }
    fn two_engines(discovery: bool) {
        use std::net::UdpSocket;
        let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
        let binary = super::super::build::compile().unwrap();
        let (port, first, second) = loop {
            let first = UdpSocket::bind("127.0.0.1:0").unwrap();
            let port = first.local_addr().unwrap().port();
            if port < 65535 {
                if let Ok(second) = UdpSocket::bind(("127.0.0.1", port + 1)) {
                    break (port, first, second);
                }
            }
        };
        let mut mock_master = None;
        let master_address = if discovery {
            use std::io::{Read, Write};
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            mock_master = Some(std::thread::spawn(move || {
                let until = Instant::now() + Duration::from_secs(30);
                loop {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            stream
                                .set_read_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            let mut request = [0; 5];
                            stream.read_exact(&mut request).unwrap();
                            assert_eq!(&request, b"list\n");
                            stream
                                .write_all(format!("addserver 127.0.0.1 {port}\n").as_bytes())
                                .unwrap();
                            return;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < until, "no host discovery request");
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => panic!("mock master: {e}"),
                    }
                }
            }));
            address.to_string()
        } else {
            String::new()
        };
        let selector =
            crate::manifest::Manifest::parse(include_bytes!("../../ports/cube2/manifest.toml"))
                .unwrap();
        let server_root = tempfile::tempdir().unwrap();
        let client_root = tempfile::tempdir().unwrap();
        crate::server_selections::remember(
            server_root.path(),
            &selector,
            &crate::server_selections::Selection {
                server: String::new(),
                master: String::new(),
                listen: format!("127.0.0.1:{port}"),
                allow_local: true,
            },
        )
        .unwrap();
        crate::server_selections::remember(
            client_root.path(),
            &selector,
            &crate::server_selections::Selection {
                server: if discovery {
                    String::new()
                } else {
                    format!("127.0.0.1:{port}")
                },
                master: master_address.clone(),
                listen: String::new(),
                allow_local: true,
            },
        )
        .unwrap();
        let (server_permissions, server_commands) =
            crate::server_selections::effective(server_root.path(), &selector).unwrap();
        let (client_permissions, client_commands) =
            crate::server_selections::effective(client_root.path(), &selector).unwrap();
        let seed = |config: String| {
            let mut bytes = b"GPS1".to_vec();
            bytes.extend((config.len() as u32).to_le_bytes());
            bytes.extend([0; 8]);
            bytes.extend(config.as_bytes());
            bytes
        };
        let server_seed = seed(
            "name NetServer\nbind F11 [name (getclientname (getclientnum NetClient))]\n".into(),
        );
        let client_seed = seed(format!(
            "name NetClient\nbind F11 [name (concatword Remote (isconnected 0 0))]\n{}",
            if discovery {
                format!("bind F10 [updatefrommaster; connect \"127.0.0.1\" {port}]\n")
            } else {
                String::new()
            }
        ));
        let client_network = super::super::network::Config {
            permissions: client_permissions,
            master: if discovery {
                Some(super::super::discovery::Master::resolve(&master_address, true).unwrap())
            } else {
                None
            },
        };
        drop(first);
        drop(second);
        let mut server = Session::start(
            Path::new(&data),
            2 * 1024 * 1048576,
            Duration::from_secs(60),
            &server_seed,
            || Ok(binary.clone()),
            Some(server_permissions.into()),
            format!("mode 0; map complex{server_commands}"),
        )
        .unwrap();
        server.tick(0, 0, 0).unwrap();
        let mut client = Session::start(
            Path::new(&data),
            2 * 1024 * 1048576,
            Duration::from_secs(60),
            &client_seed,
            || Ok(binary.clone()),
            Some(client_network),
            client_commands,
        )
        .unwrap();
        client.tick(0, 0, 0).unwrap();
        if let Some(master) = mock_master {
            master.join().unwrap();
            client.key(67, true, false, 0).unwrap();
            client.tick(0, 0, 0).unwrap();
            client.key(67, false, false, 0).unwrap();
            client.tick(0, 0, 0).unwrap();
            let saved = client.snapshot().unwrap();
            let files = protocol::save_files(&saved).unwrap();
            let servers = files.iter().find(|f| f.path == "servers.cfg").unwrap();
            assert!(
                std::str::from_utf8(servers.bytes)
                    .unwrap()
                    .contains(&format!("127.0.0.1 {port}")),
                "server did not appear in the original browser"
            );
        }
        let until = Instant::now() + Duration::from_secs(12);
        let mut connected = false;
        while Instant::now() < until {
            server.tick(0, 0, 0).unwrap();
            client.tick(1, 2, 0).unwrap();
            if server
                .diagnostics
                .lock()
                .unwrap()
                .windows(b"client connected".len())
                .any(|s| s == b"client connected")
            {
                connected = true;
                break;
            }
        }
        assert!(
            connected,
            "server diagnostics: {}",
            String::from_utf8_lossy(&server.diagnostics.lock().unwrap())
        );
        for _ in 0..60 {
            server.tick(0, 0, 0).unwrap();
            client.tick(1, 1, 0).unwrap();
        }
        // Stop polling either game as a UI would when minimized. Their worker
        // clocks must keep the real ENet connection alive beyond 30 seconds.
        server.set_deadline(Duration::from_millis(500)).unwrap();
        client.set_deadline(Duration::from_millis(500)).unwrap();
        let store = |root: &Path, id| super::super::save::Store::open(root, id, "test", 1).unwrap();
        let history = |root: &Path, id| crate::progress::Session::open(root, id, "test").unwrap();
        let mut server = super::super::runner::Worker::start(
            server,
            store(server_root.path(), "server"),
            Some(history(server_root.path(), "server")),
        )
        .unwrap();
        let mut client = super::super::runner::Worker::start(
            client,
            store(client_root.path(), "client"),
            Some(history(client_root.path(), "client")),
        )
        .unwrap();
        std::thread::sleep(Duration::from_secs(35));
        for worker in [&server, &client] {
            let output = worker.poll().unwrap();
            assert!(output.error.is_none(), "{:?}", output.error);
            assert!(output.ticks > 100);
            assert!(output.frame.is_some());
            assert!(output.audio.len() <= 4800 * output.audio_channels as usize);
            assert!(!output.exited);
        }
        server
            .submit(
                0,
                0,
                0,
                vec![protocol::key_event(68, true, false, 0).unwrap()],
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        client
            .submit(
                0,
                0,
                0,
                vec![protocol::key_event(68, true, false, 0).unwrap()],
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        client.finish().unwrap();
        server.finish().unwrap();
        let config = |snapshot: Vec<u8>| {
            super::protocol::save_files(&snapshot)
                .unwrap()
                .iter()
                .find(|f| f.path == "config.cfg")
                .unwrap()
                .bytes
                .to_vec()
        };
        let server_config = config(store(server_root.path(), "server").seed);
        let client_config = config(store(client_root.path(), "client").seed);
        assert!(
            server_config
                .windows(b"name \"NetClient\"".len())
                .any(|s| s == b"name \"NetClient\""),
            "server did not receive the client's game identity"
        );
        assert!(
            client_config
                .windows(b"name \"Remote1\"".len())
                .any(|s| s == b"name \"Remote1\""),
            "client did not establish a remote game connection"
        );
        for (root, id) in [
            (server_root.path(), "server"),
            (client_root.path(), "client"),
        ] {
            let elapsed = crate::progress::load(root, id, "test").unwrap().played_ms;
            assert!(
                (33000..45000).contains(&elapsed),
                "background play clock: {elapsed}"
            );
        }
    }
    #[test]
    #[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
    fn original_server_browser_reads_only_the_host_master_snapshot() {
        use std::io::{Read, Write};
        let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original game data");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, done) = std::sync::mpsc::channel();
        let master = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(60);
            loop {
                if done.try_recv().is_ok() || Instant::now() > until {
                    return false;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut request = [0; 5];
                        stream.read_exact(&mut request).unwrap();
                        assert_eq!(&request, b"list\n");
                        let mut response=b"echo NOT_A_SCRIPT\naddserver 127.0.0.1 32711\naddserver unapproved.invalid 32712\naddserver 127.0.0.1 32713;quit\n".to_vec();
                        // Exceeds both the first 4 KiB and doubled 8 KiB
                        // browser buffers, exercising bounded partial reads.
                        for port in 32000..32511 {
                            response.extend_from_slice(
                                format!("addserver 127.0.0.1 {port}\n").as_bytes(),
                            );
                        }
                        stream.write_all(&response).unwrap();
                        return true;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("mock master: {e}"),
                }
            }
        });
        let config = b"exec data/defaults.cfg\nbind F10 [updatefrommaster]\n";
        let mut seed = b"GPS1".to_vec();
        for n in [config.len() as u32, 0, 0] {
            seed.extend(n.to_le_bytes());
        }
        seed.extend(config);
        let p = crate::manifest::Permissions {
            input: true,
            audio: false,
            save: true,
            network: "multiplayer".into(),
            network_hosts: Vec::new(),
            network_listen: Vec::new(),
            network_allow_local: true,
        };
        let network = super::super::network::Config {
            permissions: p,
            master: Some(
                super::super::discovery::Master::resolve(&address.to_string(), true).unwrap(),
            ),
        };
        let mut session = Session::start(
            Path::new(&data),
            2 * 1024 * 1048576,
            Duration::from_secs(60),
            &seed,
            super::super::build::compile,
            Some(network),
            format!("; mastername \"127.0.0.1\"; masterport {}", address.port()),
        )
        .unwrap();
        session.tick(0, 0, 0).unwrap();
        assert!(
            master.join().unwrap(),
            "engine never caused the host to request a master list"
        );
        let _ = stop.send(());
        session.set_deadline(Duration::from_millis(500)).unwrap();
        // The cache is collected by the next RPC; retrieval remains inside the
        // ordinary tick deadline, including the original synchronous browser.
        session.key(67, true, false, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        session.key(67, false, false, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        let saved = session.snapshot().unwrap();
        let files = protocol::save_files(&saved).unwrap();
        let servers = &files
            .iter()
            .find(|file| file.path == "servers.cfg")
            .expect("server list was not saved")
            .bytes;
        let servers = std::str::from_utf8(servers).unwrap();
        assert!(servers.contains("127.0.0.1 32711"), "{servers}");
        assert_eq!(
            servers.matches("addserver ").count(),
            512,
            "large master snapshot was truncated"
        );
        assert!(
            !servers.contains("unapproved.invalid")
                && !servers.contains("32713")
                && !servers.contains("NOT_A_SCRIPT"),
            "{servers}"
        );
        drop(session);
    }
    #[test]
    #[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
    fn original_game_runs_through_the_rust_session() {
        let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original game data");
        let original = fs::read(Path::new(&data).join("data/stdlib.cfg")).unwrap();
        let config = b"exec data/defaults.cfg\nname \"GamePKGTest\"\nbind T [saycommand]\neditbind T [saycommand]\nbind F10 [disconnect; newmap 10; if (= $editing 0) [edittoggle]; maptitle Persisted; newent light 128 255 255 255; savemap GamePKGPersist]\nbind F11 [map GamePKGPersist; name $maptitle]\neditbind F11 [map GamePKGPersist; name $maptitle]\nbind F12 [quit]\neditbind F12 [quit]\neditbind MOUSE3 [name Middle]\neditbind MOUSE6 [name SideOne; onrelease [name Released]]\neditbind MOUSE7 [name SideTwo]\npointer_count = 0\neditbind MOUSE4 [pointer_count = (+ $pointer_count 1); name (concatword Wheel $pointer_count)]\neditbind MOUSE5 [name WheelDown]\neditbind MOUSE8 [name WheelRight]\neditbind MOUSE9 [name WheelLeft]\n";
        let mut seed = b"GPS1".to_vec();
        for length in [config.len() as u32, 0, 0] {
            seed.extend(length.to_le_bytes());
        }
        seed.extend(config);
        let mut session = Session::compile_and_start_with_save(
            Path::new(&data),
            std::env::var("GAMEPKG_CUBE2_TEST_MEMORY_MIB")
                .ok()
                .map(|v| v.parse::<u64>().unwrap())
                .unwrap_or(2048)
                * 1048576,
            Duration::from_secs(60),
            &seed,
        )
        .unwrap();
        // Gameplay fixture explicitly chooses its map through original input.
        session.tick(0, 0, 0).unwrap();
        session.key(23, true, false, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        session.key(23, false, false, 0).unwrap();
        session.text("/mode 0; map complex; showgui \"\"").unwrap();
        session.tick(0, 0, 0).unwrap();
        session.tick(1 << 6, 0, 0).unwrap();
        let first = session.tick(0, 0, 0).unwrap();
        assert_eq!((first.width, first.height), (640, 480));
        for (width, height) in [(960, 540), (800, 600), (640, 480)] {
            session
                .queue(protocol::resize_event(width, height).unwrap())
                .unwrap();
            let resized = session.tick(0, 0, 0).unwrap();
            assert_eq!((resized.width, resized.height), (width, height));
        }

        let mut moved = session.tick(1, 80, 0).unwrap();
        let mut sound = moved.audio.iter().any(|s| *s != 0);
        for _ in 0..12 {
            moved = session.tick(1 | 1 << 16, 15, 0).unwrap();
            sound |= moved.audio.iter().any(|s| *s != 0);
        }
        assert_ne!(
            crate::verify::digest(&first.rgba),
            crate::verify::digest(&moved.rgba)
        );
        assert!(sound, "Cube 2 mixer produced no sound during firing");
        session.key(67, true, false, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        session.key(67, false, false, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        let snapshot = session.snapshot().unwrap();
        let files = protocol::save_files(&snapshot).unwrap();
        let map = files
            .iter()
            .find(|file| file.path == "packages/base/GamePKGPersist.ogz")
            .expect("edited map was not saved");
        assert!(map.bytes.len() > 64);
        let map_bytes = map.bytes.to_vec();
        assert!(
            !Path::new(&data)
                .join("packages/base/GamePKGPersist.ogz")
                .exists(),
            "original data was modified"
        );
        assert!(snapshot
            .windows(b"GamePKGTest".len())
            .any(|s| s == b"GamePKGTest"));
        protocol::save_seed(&snapshot).unwrap();
        let library = tempfile::tempdir().unwrap();
        let mut save =
            super::super::save::Store::open(library.path(), "com.gamepkg.cube2", "test-creator", 1)
                .unwrap();
        save.commit(&snapshot).unwrap();
        assert_eq!(
            super::super::save::Store::open(library.path(), "com.gamepkg.cube2", "test-creator", 1)
                .unwrap()
                .seed,
            snapshot
        );
        assert!(super::super::save::Store::open(
            library.path(),
            "com.gamepkg.cube2",
            "other-creator",
            1
        )
        .unwrap()
        .seed
        .is_empty());
        assert_eq!(
            fs::read(Path::new(&data).join("data/stdlib.cfg")).unwrap(),
            original
        );
        // Exercise real text input through the original console, then a
        // function-key binding to the same quit() handler used by its menu.
        let assert_name = |session: &mut Session, expected: &str| {
            let snapshot = session.snapshot().unwrap();
            let files = protocol::save_files(&snapshot).unwrap();
            let config = files.iter().find(|f| f.path == "config.cfg").unwrap().bytes;
            let pattern = format!("name \"{expected}\"");
            assert!(
                config
                    .windows(pattern.len())
                    .any(|s| s == pattern.as_bytes()),
                "pointer binding did not set {expected}"
            );
        };
        for (button, name) in [(2, "Middle"), (5, "SideTwo")] {
            session
                .queue(protocol::mouse_button_event(button, true, 0).unwrap())
                .unwrap();
            session
                .queue(protocol::mouse_button_event(button, false, 0).unwrap())
                .unwrap();
            session.tick(0, 0, 0).unwrap();
            assert_name(&mut session, name);
        }
        session
            .queue(protocol::mouse_button_event(4, true, 0).unwrap())
            .unwrap();
        session.tick(0, 0, 0).unwrap();
        assert_name(&mut session, "SideOne");
        session.reset_input();
        session.tick(0, 0, 0).unwrap();
        assert_name(&mut session, "Released");
        for (x, y, name) in [
            (0, 3, "Wheel3"),
            (0, -1, "WheelDown"),
            (1, 0, "WheelRight"),
            (-1, 0, "WheelLeft"),
        ] {
            session
                .queue(protocol::wheel_event(x, y, 0).unwrap())
                .unwrap();
            session.tick(0, 0, 0).unwrap();
            assert_name(&mut session, name);
        }
        session.key(23, true, false, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        session.key(23, false, false, 0).unwrap();
        session.text("/name GamePKGTyped").unwrap();
        session.tick(0, 0, 0).unwrap();
        session.tick(1 << 6, 0, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        session.key(69, true, false, 0).unwrap();
        assert!(session.tick_update(0, 0, 0).unwrap().is_none());
        let final_save = session.snapshot().unwrap();
        assert!(final_save.windows(12).any(|s| s == b"GamePKGTyped"));
        assert!(session.stopped);
        session.close().unwrap();
        let pid = session.child.id();
        let executable = fs::read(session._binary.path().join("engine")).unwrap();
        drop(session);
        assert_eq!(
            unsafe { libc::kill(pid as i32, 0) },
            -1,
            "worker survived session drop"
        );
        let mut restored = Session::start(
            Path::new(&data),
            2 * 1024 * 1048576,
            Duration::from_secs(60),
            &final_save,
            || Ok(executable),
            None,
            String::new(),
        )
        .unwrap();
        restored.tick(0, 0, 0).unwrap();
        restored.key(68, true, false, 0).unwrap();
        restored.tick(0, 0, 0).unwrap();
        restored.key(68, false, false, 0).unwrap();
        restored.tick(0, 0, 0).unwrap();
        let restored_save = restored.snapshot().unwrap();
        let files = protocol::save_files(&restored_save).unwrap();
        assert_eq!(
            files
                .iter()
                .find(|f| f.path == "packages/base/GamePKGPersist.ogz")
                .unwrap()
                .bytes,
            map_bytes
        );
        let config = files.iter().find(|f| f.path == "config.cfg").unwrap().bytes;
        assert!(
            config
                .windows(b"name \"Persisted\"".len())
                .any(|s| s == b"name \"Persisted\""),
            "edited map title was not restored: {}",
            String::from_utf8_lossy(config)
        );
        // An engine failure during a UI pause must reap the process, preserve
        // the last good archive, and stop the play clock at failure time.
        restored.set_deadline(Duration::from_millis(500)).unwrap();
        let failed_root = tempfile::tempdir().unwrap();
        let mut store =
            super::super::save::Store::open(failed_root.path(), "failed", "test", 1).unwrap();
        store.commit(&restored_save).unwrap();
        let progress =
            crate::progress::Session::open(failed_root.path(), "failed", "test").unwrap();
        let pid = restored.child.id();
        let mut worker =
            super::super::runner::Worker::start(restored, store, Some(progress)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while worker.poll().unwrap().ticks == 0 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGKILL) }, 0);
        let deadline = Instant::now() + Duration::from_secs(2);
        while worker.poll().unwrap().error.is_none() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(1100));
        assert!(worker.finish().is_err());
        assert_eq!(
            super::super::save::Store::open(failed_root.path(), "failed", "test", 1)
                .unwrap()
                .seed,
            restored_save
        );
        assert!(
            crate::progress::load(failed_root.path(), "failed", "test")
                .unwrap()
                .played_ms
                < 1000
        );
        assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
    }
}
