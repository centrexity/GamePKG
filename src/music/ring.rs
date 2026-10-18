//! Safe atomic SPSC blocks: one decoder writer, one guarded output consumer.
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    OnceLock,
};
const BLOCKS: usize = 6;
pub(super) const FRAMES: usize = 800;
struct Block {
    samples: [AtomicU32; FRAMES],
    length: AtomicUsize,
    generation: AtomicU64,
}
impl Block {
    fn new() -> Self {
        Self {
            samples: [const { AtomicU32::new(0) }; FRAMES],
            length: AtomicUsize::new(0),
            generation: AtomicU64::new(0),
        }
    }
}
pub(super) struct Ring {
    blocks: [Block; BLOCKS],
    head: AtomicU64,
    tail: AtomicU64,
    offset: AtomicUsize,
    ready: AtomicUsize,
    busy: AtomicBool,
    worker: OnceLock<std::thread::Thread>,
}
impl Ring {
    pub(super) fn new() -> Self {
        Self {
            blocks: std::array::from_fn(|_| Block::new()),
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            offset: AtomicUsize::new(0),
            ready: AtomicUsize::new(0),
            busy: AtomicBool::new(false),
            worker: OnceLock::new(),
        }
    }
    pub(super) fn set_worker(&self, worker: std::thread::Thread) {
        self.worker.set(worker).expect("one music producer");
        self.wake();
    }
    pub(super) fn wake(&self) {
        if let Some(worker) = self.worker.get() {
            worker.unpark();
        }
    }
    pub(super) fn frames(&self) -> usize {
        self.ready.load(Ordering::Acquire)
    }
    pub(super) fn empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }
    pub(super) fn exhausted(&self) -> bool {
        self.tail.load(Ordering::Relaxed) == u64::MAX
    }
    pub(super) fn can_write(&self) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        tail != u64::MAX
            && tail
                .checked_sub(self.head.load(Ordering::Acquire))
                .is_some_and(|used| used < BLOCKS as u64)
    }
    pub(super) fn push(&self, generation: u64, samples: &[i16]) -> bool {
        if samples.is_empty()
            || samples.len() > FRAMES * 2
            || !samples.len().is_multiple_of(2)
            || !self.can_write()
        {
            return false;
        }
        let tail = self.tail.load(Ordering::Relaxed);
        let block = &self.blocks[(tail % BLOCKS as u64) as usize];
        for (slot, frame) in block.samples.iter().zip(samples.chunks_exact(2)) {
            slot.store(
                u32::from(frame[0] as u16) | (u32::from(frame[1] as u16) << 16),
                Ordering::Relaxed,
            );
        }
        block.length.store(samples.len() / 2, Ordering::Relaxed);
        block.generation.store(generation, Ordering::Relaxed);
        // Publish the count before the block: a consumer never subtracts an
        // unpublished count. Only tail publication makes these samples readable.
        self.ready.fetch_add(samples.len() / 2, Ordering::Release);
        self.tail.store(tail + 1, Ordering::Release);
        true
    }
    pub(super) fn consumer(&self) -> Option<Consumer<'_>> {
        self.busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Consumer {
                ring: self,
                freed: false,
            })
    }
    pub(super) fn wait_consumer(&self) {
        while self.busy.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }
}
pub(super) struct Consumer<'a> {
    ring: &'a Ring,
    freed: bool,
}
impl Consumer<'_> {
    fn release(&mut self, head: u64) {
        self.ring.offset.store(0, Ordering::Relaxed);
        self.ring.head.store(
            head.checked_add(1).expect("music ring position invariant"),
            Ordering::Release,
        );
        self.freed = true;
    }
    pub(super) fn discard_through(&mut self, generation: u64, include_current: bool) {
        loop {
            let head = self.ring.head.load(Ordering::Relaxed);
            if head == self.ring.tail.load(Ordering::Acquire) {
                return;
            }
            let block = &self.ring.blocks[(head % BLOCKS as u64) as usize];
            let tag = block.generation.load(Ordering::Relaxed);
            if tag > generation || (!include_current && tag == generation) {
                return;
            }
            let remaining =
                block.length.load(Ordering::Relaxed) - self.ring.offset.load(Ordering::Relaxed);
            self.ring.ready.fetch_sub(remaining, Ordering::Release);
            self.release(head);
        }
    }
    pub(super) fn next(&mut self, generation: u64) -> Option<[i16; 2]> {
        self.discard_through(generation, false);
        let head = self.ring.head.load(Ordering::Relaxed);
        if head == self.ring.tail.load(Ordering::Acquire) {
            return None;
        }
        let block = &self.ring.blocks[(head % BLOCKS as u64) as usize];
        if block.generation.load(Ordering::Relaxed) != generation {
            return None;
        }
        let offset = self.ring.offset.load(Ordering::Relaxed);
        let sample = block.samples[offset].load(Ordering::Relaxed);
        self.ring.ready.fetch_sub(1, Ordering::Release);
        if offset + 1 == block.length.load(Ordering::Relaxed) {
            self.release(head);
        } else {
            self.ring.offset.store(offset + 1, Ordering::Relaxed);
        }
        Some([sample as u16 as i16, (sample >> 16) as u16 as i16])
    }
}
impl Drop for Consumer<'_> {
    fn drop(&mut self) {
        self.ring.busy.store(false, Ordering::Release);
        if self.freed {
            self.ring.wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn full_invalid_stale_future_and_exhausted_blocks_preserve_bounds_and_order() {
        let ring = Ring::new();
        assert!(!ring.push(1, &[]));
        assert!(!ring.push(1, &[1]));
        assert!(!ring.push(1, &[1; 1602]));
        for _ in 0..BLOCKS {
            assert!(ring.push(1, &[i16::MIN, i16::MAX, -1, 0]));
        }
        assert!(!ring.push(1, &[123, -456]));
        assert_eq!(ring.frames(), BLOCKS * 2);
        let mut consumer = ring.consumer().unwrap();
        assert!(ring.consumer().is_none());
        for _ in 0..BLOCKS {
            assert_eq!(consumer.next(1), Some([i16::MIN, i16::MAX]));
            assert_eq!(consumer.next(1), Some([-1, 0]));
        }
        assert!(consumer.next(1).is_none());
        drop(consumer);
        assert_eq!(ring.frames(), 0);
        assert!(ring.push(1, &[11, -11, 12, -12]));
        assert!(ring.push(2, &[21, -21]));
        let mut consumer = ring.consumer().unwrap();
        assert_eq!(consumer.next(1), Some([11, -11]));
        assert_eq!(consumer.next(2), Some([21, -21]));
        assert!(ring.push(3, &[31, -31]));
        consumer.discard_through(2, true);
        assert!(consumer.next(2).is_none());
        assert_eq!(ring.frames(), 1);
        assert_eq!(consumer.next(3), Some([31, -31]));
        drop(consumer);
        ring.head.store(u64::MAX, Ordering::Relaxed);
        ring.tail.store(u64::MAX, Ordering::Relaxed);
        assert!(ring.exhausted());
        assert!(!ring.push(4, &[1, 2]));
        assert!(ring.consumer().unwrap().next(4).is_none());
    }
    #[test]
    fn concurrent_blocks_keep_every_stereo_sample_without_overwriting_unread_data() {
        let ring = Arc::new(Ring::new());
        let writer = ring.clone();
        let producer = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let mut block = [0i16; FRAMES * 2];
            for batch in 0..1024 {
                let count = if batch % 17 == 0 { 113 } else { FRAMES };
                for (index, frame) in block[..count * 2].chunks_exact_mut(2).enumerate() {
                    frame[0] = ((batch * 13 + index) % 32768) as i16;
                    frame[1] = -frame[0];
                }
                while !writer.push(7, &block[..count * 2]) {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "music producer stalled"
                    );
                    std::thread::yield_now();
                }
                assert!(writer.frames() <= BLOCKS * FRAMES);
            }
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        for batch in 0..1024 {
            let count = if batch % 17 == 0 { 113 } else { FRAMES };
            let mut index = 0;
            while index < count {
                let mut consumer = ring.consumer().unwrap();
                while index < count {
                    let Some(frame) = consumer.next(7) else {
                        break;
                    };
                    let expected = ((batch * 13 + index) % 32768) as i16;
                    assert_eq!(frame, [expected, -expected]);
                    index += 1;
                }
                drop(consumer);
                assert!(ring.frames() <= BLOCKS * FRAMES);
                if index < count {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "music consumer stalled"
                    );
                    std::thread::yield_now();
                }
            }
        }
        producer.join().unwrap();
        assert!(ring.empty());
        assert_eq!(ring.frames(), 0);
    }
}
