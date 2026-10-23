//! Shared reservations for bounded game data, released by ownership.
use anyhow::{ensure, Context, Result};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
struct Shared {
    limit: u64,
    used: AtomicU64,
    // Child leases retain this shared owner, and therefore the whole parent
    // capacity charge, even after the child Budget/cache object is dropped.
    _parent: Option<Lease>,
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
            _parent: None,
        }))
    }
    pub fn limit(&self) -> u64 {
        self.0.limit
    }
    pub fn used(&self) -> u64 {
        self.0.used.load(Ordering::Acquire)
    }
    pub(crate) fn owns(&self, lease: &Lease) -> bool {
        Arc::ptr_eq(&self.0, &lease.shared)
    }
    /// Reserve a fixed child capacity inside this same hard limit. Child
    /// reservations consume that capacity, not an independent allowance.
    pub fn partition(&self, limit: u64) -> Result<Self> {
        let parent = self.reserve(limit)?;
        Ok(Self(Arc::new(Shared {
            limit,
            used: AtomicU64::new(0),
            _parent: Some(parent),
        })))
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
    fn child_capacity_is_not_released_until_its_last_lease_owner_drops() {
        let root = Budget::new(16);
        assert!(root.partition(0).is_err());
        assert!(root.partition(17).is_err());
        let child = root.partition(10).unwrap();
        assert_eq!(root.used(), 10);
        assert_eq!(child.used(), 0);
        let nested = child.partition(6).unwrap();
        assert_eq!(child.used(), 6);
        assert_eq!(root.used(), 10);
        let retained = nested.reserve(6).unwrap();
        assert!(nested.reserve(1).is_err());
        let guest = root.reserve(6).unwrap();
        assert!(root.reserve(1).is_err());
        drop(nested);
        drop(child);
        assert_eq!(root.used(), 16);
        drop(guest);
        assert_eq!(root.used(), 10);
        drop(retained);
        assert_eq!(root.used(), 0);
        let all = root.reserve(16).unwrap();
        drop(all);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn concurrent_children_and_live_leases_cannot_overcommit_the_parent() {
        let root = Budget::new(16);
        let ready = Arc::new(std::sync::Barrier::new(5));
        let release = Arc::new(std::sync::Barrier::new(5));
        let workers = (0..4)
            .map(|_| {
                let root = root.clone();
                let ready = ready.clone();
                let release = release.clone();
                std::thread::spawn(move || {
                    let lease = root
                        .partition(6)
                        .ok()
                        .and_then(|child| child.reserve(6).ok());
                    let acquired = lease.is_some();
                    ready.wait();
                    release.wait();
                    drop(lease);
                    acquired
                })
            })
            .collect::<Vec<_>>();
        ready.wait();
        let held = root.used();
        release.wait();
        let acquired = workers
            .into_iter()
            .map(|worker| usize::from(worker.join().unwrap()))
            .sum::<usize>();
        assert_eq!(acquired, 2);
        assert_eq!(held, 12);
        assert_eq!(root.used(), 0);
    }
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
