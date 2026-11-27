#[cfg(test)]
use super::transport::append_engine_log;
use super::transport::Worker;
use anyhow::{bail, ensure, Context, Result};
use std::{
    path::Path,
    time::{Duration, Instant},
};
const PACKET: usize = 8 * 1048576 + 65536;

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
    roms: crate::files::RomSet,
    save: Option<crate::saves::Save>,
    pub frame: crate::drawing::Frame,
    pub audio: Vec<i16>,
    pub audio_channels: u32,
    pub achievements: Vec<String>,
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
        roms: crate::files::RomSet,
        settings: &crate::settings::Settings,
        shader_count: usize,
    ) -> Result<Self> {
        crate::build_policy::require(manifest.builtin_engine.as_deref())?;

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
            frame: crate::drawing::Frame::for_profile(render_profile),
            audio: Vec::new(),
            audio_channels: 1,
            achievements: Vec::new(),
            shader_count,
            deadline: Duration::from_millis(settings.tick_deadline_ms),
        };
        let end = Instant::now() + Duration::from_secs(5);
        let ready = (|| -> Result<()> {
            loop {
                let packet = session.receive(end)?;
                if packet == [255] {
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
        self.transport
            .read_exact(&mut len, end)
            .context("reading native packet length")?;
        let n = u32::from_le_bytes(len) as usize;
        ensure!(
            (1..=maximum).contains(&n),
            "native worker sent an invalid packet size"
        );
        let mut b = vec![0; n];
        self.transport
            .read_exact(&mut b, end)
            .with_context(|| format!("reading native packet body ({n} bytes)"))?;
        Ok(b)
    }
    fn send(&mut self, bytes: &[u8], end: Instant) -> Result<()> {
        ensure!(bytes.len() <= PACKET, "native reply too large");
        let mut data = (bytes.len() as u32).to_le_bytes().to_vec();
        data.extend(bytes);
        self.transport.write_all(&data, end)
    }

    fn service(&mut self, packet: &[u8], end: Instant) -> Result<()> {
        let began = Instant::now();
        let reply = self.request(packet).unwrap_or_else(|_| vec![1]);
        self.send(&reply, end).with_context(|| {
            format!(
                "replying to native host request {} ({} input bytes, {} reply bytes, {} ms servicing)",
                packet.first().copied().unwrap_or(0),
                packet.len(),
                reply.len(),
                began.elapsed().as_millis()
            )
        })
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
            3 => {
                return Ok(number(
                    self.roms
                        .size(n(b, 0)? as usize)
                        .and_then(|size| usize::try_from(size).ok()),
                ))
            }
            4 => {
                let data = self
                    .roms
                    .read(n(b, 0)? as usize, u64::from(n(b, 4)?), n(b, 8)? as usize)
                    .context("invalid ROM chunk or failed read")?;
                let mut response = Vec::with_capacity(1 + data.len());
                response.push(0);
                response.extend(data);
                return Ok(response);
            }
            5 => return Ok(number(self.save.as_ref().map(|s| s.bytes.len()))),
            6 => {
                return bytes(
                    &self.save.as_ref().context("save access denied")?.bytes,
                    n(b, 0)? as usize,
                    n(b, 4)? as usize,
                )
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
            _ => bail!("unknown native host request"),
        };
        Ok(vec![if ok { 0 } else { 1 }])
    }
    pub fn set_input_events(&mut self, events: Vec<crate::input_events::Event>) {
        self.input_events.begin(events);
    }
    pub fn step(&mut self, input: u32, state: u64) -> Result<u64> {
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
        let began = Instant::now();
        let mut calls = 0;
        let mut last_request = None;
        let result = (|| {
            let end = Instant::now() + self.deadline;
            let mut request = input.to_le_bytes().to_vec();
            request.extend(state.to_le_bytes());
            self.send(&request, end)?;
            loop {
                let p = self.receive(end)?;
                if p.first() == Some(&0) {
                    ensure!(p.len() == 9, "invalid native tick result");
                    return Ok(u64::from_le_bytes(p[1..].try_into()?));
                }
                calls += 1;
                last_request = Some((p[0], p.len()));
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
                let elapsed = began.elapsed().as_micros();
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
                Err(self.stop_with_log(e.context(format!(
                    "native tick stopped after {elapsed} us; {calls} host calls; last request {last_request:?}"
                ))))
            }
        }
    }
}

#[cfg(test)]
mod native_log_tests {
    use super::*;
    #[test]
    fn engine_log_keeps_only_the_newest_bounded_bytes() {
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
}
