//! Fixed-storage stereo queue: producers are serialized by the host control
//! lock; exactly one device callback owns consumption. Neither reader waits.
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
pub(super) const CAPACITY: usize = 4800;
pub(super) struct Queue {
    frames: [AtomicU32; CAPACITY],
    write: AtomicU64,
    read: AtomicU64,
}
impl Queue {
    pub fn new() -> Self {
        Self {
            frames: std::array::from_fn(|_| AtomicU32::new(0)),
            write: AtomicU64::new(0),
            read: AtomicU64::new(0),
        }
    }
    pub fn position(&self) -> u64 {
        self.write.load(Ordering::Acquire)
    }
    pub fn push(&self, samples: &[i16], channels: usize) -> bool {
        if !(1..=2).contains(&channels)
            || !samples.len().is_multiple_of(channels)
            || samples.len() / channels > 1600
        {
            return false;
        }
        let count = samples.len() / channels;
        let write = self.write.load(Ordering::Relaxed);
        let read = self.read.load(Ordering::Acquire);
        let Some(end) = write.checked_add(count as u64) else {
            return false;
        };
        if end - read > CAPACITY as u64 {
            return false;
        }
        for (offset, frame) in samples.chunks_exact(channels).enumerate() {
            let left = frame[0] as u16;
            let right = frame[channels - 1] as u16;
            self.frames[((write + offset as u64) % CAPACITY as u64) as usize].store(
                u32::from(left) | (u32::from(right) << 16),
                Ordering::Relaxed,
            );
        }
        self.write.store(end, Ordering::Release);
        true
    }
    pub fn pop(&self) -> Option<[i16; 2]> {
        let read = self.read.load(Ordering::Relaxed);
        if read == self.write.load(Ordering::Acquire) {
            return None;
        }
        let frame = self.frames[(read % CAPACITY as u64) as usize].load(Ordering::Relaxed);
        self.read.store(read + 1, Ordering::Release);
        Some([frame as u16 as i16, (frame >> 16) as u16 as i16])
    }
    /// Only the consumer calls this. A command captures the producer position,
    /// so clearing cannot discard packets submitted after that command.
    pub fn discard_through(&self, position: u64) {
        let read = self.read.load(Ordering::Relaxed);
        let end = position.min(self.write.load(Ordering::Acquire));
        if end > read {
            self.read.store(end, Ordering::Release);
        }
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.read.load(Ordering::Acquire) == self.position()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packet_bounds_stereo_clear_boundaries_wrap_and_exhaustion() {
        let queue = Queue::new();
        assert!(!queue.push(&[1], 0));
        assert!(!queue.push(&[1], 2));
        assert!(!queue.push(&[1; 1601], 1));
        for _ in 0..3 {
            assert!(queue.push(&[7; 1600], 1));
        }
        assert!(!queue.push(&[8], 1));
        for _ in 0..3000 {
            assert_eq!(queue.pop(), Some([7, 7]));
        }
        assert!(queue.push(&[123, -456, -32768, 32767], 2));
        let boundary = queue.position();
        assert!(queue.push(&[9, -10], 2));
        queue.discard_through(boundary);
        assert_eq!(queue.pop(), Some([9, -10]));
        assert_eq!(queue.pop(), None);
        queue.discard_through(0);
        assert!(queue.is_empty());
        queue.read.store(u64::MAX, Ordering::Relaxed);
        queue.write.store(u64::MAX, Ordering::Relaxed);
        assert!(!queue.push(&[1], 1));
        assert_eq!(queue.pop(), None);
    }
    #[test]
    fn concurrent_packets_preserve_every_stereo_frame_across_ring_wraps() {
        let queue = std::sync::Arc::new(Queue::new());
        let producer = queue.clone();
        let writer = std::thread::spawn(move || {
            let start = std::time::Instant::now();
            for block in 0..1024 {
                let frames = (0..113)
                    .flat_map(|frame| {
                        let value = (block * 113 + frame) as i16;
                        [value, !value]
                    })
                    .collect::<Vec<_>>();
                while !producer.push(&frames, 2) {
                    assert!(start.elapsed() < std::time::Duration::from_secs(3));
                    std::thread::yield_now();
                }
            }
        });
        let start = std::time::Instant::now();
        for frame in 0..1024 * 113 {
            let value = frame as i16;
            loop {
                if let Some(actual) = queue.pop() {
                    assert_eq!(actual, [value, !value]);
                    break;
                }
                assert!(start.elapsed() < std::time::Duration::from_secs(3));
                std::thread::yield_now();
            }
        }
        writer.join().unwrap();
        assert!(queue.is_empty());
    }
}
