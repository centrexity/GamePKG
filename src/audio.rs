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
const MAX_QUEUED: usize = 4800; // At most 100 ms of mono input at 48 kHz.
struct Buffer {
    samples: VecDeque<i16>,
    phase: f64,
    current: i16,
    gain: f32,
}
impl Buffer {
    fn new(volume: u8) -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_QUEUED),
            phase: 1.0,
            current: 0,
            gain: volume as f32 / 100.0,
        }
    }
    fn append(&mut self, samples: &[i16]) -> bool {
        if samples.len() > 1600 || self.samples.len() + samples.len() > MAX_QUEUED {
            return false;
        }
        self.samples.extend(samples);
        true
    }
    fn next(&mut self, rate: u32) -> f32 {
        while self.phase >= 1.0 {
            self.current = self.samples.pop_front().unwrap_or(0);
            self.phase -= 1.0;
        }
        self.phase += 48000.0 / rate as f64;
        self.current as f32 / 32768.0 * self.gain
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
}
impl Player {
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
    pub fn clear(&self) {
        if let Ok(mut b) = self.sink.buffer.lock() {
            b.samples.clear();
            b.current = 0;
        }
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|mut e| e.take())
    }
    pub fn non_silent_frames(&self) -> u64 {
        self.non_silent.load(Ordering::Relaxed)
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
                if sample != 0.0 {
                    audible += 1;
                }
                frame.fill(T::from_sample(sample));
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
    fn queue_volume_silence_and_resampling_are_bounded() {
        let mut buffer = Buffer::new(50);
        assert!(buffer.append(&[16384, -16384]));
        assert_eq!(buffer.next(48000), 0.25);
        assert_eq!(buffer.next(48000), -0.25);
        assert_eq!(buffer.next(48000), 0.0);
        let mut muted = Buffer::new(0);
        assert!(muted.append(&[32767]));
        assert_eq!(muted.next(48000), 0.0);
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
        assert_eq!(upsample.next(96000), 0.5);
        assert_eq!(upsample.next(96000), 0.5);
        assert_eq!(upsample.next(96000), -0.5);
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
    }
}
