use anyhow::{bail, ensure, Context, Result};
use std::{
    io::{Read, Write},
    os::fd::{AsRawFd, RawFd},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Stdio},
    time::{Duration, Instant},
};
const PACKET: usize = 8 * 1048576 + 65536;
fn validate_engine_save(manifest: &crate::manifest::Manifest, length: usize) -> Result<()> {
    ensure!(
        manifest.builtin_engine.as_deref() == Some(crate::doom::PROFILE)
            && manifest.permissions.save,
        "engine save access denied"
    );
    ensure!(length <= crate::doom::SAVE_MAX, "engine save too large");
    Ok(())
}
fn stereo_engine_audio(
    manifest: &crate::manifest::Manifest,
    channels: u32,
    queued: usize,
    payload: &[u8],
) -> Result<Vec<i16>> {
    ensure!(
        matches!(
            manifest.builtin_engine.as_deref(),
            Some(crate::zelda3::PROFILE | crate::doom::PROFILE)
        ) && manifest.permissions.audio,
        "stereo engine audio access denied"
    );
    ensure!(
        payload.len() <= 6400 && payload.len().is_multiple_of(4),
        "invalid stereo engine audio packet"
    );
    ensure!(
        (queued == 0 || (channels == 2 && queued.is_multiple_of(2)))
            && queued
                .checked_add(payload.len() / 2)
                .is_some_and(|size| size <= 3200),
        "stereo audio budget exceeded"
    );
    Ok(payload
        .as_chunks::<2>()
        .0
        .iter()
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
        .collect())
}

pub(crate) struct Session {
    pub(crate) window: crate::window_control::Service,
    pub(crate) user_files: crate::user_files::Service,
    input_events: crate::input_events::Queue,
    child: Child,
    _parent_thread: std::sync::mpsc::Sender<()>,
    input: ChildStdin,
    output: ChildStdout,
    _binary: tempfile::TempDir,
    package: crate::archive::Package,
    manifest: crate::manifest::Manifest,
    roms: Vec<Vec<u8>>,
    save: Option<crate::saves::Save>,
    engine_data: Option<Vec<u8>>,
    pub frame: crate::drawing::Frame,
    pub audio: Vec<i16>,
    pub audio_channels: u32,
    pub achievements: Vec<String>,
    pub diagnostic: Option<(u32, u32, u32)>,
    pub exited: bool,
    network: crate::network::Service,
    shader_count: usize,
    deadline: Duration,
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
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
impl Session {
    pub fn new(
        root: &Path,
        digest: &str,
        package: crate::archive::Package,
        manifest: crate::manifest::Manifest,
        roms: Vec<Vec<u8>>,
        settings: &crate::settings::Settings,
        shader_count: usize,
    ) -> Result<Self> {
        let path = super::path_for_digest(root, digest);
        let bytes = crate::files::read_regular(&path, 64 * 1048576, false)
            .context("Build this game for this computer before playing")?;
        super::validate_executable(&bytes)?;
        crate::integrity::verify_build(&path, &bytes)?;
        let memory = manifest
            .limits
            .hard_memory_bytes
            .min(settings.memory_ceiling_mib * 1048576);
        if let Some(profile) = manifest.builtin_engine.as_deref() {
            ensure!(
                memory >= crate::builtin::memory(profile)?,
                "memory ceiling is too low for this engine"
            );
        }
        let save = if manifest.permissions.save {
            Some(crate::saves::Save::open_for_game(root, &manifest)?)
        } else {
            None
        };
        let binary = tempfile::tempdir()?;
        std::fs::write(binary.path().join("game"), bytes)?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            binary.path().join("game"),
            std::fs::Permissions::from_mode(0o500),
        )?;
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
        for p in ["/lib", "/lib64"] {
            if Path::new(p).exists() {
                command.args(["--ro-bind", p, p]);
            }
        }
        command
            .arg("--ro-bind")
            .arg(binary.path())
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
            .stderr(Stdio::null());
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
            .context("native playback requires bubblewrap and prlimit; isolation is mandatory")?;
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        for fd in [input.as_raw_fd(), output.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err(std::io::Error::last_os_error().into());
            }
        }
        let mut session = Self {
            child,
            _parent_thread: parent_thread,
            input,
            output,
            _binary: binary,
            package,
            network: crate::network::Service::new(manifest.permissions.clone()),
            input_events: crate::input_events::Queue::new(manifest.permissions.input),
            window: crate::window_control::Service::new(manifest.permissions.input),
            user_files: crate::user_files::Service::new(manifest.permissions.files.clone()),
            manifest,
            roms,
            save,
            engine_data: None,
            frame: Default::default(),
            audio: Vec::new(),
            audio_channels: 1,
            achievements: Vec::new(),
            diagnostic: None,
            exited: false,
            shader_count,
            deadline: Duration::from_millis(settings.tick_deadline_ms),
        };
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let packet = session.receive(end)?;
            if packet == [255] {
                break;
            }
            session.service(&packet, end)?;
        }
        Ok(session)
    }
    fn receive(&mut self, end: Instant) -> Result<Vec<u8>> {
        let fd = self.output.as_raw_fd();
        let mut len = [0; 4];
        transfer(&mut self.output, fd, &mut len, end)?;
        let n = u32::from_le_bytes(len) as usize;
        ensure!(
            (1..=PACKET).contains(&n),
            "native worker sent an invalid packet size"
        );
        let mut b = vec![0; n];
        transfer(&mut self.output, fd, &mut b, end)?;
        Ok(b)
    }
    fn send(&mut self, bytes: &[u8], end: Instant) -> Result<()> {
        ensure!(bytes.len() <= PACKET, "native reply too large");
        let mut data = (bytes.len() as u32).to_le_bytes().to_vec();
        data.extend(bytes);
        let mut rest = data.as_slice();
        while !rest.is_empty() {
            ready(self.input.as_raw_fd(), libc::POLLOUT, end)?;
            match self.input.write(rest) {
                Ok(0) => bail!("native worker stopped reading"),
                Ok(n) => rest = &rest[n..],
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    fn service(&mut self, packet: &[u8], end: Instant) -> Result<()> {
        let reply = self.request(packet).unwrap_or_else(|_| vec![1]);
        self.send(&reply, end)
    }
    fn request(&mut self, p: &[u8]) -> Result<Vec<u8>> {
        let (&op, b) = p.split_first().context("empty native request")?;
        fn n(b: &[u8], i: usize) -> Result<u32> {
            Ok(u32::from_le_bytes(
                b.get(i..i + 4)
                    .context("short native request")?
                    .try_into()?,
            ))
        }
        fn f(b: &[u8], i: usize) -> Result<f32> {
            Ok(f32::from_bits(n(b, i)?))
        }
        fn number(n: Option<usize>) -> Vec<u8> {
            n.and_then(|v| i32::try_from(v).ok())
                .unwrap_or(-1)
                .to_le_bytes()
                .to_vec()
        }
        fn bytes(data: &[u8], offset: usize, len: usize) -> Result<Vec<u8>> {
            ensure!(len <= 65536 && offset <= data.len(), "invalid chunk");
            let mut v = vec![0];
            v.extend(&data[offset..offset + len.min(data.len() - offset)]);
            Ok(v)
        }
        let ok = match op {
            29 => {
                ensure!(b.len() == 8, "invalid window-control packet");
                return Ok(vec![if self.window.request(n(b, 0)?, n(b, 4)?) {
                    0
                } else {
                    1
                }]);
            }
            28 => {
                ensure!(
                    b.len() >= 8 && b.len() - 8 <= crate::user_files::MAX_REQUEST,
                    "invalid user-file packet"
                );
                return Ok(
                    match self.user_files.call(n(b, 0)?, &b[8..], n(b, 4)? as usize) {
                        Some(bytes) => {
                            let mut reply = vec![0];
                            reply.extend(bytes);
                            reply
                        }
                        None => vec![255],
                    },
                );
            }
            27 => {
                ensure!(b.len() == 4, "invalid input request");
                return Ok(match self.input_events.read(n(b, 0)? as usize) {
                    Some(bytes) => {
                        let mut reply = vec![0];
                        reply.extend(bytes);
                        reply
                    }
                    None => vec![255],
                });
            }
            26 => {
                ensure!(b.is_empty(), "invalid random-value request");
                return Ok(crate::random::value()?.to_le_bytes().to_vec());
            }
            1 => {
                let len = n(b, 0)? as usize;
                ensure!(len <= 1024, "path too long");
                let path = std::str::from_utf8(b.get(4..4 + len).context("short path")?)?;
                let offset = n(b, 4 + len)? as u64;
                let count = n(b, 8 + len)? as usize;
                ensure!(count <= 65536, "asset request too large");
                let mut v = vec![0];
                v.extend(self.package.asset_chunk(path, offset, count)?);
                return Ok(v);
            }
            2 => return Ok(number(Some(self.roms.len()))),
            3 => return Ok(number(self.roms.get(n(b, 0)? as usize).map(Vec::len))),
            4 => {
                return bytes(
                    self.roms.get(n(b, 0)? as usize).context("unknown ROM")?,
                    n(b, 4)? as usize,
                    n(b, 8)? as usize,
                )
            }
            5 => return Ok(number(self.save.as_ref().map(|s| s.bytes.len()))),
            6 => {
                return bytes(
                    &self.save.as_ref().context("save access denied")?.bytes,
                    n(b, 0)? as usize,
                    n(b, 4)? as usize,
                )
            }
            25 => {
                ensure!(
                    self.manifest.builtin_engine.as_deref() == Some(crate::doom::PROFILE)
                        && b.is_empty(),
                    "invalid engine quit request"
                );
                self.exited = true;
                true
            }
            24 => {
                validate_engine_save(&self.manifest, b.len())?;
                let save = self.save.as_mut().context("save access denied")?;
                save.bytes = b.to_vec();
                save.dirty = true;
                true
            }
            7 => {
                ensure!(b.len() <= crate::saves::MAX_SAVE, "save too large");
                let save = self.save.as_mut().context("save access denied")?;
                save.bytes = b.to_vec();
                save.dirty = true;
                true
            }
            8 => {
                self.frame.background = Some(n(b, 0)?);
                true
            }
            9 => self
                .frame
                .rect(f(b, 0)?, f(b, 4)?, f(b, 8)?, f(b, 12)?, n(b, 16)?),
            10 => self.frame.circle(f(b, 0)?, f(b, 4)?, f(b, 8)?, n(b, 12)?),
            11 => self.frame.shader(n(b, 0)? as i32, self.shader_count),
            12 => {
                ensure!(
                    b.len() <= 3200
                        && b.len() % 2 == 0
                        && self.manifest.permissions.audio
                        && (self.audio.is_empty() || self.audio_channels == 1),
                    "audio access denied"
                );
                ensure!(
                    self.audio.len() + b.len() / 2 <= 1600,
                    "audio budget exceeded"
                );
                self.audio.extend(
                    b.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|s| i16::from_le_bytes([s[0], s[1]])),
                );
                true
            }
            23 => {
                let samples =
                    stereo_engine_audio(&self.manifest, self.audio_channels, self.audio.len(), b)?;
                self.audio_channels = 2;
                self.audio.extend(samples);
                true
            }
            13 => {
                let image = crate::drawing::image::Image::new(
                    n(b, 16)?,
                    n(b, 20)?,
                    b.get(24..).context("short image")?,
                )
                .context("invalid image")?;
                self.frame
                    .image(f(b, 0)?, f(b, 4)?, f(b, 8)?, f(b, 12)?, image)
            }
            14 => {
                let len = n(b, 0)? as usize;
                ensure!(len <= 1024, "path too long");
                let path = std::str::from_utf8(b.get(4..4 + len).context("short path")?)?;
                let image = crate::drawing::image::Image::from_png(&self.package.asset(path)?)
                    .context("invalid PNG")?;
                self.frame.image(
                    f(b, 4 + len)?,
                    f(b, 8 + len)?,
                    f(b, 12 + len)?,
                    f(b, 16 + len)?,
                    image,
                )
            }
            15 => {
                let image = crate::drawing::image::Image::new(
                    n(b, 4)?,
                    n(b, 8)?,
                    b.get(12..).context("short image")?,
                )
                .context("invalid image")?;
                self.frame
                    .shader_image(n(b, 0)? as i32, self.shader_count, image)
            }
            16 => {
                ensure!(b.len() <= 1028, "path too long");
                let path = std::str::from_utf8(b.get(4..).context("short path")?)?;
                let image = crate::drawing::image::Image::from_png(&self.package.asset(path)?)
                    .context("invalid PNG")?;
                self.frame
                    .shader_image(n(b, 0)? as i32, self.shader_count, image)
            }
            17 => {
                let a = self
                    .manifest
                    .achievements
                    .get(n(b, 0)? as usize)
                    .context("unknown achievement")?;
                if !self.achievements.contains(&a.id) {
                    self.achievements.push(a.id.clone());
                }
                true
            }
            18 => false,
            22 => {
                let operation = *b.first().context("empty network request")?;
                let capacity = n(b, 1)? as usize;
                return Ok(self.network.call(
                    operation,
                    b.get(5..).context("short network request")?,
                    capacity,
                ));
            }
            19 => {
                if self.engine_data.is_none() {
                    let data = self.roms.first().context("engine ROM missing")?;
                    self.engine_data = Some(match self.manifest.builtin_engine.as_deref() {
                        Some(crate::doom::PROFILE) => {
                            crate::doom::validate_wad(data)?;
                            data.clone()
                        }
                        Some(crate::zelda3::PROFILE) => crate::zelda3_rom::decode(data)?,
                        _ => bail!("no approved engine"),
                    });
                }
                return Ok(number(self.engine_data.as_ref().map(Vec::len)));
            }
            20 => {
                return bytes(
                    self.engine_data
                        .as_ref()
                        .context("engine not initialized")?,
                    n(b, 0)? as usize,
                    n(b, 4)? as usize,
                )
            }
            21 => {
                ensure!(self.manifest.builtin_engine.is_some(), "no approved engine");
                self.diagnostic = Some((n(b, 0)?, n(b, 4)?, n(b, 8)?));
                true
            }
            _ => bail!("unknown native host request"),
        };
        Ok(vec![if ok { 0 } else { 1 }])
    }
    pub fn set_input_events(&mut self, events: Vec<crate::input_events::Event>) {
        self.input_events.begin(events);
    }
    pub fn step(
        &mut self,
        input: u32,
        state: u64,
        events: &[crate::doom_input::Event],
    ) -> Result<u64> {
        ensure!(
            events.len() <= crate::doom_input::MAX_PER_TICK,
            "too many desktop events"
        );
        ensure!(
            events.is_empty()
                || (self.manifest.builtin_engine.as_deref() == Some(crate::doom::PROFILE)
                    && self.manifest.permissions.input),
            "desktop input access denied"
        );
        self.frame.reset();
        self.audio.clear();
        self.audio_channels = 1;
        self.achievements.clear();
        let previous = self.save.as_ref().map(|s| s.bytes.clone());
        let result = (|| {
            let end = Instant::now() + self.deadline;
            let mut request = input.to_le_bytes().to_vec();
            request.extend(state.to_le_bytes());
            for event in events {
                event.encode(&mut request)?;
            }
            self.send(&request, end)?;
            let mut calls = 0;
            loop {
                let p = self.receive(end)?;
                if p.first() == Some(&0) {
                    ensure!(p.len() == 9, "invalid native tick result");
                    return Ok(u64::from_le_bytes(p[1..].try_into()?));
                }
                calls += 1;
                ensure!(calls <= 8192, "native host call budget exceeded");
                self.service(&p, end)?;
            }
        })();
        match result {
            Ok(v) => {
                if let Some(save) = &mut self.save {
                    if let Err(error) = save.commit() {
                        self.user_files.finish(false);
                        self.window.finish(false);
                        return Err(error);
                    }
                }
                self.user_files.finish(true);
                self.window.finish(true);
                Ok(v)
            }
            Err(e) => {
                self.user_files.finish(false);
                self.window.finish(false);
                self.frame.reset();
                self.audio.clear();
                self.achievements.clear();
                if let (Some(save), Some(bytes)) = (&mut self.save, previous) {
                    save.bytes = bytes;
                    save.dirty = false;
                }
                let _ = self.child.kill();
                let _ = self.child.wait();
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod stereo_tests {
    use super::*;
    #[test]
    fn large_engine_saves_require_exact_profile_permission_and_limit() {
        let mut manifest: crate::manifest::Manifest =
            toml::from_str(include_str!("../../ports/doom/manifest.toml")).unwrap();
        assert!(validate_engine_save(&manifest, crate::doom::SAVE_MAX).is_ok());
        assert!(validate_engine_save(&manifest, crate::doom::SAVE_MAX + 1).is_err());
        assert!(validate_engine_save(&manifest, usize::MAX).is_err());
        manifest.permissions.save = false;
        assert!(validate_engine_save(&manifest, 1).is_err());
        manifest.permissions.save = true;
        for engine in [None, Some("doom-unreviewed"), Some(crate::zelda3::PROFILE)] {
            manifest.builtin_engine = engine.map(str::to_owned);
            assert!(validate_engine_save(&manifest, 1).is_err());
        }
    }
    #[test]
    fn stereo_rpc_requires_exact_engine_permission_complete_frames_and_budget() {
        let mut manifest: crate::manifest::Manifest =
            toml::from_str(include_str!("../../ports/zelda3/manifest.toml")).unwrap();
        let payload = [0x34, 0x12, 0xbb, 0xdc];
        assert_eq!(
            stereo_engine_audio(&manifest, 1, 0, &payload).unwrap(),
            [0x1234, -0x2345]
        );
        for (channels, queued) in [(1, 2), (2, 3), (2, 3200), (2, usize::MAX)] {
            assert!(stereo_engine_audio(&manifest, channels, queued, &payload).is_err());
        }
        assert!(stereo_engine_audio(&manifest, 2, 0, &[0; 3]).is_err());
        assert!(stereo_engine_audio(&manifest, 2, 0, &[0; 6404]).is_err());
        manifest.permissions.audio = false;
        assert!(stereo_engine_audio(&manifest, 1, 0, &payload).is_err());
        manifest.permissions.audio = true;
        manifest.builtin_engine = None;
        assert!(stereo_engine_audio(&manifest, 1, 0, &payload).is_err());
        manifest.builtin_engine = Some(crate::doom::PROFILE.into());
        assert!(stereo_engine_audio(&manifest, 1, 0, &payload).is_ok());
        manifest.builtin_engine = Some("doom-unreviewed".into());
        assert!(stereo_engine_audio(&manifest, 1, 0, &payload).is_err());
    }
}
