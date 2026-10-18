//! Shared reservations for bounded game data, released by ownership.
use anyhow::{ensure, Context, Result};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
struct Shared {
    limit: u64,
    used: AtomicU64,
}
#[derive(Clone)]
pub struct Budget(Arc<Shared>);
pub struct Lease {
    shared: Arc<Shared>,
    bytes: u64,
}
impl Budget {
    pub fn new(limit: u64) -> Self {
        Self(Arc::new(Shared {
            limit,
            used: AtomicU64::new(0),
        }))
    }
    pub fn limit(&self) -> u64 {
        self.0.limit
    }
    pub fn used(&self) -> u64 {
        self.0.used.load(Ordering::Acquire)
    }
    pub fn reserve(&self, bytes: u64) -> Result<Lease> {
        ensure!(
            bytes > 0 && bytes <= self.0.limit,
            "invalid memory reservation"
        );
        let mut used = self.0.used.load(Ordering::Acquire);
        loop {
            let next = used
                .checked_add(bytes)
                .filter(|next| *next <= self.0.limit)
                .context("game memory budget exceeded")?;
            match self
                .0
                .used
                .compare_exchange_weak(used, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(current) => used = current,
            }
        }
        Ok(Lease {
            shared: self.0.clone(),
            bytes,
        })
    }
}
impl Lease {
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        // The audio callback may release the last clip owner. It must never
        // wait for a host reservation lock while freeing that recording.
        let previous = self.shared.used.fetch_sub(self.bytes, Ordering::AcqRel);
        assert!(previous >= self.bytes, "memory reservation invariant");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_reservations_release_on_move_drop_and_failure() {
        let budget = Budget::new(16);
        assert!(budget.reserve(0).is_err());
        assert!(budget.reserve(17).is_err());
        let guest = budget.reserve(10).unwrap();
        let copy = budget.clone();
        let music = copy.reserve(6).unwrap();
        assert_eq!(music.bytes(), 6);
        assert_eq!(budget.used(), 16);
        assert!(copy.reserve(1).is_err());
        drop(music);
        assert_eq!(budget.used(), 10);
        let replacement = copy.reserve(6).unwrap();
        drop(guest);
        drop(replacement);
        assert_eq!(budget.used(), 0);
        let huge = Budget::new(u64::MAX);
        let whole = huge.reserve(u64::MAX).unwrap();
        assert!(huge.reserve(1).is_err());
        drop(whole);
        assert_eq!(huge.used(), 0);
    }
    #[test]
    fn concurrent_reservations_cannot_overcommit() {
        let budget = Budget::new(3);
        let ready = Arc::new(std::sync::Barrier::new(5));
        let release = Arc::new(std::sync::Barrier::new(5));
        let workers = (0..4)
            .map(|_| {
                let budget = budget.clone();
                let ready = ready.clone();
                let release = release.clone();
                std::thread::spawn(move || {
                    let lease = budget.reserve(1).ok();
                    let acquired = lease.is_some();
                    ready.wait();
                    release.wait();
                    drop(lease);
                    acquired
                })
            })
            .collect::<Vec<_>>();
        ready.wait();
        assert_eq!(budget.used(), 3);
        release.wait();
        assert_eq!(
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .filter(|acquired| *acquired)
                .count(),
            3
        );
        assert_eq!(budget.used(), 0);
    }
}
