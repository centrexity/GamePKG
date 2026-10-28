#[cfg(test)]
use super::transport::append_engine_log;
use super::transport::Worker;
use anyhow::{bail, ensure, Context, Result};
use std::{
    path::Path,
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
    pub(crate) effects: crate::audio_effects::Runtime,
    clock: crate::frame_clock::Clock,
    pub(crate) window: crate::window_control::Service,
    pub(crate) user_files: crate::user_files::Service,
    input_events: crate::input_events::Queue,
    transport: Worker,
    _binary: tempfile::TempDir,
    package: crate::archive::Package,
    asset_image: crate::drawing::image::Cache,
    manifest: crate::manifest::Manifest,
    roms: Vec<Vec<u8>>,
    save: Option<crate::saves::Save>,
    engine_data: Option<Vec<u8>>,
    video: Option<crate::manifest::ZeldaVideo>,
    video_configured: bool,
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
        self.transport.stop();
    }
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
        let video = crate::video_selections::effective(root, &manifest)?;
        let window = crate::window_control::Service::with_homepage(
            manifest.permissions.input,
            manifest.homepage.as_deref(),
        )?;
        let path = super::path_for_digest(root, digest);
        let bytes = crate::files::read_regular(&path, 64 * 1048576, false)
            .context("Build this game for this computer before playing")?;
        super::validate_executable(&bytes)?;
        crate::integrity::verify_build(&path, &bytes)?;
        let memory = manifest
            .limits
            .hard_memory_bytes
            .min(settings.memory_ceiling_mib * 1048576);
        manifest.limits.ensure_render_memory(memory)?;
        if let Some(profile) = manifest.builtin_engine.as_deref() {
            ensure!(
                memory >= crate::builtin::memory(profile)?,
                "memory ceiling is too low for this engine"
            );
        }
        let memory = crate::zelda_snapshots::guest_memory(&manifest, memory)?;
        let (effects, memory) = crate::audio_effects::Runtime::new(
            Some(&package),
            manifest.limits.effects_memory_bytes,
            manifest.limits.music_memory_bytes,
            memory,
            manifest.permissions.audio,
        )?;
        let save = if manifest.permissions.save {
            Some(crate::saves::Save::open_for_game(root, &manifest)?)
        } else {
            None
        };
        let binary = tempfile::tempdir()?;
        #[cfg(windows)]
        ensure!(
            manifest.builtin_engine.as_deref().is_none_or(|profile| {
                profile == crate::doom::PROFILE || profile == crate::zelda3::PROFILE
            }),
            "this engine uses its separate native player"
        );
        let name = if cfg!(windows) { "game.exe" } else { "game" };
        std::fs::write(binary.path().join(name), bytes)?;
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                binary.path().join("game"),
                std::fs::Permissions::from_mode(0o500),
            )?;
        }
        let transport = Worker::spawn(binary.path(), memory, digest)?;
        let render_profile = manifest.limits.render_profile;
        let mut session = Self {
            effects,
            clock: crate::frame_clock::Clock::default(),
            transport,
            _binary: binary,
            package,
            asset_image: crate::drawing::image::Cache::for_profile(render_profile),
            network: crate::network::Service::new(manifest.permissions.clone()),
            input_events: crate::input_events::Queue::new(manifest.permissions.input),
            window,
            user_files: crate::user_files::Service::new(manifest.permissions.files.clone()),
            manifest,
            roms,
            save,
            engine_data: None,
            video,
            video_configured: false,
            frame: crate::drawing::Frame::for_profile(render_profile),
            audio: Vec::new(),
            audio_channels: 1,
            achievements: Vec::new(),
            diagnostic: None,
            exited: false,
            shader_count,
            deadline: Duration::from_millis(settings.tick_deadline_ms),
        };
        let end = Instant::now() + Duration::from_secs(5);
        let ready = (|| -> Result<()> {
            loop {
                let packet = session.receive(end)?;
                if packet == [255] {
                    crate::video_selections::require_engine_support(
                        session.video,
                        session.video_configured,
                    )?;
                    return Ok(());
                }
                session.service(&packet, end)?;
            }
        })();
        if let Err(error) = ready {
            return Err(session.stop_with_log(error));
        }
        Ok(session)
    }
    fn stop_with_log(&mut self, error: anyhow::Error) -> anyhow::Error {
        self.transport.stop();
        let message = self.transport.message();
        if message.is_empty() {
            error
        } else {
            error.context(format!("Native game: {message}"))
        }
    }
    fn receive(&mut self, end: Instant) -> Result<Vec<u8>> {
        self.receive_bounded(end, PACKET)
    }
    fn receive_bounded(&mut self, end: Instant, maximum: usize) -> Result<Vec<u8>> {
        let mut len = [0; 4];
        self.transport.read_exact(&mut len, end)?;
        let n = u32::from_le_bytes(len) as usize;
        ensure!(
            (1..=maximum).contains(&n),
            "native worker sent an invalid packet size"
        );
        let mut b = vec![0; n];
        self.transport.read_exact(&mut b, end)?;
        Ok(b)
    }
    fn send(&mut self, bytes: &[u8], end: Instant) -> Result<()> {
        self.send_tagged(bytes, end, false)
    }
    fn send_tagged(&mut self, bytes: &[u8], end: Instant, control: bool) -> Result<()> {
        ensure!(bytes.len() <= PACKET, "native reply too large");
        let prefix = bytes.len() as u32 | if control { 0x80000000 } else { 0 };
        let mut data = prefix.to_le_bytes().to_vec();
        data.extend(bytes);
        self.transport.write_all(&data, end)
    }

    // Only the trusted Zelda bridge handles these messages, between ordinary ticks.
    // A rejected state is recoverable; framing/transport errors terminate the worker.
    fn snapshot_command(
        &mut self,
        bytes: &[u8],
        expected: usize,
        end: Instant,
    ) -> Result<Option<Vec<u8>>> {
        let result = (|| {
            ensure!(
                self.manifest.builtin_engine.as_deref() == Some(crate::zelda3::PROFILE)
                    && self.manifest.permissions.save,
                "Zelda snapshots require save permission"
            );
            self.send_tagged(bytes, end, true)?;
            ensure!(
                expected <= crate::zelda_snapshots::CHUNK,
                "invalid snapshot response extent"
            );
            let mut response = self.receive_bounded(end, expected + 2)?;
            ensure!(
                response.len() >= 2 && response[0] == 0x82,
                "invalid native snapshot response"
            );
            match response[1] {
                0 => {
                    ensure!(
                        response.len() == expected + 2,
                        "invalid native snapshot payload extent"
                    );
                    response.drain(..2);
                    Ok(Some(response))
                }
                1 if response.len() == 2 => Ok(None),
                _ => bail!("invalid native snapshot status"),
            }
        })();
        result.map_err(|error| self.stop_with_log(error))
    }
    pub(crate) fn capture_snapshot(&mut self, output: &mut [u8]) -> Result<()> {
        use crate::zelda_snapshots::{CHUNK, SIZE};
        ensure!(output.len() == SIZE, "invalid Zelda snapshot size");
        let end = Instant::now() + self.deadline;
        let size = self
            .snapshot_command(&[0], 4, end)?
            .context("Zelda snapshot capture failed")?;
        ensure!(
            size == (SIZE as u32).to_le_bytes(),
            "invalid native snapshot size"
        );
        for (index, chunk) in output.chunks_mut(CHUNK).enumerate() {
            let mut command = vec![1];
            command.extend_from_slice(&((index * CHUNK) as u32).to_le_bytes());
            command.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            let bytes = self
                .snapshot_command(&command, chunk.len(), end)?
                .context("Zelda snapshot read failed")?;
            ensure!(bytes.len() == chunk.len(), "invalid native snapshot chunk");
            chunk.copy_from_slice(&bytes);
        }
        Ok(())
    }
    pub(crate) fn restore_snapshot(&mut self, bytes: &[u8]) -> Result<()> {
        use crate::zelda_snapshots::{CHUNK, SIZE};
        ensure!(bytes.len() == SIZE, "invalid Zelda snapshot size");
        let end = Instant::now() + self.deadline;
        for (index, chunk) in bytes.chunks(CHUNK).enumerate() {
            let mut command = Vec::with_capacity(chunk.len() + 5);
            command.push(2);
            command.extend_from_slice(&((index * CHUNK) as u32).to_le_bytes());
            command.extend_from_slice(chunk);
            let response = self
                .snapshot_command(&command, 0, end)?
                .context("Zelda snapshot write failed")?;
            ensure!(
                response.is_empty(),
                "invalid native snapshot write response"
            );
        }
        let mut command = vec![3];
        command.extend_from_slice(&(SIZE as u32).to_le_bytes());
        let response = self
            .snapshot_command(&command, 12, end)?
            .context("Zelda snapshot was rejected")?;
        ensure!(response.len() == 12, "invalid native snapshot diagnostic");
        let number = |offset| u32::from_le_bytes(response[offset..offset + 4].try_into().unwrap());
        self.diagnostic = Some((number(0), number(4), number(8)));
        self.frame.reset();
        self.audio.clear();
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
            43 => {
                ensure!(b.is_empty(), "invalid random-block request");
                let mut bytes = vec![0; 512];
                crate::random::fill(&mut bytes)?;
                return Ok(bytes);
            }
            41 => {
                ensure!(
                    b.is_empty()
                        && self.manifest.builtin_engine.as_deref() == Some(crate::zelda3::PROFILE),
                    "invalid Zelda video request"
                );
                let video = self.video.unwrap_or_default();
                video.validate()?;
                self.video_configured = true;
                return Ok(number(Some(
                    (video.extra_width | (video.configuration_bits() << 8)) as usize,
                )));
            }
            40 => {
                ensure!(
                    b.len() == 4
                        && self.manifest.builtin_engine.as_deref() == Some(crate::doom::PROFILE),
                    "invalid DOOM mode request"
                );
                let index = n(b, 0)? as usize;
                ensure!(
                    index < self.roms.len().saturating_sub(1) && index < 6,
                    "unknown patch mode index"
                );
                return Ok(number(Some(
                    self.manifest
                        .doom_wad_modes
                        .get(index)
                        .copied()
                        .unwrap_or(crate::manifest::DoomWadMode::File) as usize,
                )));
            }
            38 => {
                ensure!(
                    (8..=1032).contains(&b.len()),
                    "invalid effect-control packet"
                );
                let path = std::str::from_utf8(&b[8..])?;
                self.effects.call(n(b, 0)?, path, n(b, 4)?)
            }
            42 => {
                ensure!(b.is_empty(), "invalid window-size packet");
                let mut reply = vec![0];
                reply.extend(self.window.size_observation().to_le_bytes());
                return Ok(reply);
            }
            36 => {
                ensure!(b.is_empty(), "invalid window-state packet");
                let mut reply = vec![0];
                reply.extend(self.window.observation().to_le_bytes());
                return Ok(reply);
            }
            34 => {
                ensure!(b.is_empty(), "invalid frame-time packet");
                let mut reply = vec![0];
                reply.extend(self.clock.micros().to_le_bytes());
                return Ok(reply);
            }
            31 => {
                ensure!(b.len() == 16, "invalid draw-clip packet");
                self.frame
                    .clip(Some([f(b, 0)?, f(b, 4)?, f(b, 8)?, f(b, 12)?]))
            }
            32 => {
                ensure!(b.is_empty(), "invalid draw-clip-reset packet");
                self.frame.clip(None)
            }
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
                save.discard_pending();
                save.bytes = b.to_vec();
                save.dirty = true;
                true
            }
            39 => {
                ensure!(b.len() >= 8, "invalid save chunk");
                let offset = n(b, 0)? as usize;
                let total = n(b, 4)? as usize;
                return Ok(vec![if self
                    .save
                    .as_mut()
                    .is_some_and(|save| save.chunk(offset, total, &b[8..]))
                {
                    0
                } else {
                    1
                }]);
            }
            7 => {
                ensure!(b.len() <= crate::saves::MAX_SAVE, "save too large");
                let save = self.save.as_mut().context("save access denied")?;
                save.discard_pending();
                save.bytes = b.to_vec();
                save.dirty = true;
                true
            }
            8 => {
                ensure!(b.len() == 4, "invalid draw-clear packet");
                self.frame.clear(n(b, 0)?);
                true
            }
            9 => self
                .frame
                .rect(f(b, 0)?, f(b, 4)?, f(b, 8)?, f(b, 12)?, n(b, 16)?),
            10 => self.frame.circle(f(b, 0)?, f(b, 4)?, f(b, 8)?, n(b, 12)?),
            11 => self.frame.shader(n(b, 0)? as i32, self.shader_count),
            12 | 37 => {
                ensure!(
                    b.len() <= 3200 && b.len() % 2 == 0,
                    "invalid portable audio packet"
                );
                let samples: Vec<_> = b
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|s| i16::from_le_bytes(*s))
                    .collect();
                crate::audio_frame::append(
                    &mut self.audio,
                    &mut self.audio_channels,
                    &samples,
                    if op == 37 { 2 } else { 1 },
                    self.manifest.permissions.audio && !self.effects.enabled(),
                )
            }
            23 => {
                let samples =
                    stereo_engine_audio(&self.manifest, self.audio_channels, self.audio.len(), b)?;
                self.audio_channels = 2;
                self.audio.extend(samples);
                true
            }
            13 | 33 => {
                let image = crate::drawing::image::Image::new(
                    n(b, 16)?,
                    n(b, 20)?,
                    b.get(24..).context("short image")?,
                )
                .context("invalid image")?;
                if op == 33 {
                    self.frame
                        .linear_image(f(b, 0)?, f(b, 4)?, f(b, 8)?, f(b, 12)?, image)
                } else {
                    self.frame
                        .image(f(b, 0)?, f(b, 4)?, f(b, 8)?, f(b, 12)?, image)
                }
            }
            14 | 30 | 35 => {
                let len = n(b, 0)? as usize;
                ensure!(
                    len <= 1024 && b.len() == 20 + len,
                    "invalid asset-image packet"
                );
                let path = std::str::from_utf8(b.get(4..4 + len).context("short path")?)?;
                let (x, y, w, h) = (
                    f(b, 4 + len)?,
                    f(b, 8 + len)?,
                    f(b, 12 + len)?,
                    f(b, 16 + len)?,
                );
                let Some(image) = crate::drawing::image::asset(
                    &mut self.package,
                    &mut self.asset_image,
                    &self.frame,
                    path,
                    [x, y, w, h],
                ) else {
                    return Ok(vec![1]);
                };
                if op == 35 {
                    self.frame.straight_linear_image(x, y, w, h, image)
                } else {
                    self.frame.image_arc(x, y, w, h, image, op == 30)
                }
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
                            crate::doom::validate_patch_set(data.len(), &self.roms[1..])?;
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
        if let Some(save) = self.save.as_mut() {
            save.discard_pending();
        }
        self.effects.begin()?;
        self.clock.begin();
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
                ensure!(
                    calls <= super::MAX_CALLS_PER_TICK,
                    "native host call budget exceeded"
                );
                self.service(&p, end)?;
            }
        })();
        match result {
            Ok(v) => {
                if let Some(save) = &mut self.save {
                    if let Err(error) = save.commit() {
                        self.frame.reset();
                        self.audio.clear();
                        self.audio_channels = 1;
                        self.user_files.finish(false);
                        self.window.finish(false);
                        let _ = self.effects.finish(false);
                        return Err(error);
                    }
                }
                self.user_files.finish(true);
                self.window.finish(true);
                Ok(v)
            }
            Err(e) => {
                let _ = self.effects.finish(false);
                self.user_files.finish(false);
                self.window.finish(false);
                self.frame.reset();
                self.audio.clear();
                self.audio_channels = 1;
                self.achievements.clear();
                if let (Some(save), Some(bytes)) = (&mut self.save, previous) {
                    save.bytes = bytes;
                    save.dirty = false;
                }
                Err(self.stop_with_log(e))
            }
        }
    }
}

#[cfg(test)]
mod stereo_tests {
    use super::*;
    #[test]
    fn engine_diagnostics_keep_only_the_bounded_newest_bytes() {
        let mut log = Vec::new();
        let mut all = Vec::new();
        for i in 0..100u8 {
            let chunk = vec![i; 173];
            all.extend_from_slice(&chunk);
            append_engine_log(&mut log, &chunk);
            assert!(log.len() <= 4096);
        }
        assert_eq!(log, &all[all.len() - 4096..]);
        let large = vec![42; 9000];
        append_engine_log(&mut log, &large);
        assert_eq!(log, &large[large.len() - 4096..]);
        append_engine_log(&mut log, b"missing PKGFLAT");
        assert!(log.ends_with(b"missing PKGFLAT") && log.len() == 4096);
    }
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
