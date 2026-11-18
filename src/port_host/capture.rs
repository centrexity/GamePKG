//! Bounded permission-revocable microphone queue, separate from output audio.
use super::{Error, Result};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
struct State {
    closed: bool,
    failed: bool,
    queue: VecDeque<[i16; 2]>,
    captured: u64,
    read: u64,
    lost: u64,
}
#[derive(Clone)]
pub struct Capture {
    state: Arc<Mutex<State>>,
    channels: usize,
    capacity: usize,
}
impl Capture {
    pub fn new(channels: usize, frames: usize, budget: u64) -> Result<Self> {
        if !(1..=2).contains(&channels) || frames == 0 || frames > 48000 {
            return Err(Error::Invalid);
        }
        if frames as u64 * 4 + 256 > budget {
            return Err(Error::Busy);
        }
        Ok(Self {
            state: Arc::new(Mutex::new(State {
                closed: false,
                failed: false,
                queue: VecDeque::with_capacity(frames),
                captured: 0,
                read: 0,
                lost: 0,
            })),
            channels,
            capacity: frames,
        })
    }
    /// Device provider only. Overflow fails closed and discards pending data;
    /// never pass stale/truncated capture off as a continuous recording.
    pub fn submit(&self, pcm: &[i16]) -> Result<()> {
        if pcm.is_empty() || pcm.len() % self.channels != 0 || pcm.len() / self.channels > 1600 {
            return Err(Error::Invalid);
        }
        let mut s = self.state.lock().map_err(|_| Error::Failed)?;
        if s.closed {
            return Err(Error::Denied);
        }
        if s.failed {
            return Err(Error::Failed);
        }
        let frames = pcm.len() / self.channels;
        if frames > self.capacity - s.queue.len() {
            s.lost = s
                .lost
                .checked_add((frames + s.queue.len()) as u64)
                .ok_or(Error::Failed)?;
            s.queue.clear();
            s.failed = true;
            return Err(Error::Failed);
        }
        s.captured = s.captured.checked_add(frames as u64).ok_or(Error::Failed)?;
        for frame in pcm.chunks_exact(self.channels) {
            s.queue.push_back([frame[0], frame[self.channels - 1]]);
        }
        Ok(())
    }
    pub fn read(&self, frames: usize, capacity: usize) -> Result<Vec<u8>> {
        if frames == 0 || frames > 1600 || capacity < frames * self.channels * 2 {
            return Err(Error::Invalid);
        }
        let mut s = self.state.lock().map_err(|_| Error::Failed)?;
        if s.closed {
            return Err(Error::Denied);
        }
        if s.failed {
            return Err(Error::Failed);
        }
        if s.queue.is_empty() {
            return Err(Error::Busy);
        }
        let count = frames.min(s.queue.len());
        let mut bytes = Vec::with_capacity(count * self.channels * 2);
        for _ in 0..count {
            let frame = s.queue.pop_front().unwrap();
            for sample in &frame[..self.channels] {
                bytes.extend(sample.to_le_bytes());
            }
        }
        s.read += count as u64;
        Ok(bytes)
    }
    pub fn status(&self) -> Result<[u64; 4]> {
        let s = self.state.lock().map_err(|_| Error::Failed)?;
        if s.closed {
            return Err(Error::Denied);
        }
        Ok([s.captured, s.read, s.lost, s.queue.len() as u64])
    }
    pub fn close(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.closed = true;
            s.queue.clear();
        }
    }
    pub fn healthy(&self) -> Result<()> {
        let s = self.state.lock().map_err(|_| Error::Failed)?;
        if s.closed {
            Err(Error::Denied)
        } else if s.failed {
            Err(Error::Failed)
        } else {
            Ok(())
        }
    }
    pub fn fail(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.failed = true;
            s.lost = s.lost.saturating_add(s.queue.len() as u64);
            s.queue.clear();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_bounds_backpressure_overflow_and_revocation_discard_data() {
        let c = Capture::new(2, 2, 4096).unwrap();
        assert_eq!(c.read(1, 4), Err(Error::Busy));
        c.submit(&[1, 2, 3, 4]).unwrap();
        assert_eq!(c.read(2, 4), Err(Error::Invalid));
        assert_eq!(c.status().unwrap(), [2, 0, 0, 2]);
        assert_eq!(c.read(1, 4).unwrap(), [1, 0, 2, 0]);
        assert_eq!(c.status().unwrap(), [2, 1, 0, 1]);
        assert_eq!(c.submit(&[5, 6, 7, 8]), Err(Error::Failed));
        assert_eq!(c.read(1, 4), Err(Error::Failed));
        assert_eq!(c.status().unwrap(), [2, 1, 3, 0]);
        let c = Capture::new(1, 2, 4096).unwrap();
        let provider = c.clone();
        c.submit(&[9]).unwrap();
        c.close();
        assert_eq!(c.read(1, 2), Err(Error::Denied));
        assert_eq!(provider.submit(&[8]), Err(Error::Denied));
        assert!(matches!(Capture::new(6, 1, 4096), Err(Error::Invalid)));
        assert!(matches!(Capture::new(1, 48000, 1), Err(Error::Busy)));
    }
}
