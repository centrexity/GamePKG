//! Host-owned CPAL input. Game code receives only opaque capture IDs/PCM data.
use crate::port_host::{capture::Capture, Error, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
struct Input {
    _stream: cpal::Stream,
    pub queue: Capture,
}
impl Input {
    fn new(channels: u16, queue: Capture) -> Result<Self> {
        if !(1..=2).contains(&channels) {
            return Err(Error::Unsupported);
        }
        let device = cpal::default_host()
            .default_input_device()
            .ok_or(Error::Unsupported)?;
        let config = device
            .supported_input_configs()
            .map_err(|_| Error::Unsupported)?
            .find(|c| {
                c.channels() == channels
                    && c.min_sample_rate() <= 48000
                    && c.max_sample_rate() >= 48000
                    && matches!(
                        c.sample_format(),
                        cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16
                    )
            })
            .ok_or(Error::Unsupported)?
            .with_sample_rate(48000);
        let stream = match config.sample_format() {
            cpal::SampleFormat::I16 => build(&device, config.config(), queue.clone(), |x: i16| x),
            cpal::SampleFormat::U16 => build(&device, config.config(), queue.clone(), |x: u16| {
                (i32::from(x) - 32768) as i16
            }),
            cpal::SampleFormat::F32 => build(&device, config.config(), queue.clone(), |x: f32| {
                if x.is_finite() {
                    (x.clamp(-1.0, 1.0) * 32768.0).clamp(-32768.0, 32767.0) as i16
                } else {
                    0
                }
            }),
            _ => return Err(Error::Unsupported),
        }?;
        queue.healthy()?;
        stream.play().map_err(|_| Error::Failed)?;
        Ok(Self {
            _stream: stream,
            queue,
        })
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        self.queue.close();
    }
}
use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    mpsc, Arc,
};
static DEVICE_OWNED: AtomicBool = AtomicBool::new(false);
/// At most one asynchronous input device globally. Start/stop never wait on the
/// window owner. Revocation immediately discards data even during device setup.
pub struct Device {
    pub queue: Capture,
    status: Arc<AtomicI32>,
    stop: mpsc::SyncSender<()>,
}
impl Device {
    pub fn new(channels: u16, frames: usize, budget: u64) -> Result<Self> {
        const STACK: usize = 1024 * 1024;
        let queue = Capture::new(
            channels as usize,
            frames,
            budget.checked_sub(STACK as u64 + 4096).ok_or(Error::Busy)?,
        )?;
        DEVICE_OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Error::Busy)?;
        let (stop, receive) = mpsc::sync_channel(1);
        let status = Arc::new(AtomicI32::new(0));
        let worker_status = status.clone();
        let worker_queue = queue.clone();
        let worker = std::thread::Builder::new()
            .name("gamepkg-microphone".into())
            .stack_size(STACK)
            .spawn(move || {
                struct Lease;
                impl Drop for Lease {
                    fn drop(&mut self) {
                        DEVICE_OWNED.store(false, Ordering::Release);
                    }
                }
                let _lease = Lease;
                match Input::new(channels, worker_queue.clone()) {
                    Ok(input) => {
                        worker_status.store(1, Ordering::Release);
                        while receive.recv_timeout(std::time::Duration::from_millis(20))
                            == Err(mpsc::RecvTimeoutError::Timeout)
                            && worker_queue.healthy().is_ok()
                        {}
                        if let Err(error) = worker_queue.healthy() {
                            worker_status.store(error as i32, Ordering::Release);
                        }
                        drop(input);
                    }
                    Err(error) => {
                        worker_status.store(error as i32, Ordering::Release);
                        worker_queue.close();
                    }
                }
            });
        if worker.is_err() {
            DEVICE_OWNED.store(false, Ordering::Release);
            return Err(Error::Failed);
        }
        Ok(Self {
            queue,
            status,
            stop,
        })
    }
    pub fn ready(&self) -> Result<()> {
        match self.status.load(Ordering::Acquire) {
            0 => Err(Error::Busy),
            1 => Ok(()),
            -1 => Err(Error::Invalid),
            -2 => Err(Error::Busy),
            -4 => Err(Error::Denied),
            -5 => Err(Error::Unsupported),
            _ => Err(Error::Failed),
        }
    }
}
impl Drop for Device {
    fn drop(&mut self) {
        self.queue.close();
        let _ = self.stop.try_send(());
    }
}
fn build<T: cpal::SizedSample + 'static>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    queue: Capture,
    convert: fn(T) -> i16,
) -> Result<cpal::Stream> {
    let failure = queue.clone();
    device
        .build_input_stream(
            config,
            move |samples: &[T], _| {
                let mut buffer = [0i16; 3200];
                for packet in samples.chunks(1600 * config.channels as usize) {
                    for (to, from) in buffer.iter_mut().zip(packet) {
                        *to = convert(*from);
                    }
                    if queue.submit(&buffer[..packet.len()]).is_err() {
                        break;
                    }
                }
            },
            move |_| failure.fail(),
            Some(std::time::Duration::from_secs(1)),
        )
        .map_err(|_| Error::Unsupported)
}
