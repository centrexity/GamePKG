//! Output-only audio. The guest never receives a device handle or microphone access.
use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
mod pcm;
const MAX_COMMANDS: usize = 32;
struct Buffer {
    samples: Arc<pcm::Queue>,
    phase: f64,
    current: [i16; 2],
    gain: f32,
    music: Option<crate::music::Output>,
    original: Option<Box<crate::audio_mix_stream::Playback>>,
    original_error: Option<String>,
}
impl Buffer {
    fn new(volume: u8) -> Self {
        Self {
            samples: Arc::new(pcm::Queue::new()),
            phase: 1.0,
            current: [0; 2],
            gain: volume as f32 / 100.0,
            music: None,
            original: None,
            original_error: None,
        }
    }
    #[cfg(test)]
    fn append(&mut self, samples: &[i16]) -> bool {
        self.original.is_none() && self.samples.push(samples, 1)
    }
    #[cfg(test)]
    fn append_stereo(&mut self, samples: &[i16]) -> bool {
        self.original.is_none() && self.samples.push(samples, 2)
    }
    fn next(&mut self, rate: u32) -> [f32; 2] {
        while self.phase >= 1.0 {
            self.current = if self.original.is_none() {
                self.samples.pop().unwrap_or([0; 2])
            } else {
                [0; 2]
            };
            if let Some(original) = &mut self.original {
                match original.frame() {
                    Ok(frame) => self.current = frame,
                    Err(error) => {
                        self.original_error = Some(error.to_string().chars().take(1024).collect());
                        self.original = None;
                        self.current = [0; 2];
                    }
                }
            }
            if let Some(music) = &self.music {
                let mut frame = [[0; 2]; 1];
                if music.read(&mut frame) == 1 {
                    for (current, music) in self.current.iter_mut().zip(frame[0]) {
                        *current = crate::audio_voices::mix_sample(music, *current, 128);
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
enum Command {
    Music(Option<crate::music::Output>),
    Original(Option<Box<crate::audio_mix_stream::Playback>>, u64),
    Play(Arc<crate::audio_voices::Clip>),
    Volume(u8),
    Pause(bool),
    Stop,
}
struct Controls {
    commands: VecDeque<Command>,
    original: bool,
}
struct Shared {
    closed: AtomicBool,
    controls: Mutex<Controls>,
    samples: Arc<pcm::Queue>,
    clear_position: AtomicU64,
    clear_serial: AtomicU64,
    failures: AtomicU32,
}
struct Device {
    buffer: Buffer,
    shared: Arc<Shared>,
    pending: VecDeque<Command>,
    cleared: u64,
}
impl Device {
    fn new(volume: u8) -> (Self, Sink) {
        let buffer = Buffer::new(volume);
        let shared = Arc::new(Shared {
            closed: AtomicBool::new(false),
            controls: Mutex::new(Controls {
                commands: VecDeque::with_capacity(MAX_COMMANDS),
                original: false,
            }),
            samples: buffer.samples.clone(),
            clear_position: AtomicU64::new(0),
            clear_serial: AtomicU64::new(0),
            failures: AtomicU32::new(0),
        });
        let sink = Sink {
            shared: shared.clone(),
        };
        (
            Self {
                buffer,
                shared,
                pending: VecDeque::with_capacity(MAX_COMMANDS),
                cleared: 0,
            },
            sink,
        )
    }
    fn begin(&mut self) {
        // Controls never own playback state. If they are busy, render the
        // current state and queued PCM instead of substituting silence.
        if let Ok(mut controls) = self.shared.controls.try_lock() {
            std::mem::swap(&mut self.pending, &mut controls.commands);
        }
        while let Some(command) = self.pending.pop_front() {
            let accepted = match command {
                Command::Music(source) => {
                    self.buffer.original = None;
                    self.buffer.music = source;
                    true
                }
                Command::Original(source, position) => {
                    self.buffer.samples.discard_through(position);
                    self.buffer.current = [0; 2];
                    self.buffer.phase = 1.0;
                    self.buffer.music = None;
                    self.buffer.original = source;
                    true
                }
                Command::Play(clip) => self
                    .buffer
                    .original
                    .as_mut()
                    .is_some_and(|source| source.effects().play(clip).is_some()),
                Command::Volume(volume) => self
                    .buffer
                    .original
                    .as_mut()
                    .is_some_and(|source| source.effects().volume(volume)),
                Command::Pause(paused) => self.buffer.original.as_mut().is_some_and(|source| {
                    source.effects().pause(paused);
                    true
                }),
                Command::Stop => self.buffer.original.as_mut().is_some_and(|source| {
                    source.effects().stop();
                    true
                }),
            };
            if !accepted {
                self.shared.failures.fetch_or(1, Ordering::Release);
            }
        }
        let serial = self.shared.clear_serial.load(Ordering::Acquire);
        if serial != self.cleared {
            self.buffer
                .samples
                .discard_through(self.shared.clear_position.load(Ordering::Acquire));
            self.buffer.current = [0; 2];
            self.cleared = serial;
        }
    }
    fn next(&mut self, rate: u32) -> [f32; 2] {
        let frame = self.buffer.next(rate);
        if self.buffer.original_error.take().is_some() {
            self.shared.failures.fetch_or(2, Ordering::Release);
        }
        frame
    }
}
impl Drop for Device {
    fn drop(&mut self) {
        self.shared.closed.store(true, Ordering::Release);
    }
}
pub struct Player {
    _stream: cpal::Stream,
    sink: Sink,
    error: Arc<Mutex<Option<String>>>,
    non_silent: Arc<AtomicU64>,
}
/// Host-only producer. The callback exclusively owns mutable playback state.
#[derive(Clone)]
pub(crate) struct Sink {
    shared: Arc<Shared>,
}
impl Sink {
    pub fn submit(&self, samples: &[i16]) -> bool {
        self.submit_channels(samples, 1)
    }
    pub fn submit_channels(&self, samples: &[i16], channels: u32) -> bool {
        self.shared.controls.lock().is_ok_and(|controls| {
            !self.shared.closed.load(Ordering::Acquire)
                && !controls.original
                && self.shared.samples.push(samples, channels as usize)
        })
    }
    pub fn submit_stereo(&self, samples: &[i16]) -> bool {
        self.submit_channels(samples, 2)
    }
    fn clear(&self) {
        if let Ok(_controls) = self.shared.controls.lock() {
            self.shared
                .clear_position
                .store(self.shared.samples.position(), Ordering::Release);
            let serial = self.shared.clear_serial.load(Ordering::Relaxed);
            if let Some(next) = serial.checked_add(1) {
                self.shared.clear_serial.store(next, Ordering::Release);
            } else {
                self.shared.failures.fetch_or(4, Ordering::Release);
            }
        }
    }

    fn command(&self, command: Command, original_required: bool) -> bool {
        self.shared.controls.lock().is_ok_and(|mut controls| {
            if self.shared.closed.load(Ordering::Acquire)
                || controls.commands.len() == MAX_COMMANDS
                || (original_required && !controls.original)
            {
                return false;
            }
            match &command {
                Command::Music(_) => controls.original = false,
                Command::Original(source, _) => controls.original = source.is_some(),
                _ => {}
            }
            controls.commands.push_back(command);
            true
        })
    }
}
impl Player {
    /// Queues a host control; false means invalid input or bounded backpressure.
    /// The device applies accepted controls in order at a callback boundary.
    pub fn music_source(&self, output: Option<crate::music::Output>) -> bool {
        if output
            .as_ref()
            .is_some_and(|output| output.sample_rate() != 48000)
        {
            return false;
        }
        self.sink.command(Command::Music(output), false)
    }
    pub fn original_source(&self, output: Option<crate::audio_mix_stream::Playback>) -> bool {
        // Capture the PCM boundary under the same producer lock as publication.
        self.sink.shared.controls.lock().is_ok_and(|mut controls| {
            if self.sink.shared.closed.load(Ordering::Acquire)
                || controls.commands.len() == MAX_COMMANDS
            {
                return false;
            }
            let position = self.sink.shared.samples.position();
            controls.original = output.is_some();
            controls
                .commands
                .push_back(Command::Original(output.map(Box::new), position));
            true
        })
    }
    /// True acknowledges enqueueing, not channel allocation. Exhausted voices
    /// are reported through error() after the callback processes the command.
    pub fn effect_play(&self, clip: Arc<crate::audio_voices::Clip>) -> bool {
        self.sink.command(Command::Play(clip), true)
    }
    pub fn effect_volume(&self, volume: u8) -> bool {
        volume <= 128 && self.sink.command(Command::Volume(volume), true)
    }
    pub fn effect_pause(&self, paused: bool) -> bool {
        self.sink.command(Command::Pause(paused), true)
    }
    pub fn effect_stop(&self) -> bool {
        self.sink.command(Command::Stop, true)
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
        let (playback, sink) = Device::new(volume);
        let error = Arc::new(Mutex::new(None));
        let non_silent = Arc::new(AtomicU64::new(0));
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                output::<f32>(&device, config, playback, error.clone(), non_silent.clone())?
            }
            cpal::SampleFormat::I16 => {
                output::<i16>(&device, config, playback, error.clone(), non_silent.clone())?
            }
            cpal::SampleFormat::U16 => {
                output::<u16>(&device, config, playback, error.clone(), non_silent.clone())?
            }
            format => bail!("unsupported audio sample format: {format:?}"),
        };
        stream.play()?;
        Ok(Self {
            _stream: stream,
            sink,
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
        self.sink.clear();
    }
    pub fn error(&self) -> Option<String> {
        match self.sink.shared.failures.swap(0, Ordering::AcqRel) {
            0 => self.error.lock().ok().and_then(|mut error| error.take()),
            1 => Some("audio effect command rejected: no free channel or inactive source".into()),
            2 => Some("combined audio playback failed".into()),
            4 => Some("audio clear counter exhausted".into()),
            _ => Some("combined audio playback and effect commands failed".into()),
        }
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
    mut playback: Device,
    error: Arc<Mutex<Option<String>>>,
    non_silent: Arc<AtomicU64>,
) -> Result<cpal::Stream> {
    Ok(device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            playback.begin();
            let mut audible = 0;
            for frame in data.chunks_mut(config.channels as usize) {
                let sample = playback.next(config.sample_rate);
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
    fn callback_consumes_actual_pcm_and_music_while_host_controls_are_locked() {
        let (mut device, sink) = Device::new(100);
        assert!(sink.submit_stereo(&[1000, -2000, 3000, -4000]));
        device.buffer.music = Some(crate::music::fixture_output(&[[123, -456], [789, -987]]));
        let locked = sink.shared.controls.lock().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            device.begin();
            send.send([device.next(48000), device.next(48000)]).unwrap();
        });
        let result = receive.recv_timeout(std::time::Duration::from_millis(200));
        drop(locked);
        reader.join().unwrap();
        assert_eq!(
            result.unwrap(),
            [
                [1123.0 / 32768.0, -2456.0 / 32768.0],
                [3789.0 / 32768.0, -4987.0 / 32768.0]
            ]
        );
    }
    #[test]
    fn callback_commands_are_bounded_ordered_and_report_voice_exhaustion() {
        let (mut device, sink) = Device::new(100);
        for _ in 0..MAX_COMMANDS {
            assert!(sink.command(Command::Music(None), false));
        }
        assert!(!sink.command(Command::Music(None), false));
        assert!(!sink.command(Command::Stop, true));
        device.begin();
        assert!(sink.shared.controls.lock().unwrap().commands.is_empty());
        let budget = crate::memory_budget::Budget::new(128 * 1024);
        let source = crate::music::fixture_output_at_rate(&[[0; 2]], 44100);
        let playback = crate::audio_mix_stream::Playback::new(source, &budget).unwrap();
        assert!(sink.command(Command::Original(Some(Box::new(playback)), 0), false));
        let clip = crate::audio_voices::Clip::new(&[123, -456], &budget).unwrap();
        for _ in 0..5 {
            assert!(sink.command(Command::Play(clip.clone()), true));
        }
        device.begin();
        assert_eq!(
            device.buffer.original.as_mut().unwrap().effects().active(),
            4
        );
        assert_eq!(sink.shared.failures.swap(0, Ordering::AcqRel), 1);
        assert!(sink.command(Command::Pause(true), true));
        assert!(sink.command(Command::Stop, true));
        device.begin();
        assert_eq!(
            device.buffer.original.as_mut().unwrap().effects().active(),
            0
        );
        assert!(sink.command(Command::Original(None, 0), false));
        assert!(!sink.command(Command::Play(clip.clone()), true));
        device.begin();
        drop(clip);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn clear_preserves_later_pcm_even_while_controls_are_busy() {
        let (mut device, sink) = Device::new(100);
        assert!(sink.submit(&[1000, 2000]));
        sink.clear();
        assert!(sink.submit_stereo(&[123, -456]));
        let locked = sink.shared.controls.lock().unwrap();
        device.begin();
        assert_eq!(device.next(48000), [123.0 / 32768.0, -456.0 / 32768.0]);
        assert_eq!(device.next(48000), [0.0; 2]);
        drop(locked);
        sink.shared.clear_serial.store(u64::MAX, Ordering::Release);
        sink.clear();
        assert_eq!(sink.shared.clear_serial.load(Ordering::Acquire), u64::MAX);
        assert_eq!(sink.shared.failures.swap(0, Ordering::AcqRel), 4);
        drop(device);
        assert!(!sink.submit(&[1]));
        assert!(!sink.command(Command::Music(None), false));
    }
    #[test]
    fn pending_source_detach_preserves_future_pcm_until_it_is_applied() {
        let (mut device, sink) = Device::new(100);
        let budget = crate::memory_budget::Budget::new(128 * 1024);
        let source = crate::music::fixture_output_at_rate(&[[0; 2]], 44100);
        let playback = crate::audio_mix_stream::Playback::new(source, &budget).unwrap();
        assert!(sink.command(Command::Original(Some(Box::new(playback)), 0), false));
        device.begin();
        assert!(!sink.submit(&[1]));
        assert!(sink.command(Command::Original(None, 0), false));
        assert!(sink.submit_stereo(&[123, -456]));
        let locked = sink.shared.controls.lock().unwrap();
        device.begin();
        assert_eq!(device.next(48000), [0.0; 2]);
        assert!(!sink.shared.samples.is_empty());
        drop(locked);
        device.begin();
        assert_eq!(device.next(48000), [123.0 / 32768.0, -456.0 / 32768.0]);
        assert_eq!(budget.used(), 0);
    }
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
                .as_chunks::<2>()
                .0
                .iter()
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
        let contention = Player::new(100).unwrap();
        let locked = contention.sink.shared.controls.lock().unwrap();
        let before = contention.non_silent_frames();
        // Publish PCM while holding the producer/control lock. The actual
        // device callback must consume all of it before this guard is released.
        assert!(contention.sink.shared.samples.push(&[16384; 3200], 2));
        let start = std::time::Instant::now();
        while contention.non_silent_frames() - before < 1600 {
            assert!(start.elapsed() < std::time::Duration::from_secs(5));
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        drop(locked);
        assert!(contention.error().is_none());
        drop(contention);
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
