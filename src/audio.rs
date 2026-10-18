//! Output-only audio. The guest never receives a device handle or microphone access.
use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
const MAX_QUEUED: usize = 4800; // At most 100 ms of audio frames at 48 kHz.
struct Buffer {
    samples: VecDeque<[i16; 2]>,
    phase: f64,
    current: [i16; 2],
    gain: f32,
    music: Option<crate::music::Output>,
}
impl Buffer {
    fn new(volume: u8) -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_QUEUED),
            phase: 1.0,
            current: [0; 2],
            gain: volume as f32 / 100.0,
            music: None,
        }
    }
    fn append(&mut self, samples: &[i16]) -> bool {
        if samples.len() > 1600 || self.samples.len() + samples.len() > MAX_QUEUED {
            return false;
        }
        self.samples
            .extend(samples.iter().map(|&sample| [sample, sample]));
        true
    }
    fn append_stereo(&mut self, samples: &[i16]) -> bool {
        if !samples.len().is_multiple_of(2)
            || samples.len() > 3200
            || self.samples.len() + samples.len() / 2 > MAX_QUEUED
        {
            return false;
        }
        self.samples
            .extend(samples.chunks_exact(2).map(|frame| [frame[0], frame[1]]));
        true
    }
    fn next(&mut self, rate: u32) -> [f32; 2] {
        while self.phase >= 1.0 {
            self.current = self.samples.pop_front().unwrap_or([0; 2]);
            if let Some(music) = &self.music {
                let mut frame = [[0; 2]; 1];
                if music.read(&mut frame) == 1 {
                    for channel in 0..2 {
                        self.current[channel] = (i32::from(self.current[channel])
                            + i32::from(frame[0][channel]))
                        .clamp(i16::MIN as i32, i16::MAX as i32)
                            as i16;
                    }
                }
            }
            self.phase -= 1.0;
        }
        self.phase += 48000.0 / rate as f64;
        self.current
            .map(|sample| sample as f32 / 32768.0 * self.gain)
    }
}
pub struct Player {
    _stream: cpal::Stream,
    sink: Sink,
    error: Arc<Mutex<Option<String>>>,
    non_silent: Arc<AtomicU64>,
}
/// Host-only writer; the device stream remains owned by the desktop app.
#[derive(Clone)]
pub(crate) struct Sink {
    buffer: Arc<Mutex<Buffer>>,
}
impl Sink {
    pub fn submit(&self, samples: &[i16]) -> bool {
        self.buffer.lock().is_ok_and(|mut b| b.append(samples))
    }
    pub fn submit_channels(&self, samples: &[i16], channels: u32) -> bool {
        match channels {
            1 => self.submit(samples),
            2 => self.submit_stereo(samples),
            _ => false,
        }
    }
    pub fn submit_stereo(&self, samples: &[i16]) -> bool {
        self.buffer
            .lock()
            .is_ok_and(|mut b| b.append_stereo(samples))
    }
}
impl Player {
    /// Host-only continuous source consumed by the device clock, outside game ticks.
    pub fn music_source(&self, output: Option<crate::music::Output>) -> bool {
        self.sink.buffer.lock().is_ok_and(|mut buffer| {
            buffer.music = output;
            true
        })
    }
    #[cfg(any(feature = "gui", test))]
    pub(crate) fn sink(&self) -> Sink {
        self.sink.clone()
    }
    pub fn new(volume: u8) -> Result<Self> {
        if volume > 100 {
            bail!("audio volume must be 0–100");
        }
        let device = cpal::default_host()
            .default_output_device()
            .context("no audio output device")?;
        let supported = device.default_output_config()?;
        let config = supported.config();
        if !(8000..=192000).contains(&config.sample_rate) || !(1..=8).contains(&config.channels) {
            bail!("unsupported audio device rate or channel count");
        }
        let buffer = Arc::new(Mutex::new(Buffer::new(volume)));
        let error = Arc::new(Mutex::new(None));
        let non_silent = Arc::new(AtomicU64::new(0));
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => output::<f32>(
                &device,
                config,
                buffer.clone(),
                error.clone(),
                non_silent.clone(),
            )?,
            cpal::SampleFormat::I16 => output::<i16>(
                &device,
                config,
                buffer.clone(),
                error.clone(),
                non_silent.clone(),
            )?,
            cpal::SampleFormat::U16 => output::<u16>(
                &device,
                config,
                buffer.clone(),
                error.clone(),
                non_silent.clone(),
            )?,
            format => bail!("unsupported audio sample format: {format:?}"),
        };
        stream.play()?;
        Ok(Self {
            _stream: stream,
            sink: Sink { buffer },
            error,
            non_silent,
        })
    }
    pub fn submit(&self, samples: &[i16]) -> bool {
        self.sink.submit(samples)
    }
    pub fn submit_channels(&self, samples: &[i16], channels: u32) -> bool {
        self.sink.submit_channels(samples, channels)
    }
    pub fn submit_stereo(&self, samples: &[i16]) -> bool {
        self.sink.submit_stereo(samples)
    }
    pub fn clear(&self) {
        if let Ok(mut b) = self.sink.buffer.lock() {
            b.samples.clear();
            b.current = [0; 2];
        }
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|mut e| e.take())
    }
    pub fn non_silent_frames(&self) -> u64 {
        self.non_silent.load(Ordering::Relaxed)
    }
}
fn device_sample(stereo: [f32; 2], channel: usize, channels: usize) -> f32 {
    match (channel, channels) {
        (_, 1) => (stereo[0] + stereo[1]) * 0.5,
        (0, _) => stereo[0],
        (1, _) => stereo[1],
        _ => 0.0,
    }
}
fn output<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    buffer: Arc<Mutex<Buffer>>,
    error: Arc<Mutex<Option<String>>>,
    non_silent: Arc<AtomicU64>,
) -> Result<cpal::Stream> {
    Ok(device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let Ok(mut buffer) = buffer.try_lock() else {
                data.fill(T::from_sample(0.0_f32));
                return;
            };
            let mut audible = 0;
            for frame in data.chunks_mut(config.channels as usize) {
                let sample = buffer.next(config.sample_rate);
                let channels = frame.len();
                let mut frame_audible = false;
                for (channel, output) in frame.iter_mut().enumerate() {
                    let sample = device_sample(sample, channel, channels);
                    frame_audible |= sample != 0.0;
                    *output = T::from_sample(sample);
                }
                audible += u64::from(frame_audible);
            }
            non_silent.fetch_add(audible, Ordering::Relaxed);
        },
        move |e| {
            if let Ok(mut message) = error.lock() {
                *message = Some(e.to_string().chars().take(1024).collect());
            }
        },
        Some(std::time::Duration::from_secs(1)),
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuous_music_uses_device_phase_and_mixes_stereo_before_master_gain() {
        let source = crate::music::fixture_output(&[[30000, -30000], [123, -456]]);
        let mut buffer = Buffer::new(50);
        buffer.music = Some(source.clone());
        assert!(buffer.append_stereo(&[30000, -30000, 1000, 2000]));
        let first = [32767.0 / 65536.0, -0.5];
        assert_eq!(buffer.next(96000), first);
        assert_eq!(buffer.next(96000), first);
        assert_eq!(source.played_frames(), 1);
        assert_eq!(buffer.next(96000), [1123.0 / 65536.0, 1544.0 / 65536.0]);
        assert_eq!(source.played_frames(), 2);
        buffer.next(96000);
        assert_eq!(buffer.next(96000), [0.0; 2]);
    }
    #[test]
    fn queue_volume_silence_and_resampling_are_bounded() {
        let mut buffer = Buffer::new(50);
        assert!(buffer.append(&[16384, -16384]));
        assert_eq!(buffer.next(48000), [0.25; 2]);
        assert_eq!(buffer.next(48000), [-0.25; 2]);
        assert_eq!(buffer.next(48000), [0.0; 2]);
        let mut muted = Buffer::new(0);
        assert!(muted.append(&[32767]));
        assert_eq!(muted.next(48000), [0.0; 2]);
        for _ in 0..3 {
            assert!(buffer.append(&[1; 1600]));
        }
        assert!(!buffer.append(&[1]));
        assert!(!buffer.append(&[1; 1601]));
        for _ in 0..4800 {
            buffer.next(48000);
        }
        assert!(buffer.samples.is_empty());
        let mut upsample = Buffer::new(100);
        upsample.append(&[16384, -16384]);
        assert_eq!(upsample.next(96000), [0.5; 2]);
        assert_eq!(upsample.next(96000), [0.5; 2]);
        assert_eq!(upsample.next(96000), [-0.5; 2]);
    }
    #[test]
    fn stereo_channels_survive_resampling_volume_and_device_mapping() {
        let mut buffer = Buffer::new(50);
        assert!(buffer.append_stereo(&[16384, -8192, 0, 32767]));
        assert_eq!(buffer.next(96000), [0.25, -0.125]);
        assert_eq!(buffer.next(96000), [0.25, -0.125]);
        let next = buffer.next(96000);
        assert_eq!(next, [0.0, 32767.0 / 65536.0]);
        assert_eq!(device_sample([0.5, -0.25], 0, 2), 0.5);
        assert_eq!(device_sample([0.5, -0.25], 1, 2), -0.25);
        assert_eq!(device_sample([0.5, -0.25], 0, 1), 0.125);
        assert_eq!(device_sample([0.5, -0.25], 2, 6), 0.0);
        assert!(!buffer.append_stereo(&[1]));
        assert!(!buffer.append_stereo(&[1; 3202]));
        let mut full = Buffer::new(100);
        for _ in 0..3 {
            assert!(full.append_stereo(&[1; 3200]));
        }
        assert!(!full.append_stereo(&[1, 2]));
        assert!(!full.append(&[1]));
    }
    #[test]
    #[cfg(all(target_os = "linux", feature = "gui"))]
    #[ignore = "requires GAMEPKG_CUBE2_TEST_ROOT, its cached engine, original data and ALSA null device"]
    fn original_cube2_stereo_reaches_the_output_callback() {
        assert!(std::env::var_os("ALSA_CONFIG_PATH").is_some());
        let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_CUBE2_TEST_ROOT").unwrap());
        let mut game = crate::runtime::Game::load(&root, "com.gamepkg.cube2").unwrap();
        assert_eq!(game.audio_channels(), 2);
        game.cube2_key(56, true, false, 0);
        game.step(0).unwrap();
        game.cube2_key(56, false, false, 0);
        game.cube2_text("mode 0; map complex; showgui \"\"");
        game.step(0).unwrap();
        game.cube2_key(40, true, false, 0);
        game.step(0).unwrap();
        game.cube2_key(40, false, false, 0);
        game.step(0).unwrap();
        let mut audible = false;
        let mut separated = false;
        for _ in 0..120 {
            game.cube2_desktop_input(1 << 16, 0, 0);
            game.step(0).unwrap();
            assert_eq!(game.audio_channels(), 2);
            assert_eq!(game.audio_frame().len() % 2, 0);
            audible |= game.audio_frame().iter().any(|&sample| sample != 0);
            separated |= game
                .audio_frame()
                .chunks_exact(2)
                .any(|pair| pair[0] != pair[1]);
        }
        assert!(audible, "Original engine emitted no sound");
        assert!(separated, "Original stereo channels were collapsed to mono");
        let output = Player::new(100).unwrap();
        game.background_player().unwrap();
        game.cube2_audio_sink(Some(output.sink()));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while output.non_silent_frames() == 0 {
            game.cube2_desktop_input(1 << 16, 0, 0);
            game.step(0).unwrap();
            assert!(
                std::time::Instant::now() < deadline,
                "Background stereo did not reach the device callback"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(output.error().is_none());
        game.finish().unwrap();
    }
    #[test]
    #[ignore = "run only with ALSA_CONFIG_PATH pointing to a null output device"]
    fn audio_output_null_device() {
        assert!(std::env::var_os("ALSA_CONFIG_PATH").is_some());
        let muted = Player::new(0).unwrap();
        for _ in 0..10 {
            muted.submit(&[10000; 768]);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(muted.non_silent_frames(), 0);
        drop(muted);
        let player = Player::new(50).unwrap();
        let sink = player.sink();
        let writer = std::thread::spawn(move || {
            for _ in 0..10 {
                sink.submit(&[10000; 768]);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        });
        writer.join().unwrap();
        assert!(player.non_silent_frames() > 0);
        assert!(player.error().is_none());
        drop(player);
        let stereo = Player::new(100).unwrap();
        let frames: Vec<i16> = (0..768).flat_map(|_| [10000, -3000]).collect();
        for _ in 0..10 {
            stereo.submit_stereo(&frames);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(stereo.non_silent_frames() > 0);
        assert!(stereo.error().is_none());
    }
}
