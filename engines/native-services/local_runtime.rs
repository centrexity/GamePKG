//! Trusted process-local facilities. Game code sees generated numeric-handle
//! bindings, not Runtime, function pointers, OS handles or native addresses.
#[path = "local_memory.rs"]
mod memory;
#[path = "performance_clock.rs"]
pub(crate) mod performance;
#[path = "local_timing.rs"]
pub mod timing;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{JoinHandle, ThreadId},
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Invalid = -1,
    Busy = -2,
    Failed = -3,
    Denied = -4,
    Unsupported = -5,
}
pub type Result<T> = std::result::Result<T, Error>;
static NEXT: AtomicU64 = AtomicU64::new(1);
#[allow(deprecated)] // Rust 1.89 has fetch_update, before the try_update rename.
fn id() -> Result<u64> {
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
        .map_err(|_| Error::Failed)
}
/// Bootstrap-owned layout. No game-selected base or executable permission.
#[derive(Clone, Copy)]
pub struct Window {
    pub address: u64,
    pub bytes: u64,
}
pub type Entry = fn(TaskContext, u64) -> u64;
struct Region {
    map: memory::Mapping,
    pages: Vec<u8>,
    bytes: u64,
}
struct Job {
    cancel: Arc<AtomicBool>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    handle: Arc<Mutex<Option<JoinHandle<u64>>>>,
}
enum Kind {
    Event {
        manual: bool,
        signaled: bool,
    },
    Mutex {
        owner: Option<ThreadId>,
        depth: u32,
        abandoned: bool,
    },
    Semaphore {
        count: u32,
        maximum: u32,
    },
}
struct Sync {
    value: Mutex<Kind>,
    changed: Condvar,
    closed: AtomicBool,
}
const MAILBOX_BYTES: usize = 1024 * 1024;
#[derive(Default)]
struct MailboxBudget {
    bytes: AtomicUsize,
    slots: AtomicUsize,
}
struct Mailbox {
    messages: Mutex<VecDeque<Vec<u8>>>,
    changed: Condvar,
    closed: AtomicBool,
    maximum: usize,
    depth: usize,
    budget: Arc<MailboxBudget>,
}
impl Drop for Mailbox {
    fn drop(&mut self) {
        // A released mailbox can still have a waking receiver. Its capacity
        // stays charged until that final reference, including the queue, drops.
        let queue = self.messages.get_mut().unwrap_or_else(|e| e.into_inner());
        queue.clear();
        queue.shrink_to_fit();
        self.budget
            .bytes
            .fetch_sub(self.maximum * self.depth, Ordering::AcqRel);
        self.budget.slots.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Record {
    version: u64,
    bytes: Vec<u8>,
}
const RECORD_BYTES: usize = 65536;
const RECORD_COUNT: usize = 64;
const RECORD_MAX: usize = 4096;
#[derive(Clone, Copy, PartialEq, Eq)]
enum WaitKind {
    Any,
    Mutex,
    Event,
    Semaphore,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventState {
    pub manual: bool,
    pub signaled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemaphoreState {
    pub count: u32,
    pub maximum: u32,
}
/// Non-consuming mutex observation. A copied state grants no ownership/lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MutexState {
    pub locked: bool,
    pub owned_by_caller: bool,
    pub abandoned: bool,
    pub depth: u32,
}
/// Numeric kinds are checked against the actual retained object, not trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitObject {
    pub id: u64,
    pub kind: u32, // 1 event, 2 mutex, 3 semaphore, 4 reviewed task completion
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitResult {
    pub index: u32,
    pub abandoned: bool,
}
enum WaitView {
    Sync(Arc<Sync>),
    Task(Arc<Mutex<Option<JoinHandle<u64>>>>),
}
enum WaitGuard<'a> {
    Sync(std::sync::MutexGuard<'a, Kind>),
    Task(std::sync::MutexGuard<'a, Option<JoinHandle<u64>>>),
}
// Runs after entry return/unwind and before actual thread completion. Never
// invokes game code; completion waits still inspect JoinHandle::is_finished.
struct TaskExit(Runtime);
impl Drop for TaskExit {
    fn drop(&mut self) {
        self.0.task_exited();
    }
}
struct State {
    records: BTreeMap<u64, Record>,
    record_bytes: usize,
    regions: BTreeMap<u64, Region>,
    jobs: BTreeMap<u64, Job>,
    sync: BTreeMap<u64, Arc<Sync>>,
    mailboxes: BTreeMap<u64, Arc<Mailbox>>,
}
struct Inner {
    state: Mutex<State>,
    closed: AtomicBool,
    entries: Vec<Entry>,
    stack_bytes: usize,
    task_limit: usize,
    mailbox_budget: Arc<MailboxBudget>,
    clock: timing::Clock,
    wait_changed: (Mutex<u64>, Condvar),
}
pub struct Runtime(Arc<Inner>, bool);
impl Clone for Runtime {
    fn clone(&self) -> Self {
        Self(self.0.clone(), false)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        if self.1 {
            let _ = self.close();
        }
    }
}
pub struct TaskContext {
    runtime: Runtime,
    cancel: Arc<AtomicBool>,
}
impl TaskContext {
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Acquire) || self.runtime.0.closed.load(Ordering::Acquire)
    }
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
}
impl Runtime {
    // No path takes this epoch lock and then an object/state lock. Mutators may
    // notify while holding a value lock; waiters snapshot, unlock, inspect all
    // values, unlock them, then compare-and-sleep under the epoch lock.
    fn notify_waiters(&self) {
        if let Ok(mut generation) = self.0.wait_changed.0.lock() {
            if let Some(next) = generation.checked_add(1) {
                *generation = next;
            } else {
                self.0.closed.store(true, Ordering::Release);
            }
        } else {
            self.0.closed.store(true, Ordering::Release);
        }
        self.0.wait_changed.1.notify_all();
    }
    fn task_exited(&self) {
        let objects = match self.0.state.lock() {
            Ok(state) => state.sync.values().cloned().collect::<Vec<_>>(),
            Err(_) => {
                self.notify_waiters();
                return;
            }
        };
        let thread = std::thread::current().id();
        for object in objects {
            if let Ok(mut value) = object.value.lock() {
                if let Kind::Mutex {
                    owner,
                    depth,
                    abandoned,
                } = &mut *value
                {
                    if *owner == Some(thread) {
                        *owner = None;
                        *depth = 0;
                        *abandoned = true;
                        object.changed.notify_all();
                    }
                }
            }
        }
        self.notify_waiters();
    }
    /// Bounded child-worker wait over 1..64 distinct, typed opaque IDs. All
    /// acquisitions are committed together; failure never partially consumes.
    /// Task completion is a retained observation, never a consuming join.
    pub fn wait_many(
        &self,
        objects: &[WaitObject],
        all: bool,
        milliseconds: u32,
        worker: bool,
    ) -> Result<WaitResult> {
        if !worker {
            return Err(Error::Busy);
        }
        if objects.is_empty() || objects.len() > 64 || milliseconds > 1000 {
            return Err(Error::Invalid);
        }
        for (index, object) in objects.iter().enumerate() {
            if object.id == 0
                || !(1..=4).contains(&object.kind)
                || objects[..index].iter().any(|old| old.id == object.id)
            {
                return Err(Error::Invalid);
            }
        }
        let cancelled = self.task_cancellation()?;
        let mut views = Vec::with_capacity(objects.len());
        {
            let state = self.0.state.lock().map_err(|_| Error::Failed)?;
            self.open()?;
            for (index, object) in objects.iter().enumerate() {
                let view = if object.kind == 4 {
                    WaitView::Task(
                        state
                            .jobs
                            .get(&object.id)
                            .ok_or(Error::Denied)?
                            .handle
                            .clone(),
                    )
                } else {
                    WaitView::Sync(state.sync.get(&object.id).cloned().ok_or(Error::Denied)?)
                };
                views.push((object.id, index, object.kind, view));
            }
        }
        // Every concurrent set takes the same total order, even for mixed kinds
        // and reversed caller arrays. Never take runtime.state while holding it.
        views.sort_by_key(|view| view.0);
        let has_tasks = objects.iter().any(|object| object.kind == 4);
        let end = Instant::now() + Duration::from_millis(milliseconds.into());
        let thread = std::thread::current().id();
        loop {
            let before = *self.0.wait_changed.0.lock().map_err(|_| Error::Failed)?;
            let mut guards = Vec::with_capacity(views.len());
            for (_, _, _, view) in &views {
                guards.push(match view {
                    WaitView::Sync(object) => {
                        WaitGuard::Sync(object.value.lock().map_err(|_| Error::Failed)?)
                    }
                    WaitView::Task(handle) => {
                        WaitGuard::Task(handle.lock().map_err(|_| Error::Failed)?)
                    }
                });
            }
            self.open()?;
            if cancelled
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Acquire))
            {
                return Err(Error::Denied);
            }
            let mut ready = [false; 64];
            for ((_, index, kind, view), guard) in views.iter().zip(&guards) {
                ready[*index] = match (view, guard) {
                    (WaitView::Sync(object), WaitGuard::Sync(value)) => {
                        if object.closed.load(Ordering::Acquire) {
                            return Err(Error::Denied);
                        }
                        match (*kind, &**value) {
                            (1, Kind::Event { signaled, .. }) => *signaled,
                            (2, Kind::Mutex { owner, .. }) => {
                                owner.is_none() || *owner == Some(thread)
                            }
                            (3, Kind::Semaphore { count, .. }) => *count != 0,
                            _ => return Err(Error::Invalid),
                        }
                    }
                    (WaitView::Task(_), WaitGuard::Task(handle)) => {
                        handle.as_ref().ok_or(Error::Denied)?.is_finished()
                    }
                    _ => return Err(Error::Failed),
                };
            }
            let chosen = if all {
                ready[..objects.len()]
                    .iter()
                    .all(|ready| *ready)
                    .then_some(0)
            } else {
                ready[..objects.len()].iter().position(|ready| *ready)
            };
            if let Some(chosen) = chosen {
                // Preflight every selected recursive acquisition before the
                // first event reset/counter decrement/ownership change.
                for ((_, index, _, _), guard) in views.iter().zip(&guards) {
                    if all || *index == chosen {
                        if let WaitGuard::Sync(value) = guard {
                            if let Kind::Mutex { depth, .. } = &**value {
                                depth.checked_add(1).ok_or(Error::Busy)?;
                            }
                        }
                    }
                }
                let mut abandoned_index = None;
                for ((_, index, _, _), guard) in views.iter().zip(&mut guards) {
                    if !all && *index != chosen {
                        continue;
                    }
                    if let WaitGuard::Sync(value) = guard {
                        match &mut **value {
                            Kind::Event { manual, signaled } => {
                                if !*manual {
                                    *signaled = false;
                                }
                            }
                            Kind::Semaphore { count, .. } => {
                                *count -= 1;
                            }
                            Kind::Mutex {
                                owner,
                                depth,
                                abandoned,
                            } => {
                                if *abandoned {
                                    abandoned_index = Some(
                                        abandoned_index
                                            .map_or(*index, |old: usize| old.min(*index)),
                                    );
                                }
                                *abandoned = false;
                                *owner = Some(thread);
                                *depth += 1;
                            }
                        }
                    }
                }
                return Ok(WaitResult {
                    index: abandoned_index.unwrap_or(chosen) as u32,
                    abandoned: abandoned_index.is_some(),
                });
            }
            drop(guards);
            let now = Instant::now();
            if now >= end {
                return Err(Error::Busy);
            }
            let generation = self.0.wait_changed.0.lock().map_err(|_| Error::Failed)?;
            // Lock contention/scheduling must not extend the requested sleep.
            let now = Instant::now();
            if now >= end {
                return Err(Error::Busy);
            }
            if *generation == before {
                // Native JoinHandle completion happens after the exit notifier.
                // A bounded sleeping recheck covers that final host-only tail;
                // no game result/flag is substituted for actual completion.
                let duration = if has_tasks {
                    (end - now).min(Duration::from_millis(1))
                } else {
                    end - now
                };
                let _next = self
                    .0
                    .wait_changed
                    .1
                    .wait_timeout(generation, duration)
                    .map_err(|_| Error::Failed)?;
            }
        }
    }
    /// Minimum charge for the fixed windows, bookkeeping and worker stacks.
    pub fn required_budget(
        windows: &[Window],
        task_limit: usize,
        stack_bytes: usize,
    ) -> Result<u64> {
        if windows.len() > 32
            || task_limit > 32
            || !(65536..=8 * 1024 * 1024).contains(&stack_bytes)
        {
            return Err(Error::Invalid);
        }
        let mut charge = (task_limit as u64)
            .checked_mul(stack_bytes as u64)
            .ok_or(Error::Invalid)?;
        // Bounded bookkeeping, wait objects, task records and transfer buffers.
        // Address reservations and thread stacks alone undercount this runtime.
        charge = charge.checked_add(2 * 1024 * 1024).ok_or(Error::Invalid)?;
        // Separate bounded mailbox capacity and queue bookkeeping reservation.
        charge = charge
            .checked_add(MAILBOX_BYTES as u64 + 65536)
            .ok_or(Error::Invalid)?;
        // Fixed, separate quota for opaque atomic records and their metadata.
        charge = charge
            .checked_add(RECORD_BYTES as u64 + 8192)
            .ok_or(Error::Invalid)?;
        let mut ranges = Vec::new();
        for window in windows {
            charge = charge.checked_add(window.bytes).ok_or(Error::Invalid)?;
            charge = charge
                .checked_add(window.bytes / memory::PAGE)
                .ok_or(Error::Invalid)?;
            let end = window
                .address
                .checked_add(window.bytes)
                .ok_or(Error::Invalid)?;
            if window.bytes == 0
                || end > 0x1_0000_0000
                || ranges.iter().any(|&(a, b)| window.address < b && end > a)
            {
                return Err(Error::Invalid);
            }
            ranges.push((window.address, end));
        }
        Ok(charge)
    }

    /// Reserve every fixed window before any game runs. On partial failure,
    /// already-created reservations are dropped.
    pub fn new(
        windows: &[Window],
        entries: Vec<Entry>,
        budget: u64,
        task_limit: usize,
        stack_bytes: usize,
    ) -> Result<Self> {
        if entries.len() > 256 {
            return Err(Error::Invalid);
        }
        if Self::required_budget(windows, task_limit, stack_bytes)? > budget {
            return Err(Error::Busy);
        }
        let mut regions = BTreeMap::new();
        for window in windows {
            let map =
                memory::Mapping::reserve(window.address, window.bytes).map_err(|e| {
                    match e.kind() {
                        std::io::ErrorKind::Unsupported => Error::Unsupported,
                        std::io::ErrorKind::InvalidInput => Error::Invalid,
                        _ => Error::Busy,
                    }
                })?;
            regions.insert(
                id()?,
                Region {
                    map,
                    pages: vec![0; (window.bytes / memory::PAGE) as usize],
                    bytes: window.bytes,
                },
            );
        }
        Ok(Self(
            Arc::new(Inner {
                state: Mutex::new(State {
                    regions,
                    records: BTreeMap::new(),
                    record_bytes: 0,
                    jobs: BTreeMap::new(),
                    sync: BTreeMap::new(),
                    mailboxes: BTreeMap::new(),
                }),
                closed: AtomicBool::new(false),
                entries,
                stack_bytes,
                task_limit,
                mailbox_budget: Arc::new(MailboxBudget::default()),
                clock: timing::Clock::new(),
                wait_changed: (Mutex::new(0), Condvar::new()),
            }),
            true,
        ))
    }
    fn open(&self) -> Result<()> {
        if self.0.closed.load(Ordering::Acquire) {
            Err(Error::Denied)
        } else {
            Ok(())
        }
    }
    /// Nonblocking clock read independent of the private broker connection.
    /// Cancellation/closure denies new reads; an already accepted read may finish.
    pub fn performance_sample(&self) -> Result<[u64; 3]> {
        if self.current_task()?.1 {
            return Err(Error::Denied);
        }
        let sample = performance::sample()?;
        Ok([sample.counter, sample.frequency, sample.uptime_ms])
    }
    fn task_cancellation(&self) -> Result<Option<Arc<AtomicBool>>> {
        self.open()?;
        let thread = std::thread::current().id();
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        for job in state.jobs.values() {
            if job
                .handle
                .lock()
                .map_err(|_| Error::Failed)?
                .as_ref()
                .is_some_and(|h| h.thread().id() == thread)
            {
                return Ok(Some(job.cancel.clone()));
            }
        }
        Ok(None)
    }
    pub fn clock_snapshot(&self) -> Result<timing::Snapshot> {
        if self.current_task()?.1 {
            return Err(Error::Denied);
        }
        self.0.clock.snapshot()
    }
    pub fn clock_wait(
        &self,
        deadline: u64,
        frequency: u64,
        generation: u64,
        worker: bool,
    ) -> Result<timing::WaitResult> {
        if !worker {
            return Err(Error::Busy);
        }
        let cancellation = self.task_cancellation()?;
        self.0
            .clock
            .wait(deadline, frequency, generation, cancellation, worker)
    }
    pub fn clock_wake(&self) -> Result<u64> {
        if self.current_task()?.1 {
            return Err(Error::Denied);
        }
        self.0.clock.wake()
    }
    /// Owned byte records, no pointers or game callbacks. Versioned complete
    /// snapshots permit source-checked games to update shared values without
    /// importing a platform mutex. Copying an ID does not extend its lifetime.
    pub fn record_create(&self, bytes: &[u8]) -> Result<u64> {
        self.open()?;
        if bytes.is_empty() || bytes.len() > RECORD_MAX {
            return Err(Error::Invalid);
        }
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        if state.records.len() >= RECORD_COUNT || state.record_bytes + bytes.len() > RECORD_BYTES {
            return Err(Error::Busy);
        }
        let record = id()?;
        state.records.insert(
            record,
            Record {
                version: 0,
                bytes: bytes.to_vec(),
            },
        );
        state.record_bytes += bytes.len();
        Ok(record)
    }
    pub fn record_snapshot(&self, record: u64, capacity: usize) -> Result<(u64, Vec<u8>)> {
        self.open()?;
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let value = state.records.get(&record).ok_or(Error::Denied)?;
        if capacity < value.bytes.len() {
            return Err(Error::Invalid);
        }
        Ok((value.version, value.bytes.clone()))
    }
    pub fn record_compare_exchange(&self, record: u64, version: u64, bytes: &[u8]) -> Result<u64> {
        self.open()?;
        if bytes.is_empty() || bytes.len() > RECORD_MAX {
            return Err(Error::Invalid);
        }
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let value = state.records.get_mut(&record).ok_or(Error::Denied)?;
        if bytes.len() != value.bytes.len() {
            return Err(Error::Invalid);
        }
        if version != value.version {
            return Err(Error::Busy);
        }
        let next = version.checked_add(1).ok_or(Error::Failed)?;
        value.bytes.copy_from_slice(bytes);
        value.version = next;
        Ok(next)
    }
    pub fn record_release(&self, record: u64) -> Result<()> {
        self.open()?;
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let value = state.records.remove(&record).ok_or(Error::Denied)?;
        state.record_bytes -= value.bytes.len();
        Ok(())
    }
    pub fn windows(&self) -> Result<Vec<(u64, u64)>> {
        self.open()?;
        Ok(self
            .0
            .state
            .lock()
            .map_err(|_| Error::Failed)?
            .regions
            .iter()
            .map(|(id, r)| (*id, r.bytes))
            .collect())
    }
    fn pages(region: &Region, offset: u64, bytes: u64) -> Result<std::ops::Range<usize>> {
        if bytes == 0
            || !offset.is_multiple_of(memory::PAGE)
            || !bytes.is_multiple_of(memory::PAGE)
            || offset
                .checked_add(bytes)
                .is_none_or(|end| end > region.bytes)
        {
            return Err(Error::Invalid);
        }
        Ok((offset / memory::PAGE) as usize..((offset + bytes) / memory::PAGE) as usize)
    }
    /// mode: 1 read-only, 2 read-write. Fresh pages are zero. Existing commits
    /// cannot be overwritten through commit; use protect or decommit explicitly.
    pub fn commit(&self, handle: u64, offset: u64, bytes: u64, mode: u32) -> Result<()> {
        self.open()?;
        if !(1..=2).contains(&mode) {
            return Err(Error::Invalid);
        }
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let r = state.regions.get_mut(&handle).ok_or(Error::Denied)?;
        let pages = Self::pages(r, offset, bytes)?;
        if r.pages[pages.clone()].iter().any(|p| *p != 0) {
            return Err(Error::Denied);
        }
        r.map
            .commit(offset, bytes, mode)
            .map_err(|_| Error::Failed)?;
        r.pages[pages].fill(mode as u8 + 1);
        Ok(())
    }
    /// mode 0 no-access, 1 read-only, 2 read-write; never executable.
    pub fn protect(&self, handle: u64, offset: u64, bytes: u64, mode: u32) -> Result<()> {
        self.open()?;
        if mode > 2 {
            return Err(Error::Invalid);
        }
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let r = state.regions.get_mut(&handle).ok_or(Error::Denied)?;
        let pages = Self::pages(r, offset, bytes)?;
        if r.pages[pages.clone()].contains(&0) {
            return Err(Error::Denied);
        }
        r.map
            .protect(offset, bytes, mode)
            .map_err(|_| Error::Failed)?;
        r.pages[pages].fill(mode as u8 + 1);
        Ok(())
    }
    pub fn decommit(&self, handle: u64, offset: u64, bytes: u64) -> Result<()> {
        self.open()?;
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let r = state.regions.get_mut(&handle).ok_or(Error::Denied)?;
        let pages = Self::pages(r, offset, bytes)?;
        r.map.decommit(offset, bytes).map_err(|_| Error::Failed)?;
        r.pages[pages].fill(0);
        Ok(())
    }
    /// Copy 1..256 page states under the memory/protection lock. The bytes
    /// describe only this runtime's backing: 0 uncommitted, 1 no-access,
    /// 2 read-only, 3 read/write. No data access or permission change occurs.
    pub fn page_states(&self, handle: u64, offset: u64, count: u32) -> Result<Vec<u8>> {
        self.open()?;
        if !(1..=256).contains(&count) {
            return Err(Error::Invalid);
        }
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let region = state.regions.get(&handle).ok_or(Error::Denied)?;
        let pages = Self::pages(region, offset, u64::from(count) * memory::PAGE)?;
        Ok(region.pages[pages].to_vec())
    }
    fn access_range(region: &Region, offset: u64, bytes: usize, write: bool) -> Result<usize> {
        if offset
            .checked_add(bytes as u64)
            .is_none_or(|end| end > region.bytes)
        {
            return Err(Error::Invalid);
        }
        if bytes != 0 {
            let first = (offset / memory::PAGE) as usize;
            let end = ((offset + bytes as u64 - 1) / memory::PAGE + 1) as usize;
            if region.pages[first..end]
                .iter()
                .any(|p| if write { *p != 3 } else { *p < 2 })
            {
                return Err(Error::Denied);
            }
        }
        usize::try_from(offset).map_err(|_| Error::Invalid)
    }
    fn access(region: &Region, offset: u64, bytes: usize, write: bool) -> Result<usize> {
        if bytes > 1024 * 1024 {
            return Err(Error::Invalid);
        }
        Self::access_range(region, offset, bytes, write)
    }
    /// Child-local, overlap-safe copy. No mapped address or slice escapes the
    /// mutex. Both complete ranges/rights are validated before the first write.
    pub fn copy(
        &self,
        source: u64,
        source_offset: u64,
        target: u64,
        target_offset: u64,
        bytes: u64,
    ) -> Result<()> {
        self.open()?;
        if bytes > 64 * 1024 * 1024 {
            return Err(Error::Invalid);
        }
        let bytes = usize::try_from(bytes).map_err(|_| Error::Invalid)?;
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let source = state.regions.get(&source).ok_or(Error::Denied)?;
        let target = state.regions.get(&target).ok_or(Error::Denied)?;
        let from = Self::access_range(source, source_offset, bytes, false)?;
        let to = Self::access_range(target, target_offset, bytes, true)?;
        // Mappings are live for the mutex duration. This is the sole place
        // that turns the checked opaque offsets into pointers for the copy.
        unsafe {
            source.map.copy_to(from, &target.map, to, bytes);
        }
        Ok(())
    }
    pub fn read(&self, handle: u64, offset: u64, output: &mut [u8]) -> Result<()> {
        self.open()?;
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let r = state.regions.get(&handle).ok_or(Error::Denied)?;
        let offset = Self::access(r, offset, output.len(), false)?;
        // No address or borrowed mapped slice escapes the runtime mutex.
        unsafe { r.map.read(offset, output) };
        Ok(())
    }
    /// Fill one opaque child-local window after validating all pages. The same
    /// mutex serializes access, protection, decommit and destruction throughout.
    pub fn fill(&self, handle: u64, offset: u64, bytes: u64, value: u8) -> Result<()> {
        self.open()?;
        if bytes > 64 * 1024 * 1024 {
            return Err(Error::Invalid);
        }
        let bytes = usize::try_from(bytes).map_err(|_| Error::Invalid)?;
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let region = state.regions.get(&handle).ok_or(Error::Denied)?;
        let offset = Self::access_range(region, offset, bytes, true)?;
        // Complete bounds/rights were checked under this still-held lock.
        unsafe {
            region.map.fill(offset, bytes, value);
        }
        Ok(())
    }
    pub fn write(&self, handle: u64, offset: u64, input: &[u8]) -> Result<()> {
        self.open()?;
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let r = state.regions.get(&handle).ok_or(Error::Denied)?;
        let offset = Self::access(r, offset, input.len(), true)?;
        unsafe { r.map.write(offset, input) };
        Ok(())
    }
    /// Atomically compare and replace a copied byte range in one opaque window.
    /// The same lock excludes writes, protection changes and decommit. All
    /// destination pages must be writable even when the comparison would fail.
    pub fn compare_exchange_memory(
        &self,
        handle: u64,
        offset: u64,
        expected: &[u8],
        replacement: &[u8],
    ) -> Result<bool> {
        self.open()?;
        if expected.is_empty() || expected.len() > 65536 || expected.len() != replacement.len() {
            return Err(Error::Invalid);
        }
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let region = state.regions.get(&handle).ok_or(Error::Denied)?;
        let offset = Self::access(region, offset, expected.len(), true)?;
        let mut current = vec![0u8; expected.len()];
        // The mapping and rights remain live for this complete comparison/write.
        // No pointer or borrowed mapped byte escapes the runtime lock.
        unsafe {
            region.map.read(offset, &mut current);
        }
        if current != expected {
            return Ok(false);
        }
        unsafe {
            region.map.write(offset, replacement);
        }
        Ok(true)
    }
    /// Bounded gather/scatter over opaque windows. A segment is
    /// [window ID, byte offset, byte count], never a mapped pointer.
    fn segment_bytes(segments: &[[u64; 3]]) -> Result<usize> {
        if segments.is_empty() || segments.len() > 32 {
            return Err(Error::Invalid);
        }
        let mut total = 0u64;
        for segment in segments {
            if segment[2] == 0 {
                return Err(Error::Invalid);
            }
            total = total.checked_add(segment[2]).ok_or(Error::Invalid)?;
            if total > 1024 * 1024 {
                return Err(Error::Invalid);
            }
        }
        usize::try_from(total).map_err(|_| Error::Invalid)
    }
    /// Concatenate reads in descriptor order. Validate EVERY descriptor before
    /// touching output, under the same lock used for protection and teardown.
    pub fn read_segments(&self, segments: &[[u64; 3]], output: &mut [u8]) -> Result<()> {
        self.open()?;
        if Self::segment_bytes(segments)? != output.len() {
            return Err(Error::Invalid);
        }
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let mut offsets = [0usize; 32];
        for (index, segment) in segments.iter().enumerate() {
            let region = state.regions.get(&segment[0]).ok_or(Error::Denied)?;
            offsets[index] = Self::access(region, segment[1], segment[2] as usize, false)?;
        }
        let mut cursor = 0usize;
        for (index, segment) in segments.iter().enumerate() {
            let region = &state.regions[&segment[0]];
            let end = cursor + segment[2] as usize;
            // Every range was checked; mappings/rights remain locked. Output
            // belongs to the caller, not to any mapped game window.
            unsafe { region.map.read(offsets[index], &mut output[cursor..end]) };
            cursor = end;
        }
        Ok(())
    }
    /// Split one copied payload across destinations in descriptor order. Later
    /// segments win if destinations alias. Validation failure changes nothing.
    pub fn write_segments(&self, segments: &[[u64; 3]], input: &[u8]) -> Result<()> {
        self.open()?;
        if Self::segment_bytes(segments)? != input.len() {
            return Err(Error::Invalid);
        }
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let mut offsets = [0usize; 32];
        for (index, segment) in segments.iter().enumerate() {
            let region = state.regions.get(&segment[0]).ok_or(Error::Denied)?;
            offsets[index] = Self::access(region, segment[1], segment[2] as usize, true)?;
        }
        let mut cursor = 0usize;
        for (index, segment) in segments.iter().enumerate() {
            let region = &state.regions[&segment[0]];
            let end = cursor + segment[2] as usize;
            // The copied input cannot alias a mapping; no pointer escapes.
            unsafe { region.map.write(offsets[index], &input[cursor..end]) };
            cursor = end;
        }
        Ok(())
    }
    pub fn spawn(&self, entry: u32, argument: u64, suspended: bool) -> Result<u64> {
        self.open()?;
        let task = *self.0.entries.get(entry as usize).ok_or(Error::Denied)?;
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        if state.jobs.len() >= self.0.task_limit {
            return Err(Error::Busy);
        }
        let handle = id()?;
        let cancel = Arc::new(AtomicBool::new(false));
        let gate = Arc::new((Mutex::new(suspended), Condvar::new()));
        let worker_gate = gate.clone();
        let context = TaskContext {
            runtime: self.clone(),
            cancel: cancel.clone(),
        };
        let exit_runtime = self.clone();
        let thread = std::thread::Builder::new()
            .name("gamepkg-task".into())
            .stack_size(self.0.stack_bytes)
            .spawn(move || {
                let _exit = TaskExit(exit_runtime);
                let Ok(mut suspended) = worker_gate.0.lock() else {
                    return 0;
                };
                while *suspended && !context.cancelled() {
                    let Ok((next, _)) = worker_gate
                        .1
                        .wait_timeout(suspended, Duration::from_secs(1))
                    else {
                        return 0;
                    };
                    suspended = next;
                }
                drop(suspended);
                if context.cancelled() {
                    0
                } else {
                    task(context, argument)
                }
            })
            .map_err(|_| Error::Failed)?;
        state.jobs.insert(
            handle,
            Job {
                cancel,
                gate,
                handle: Arc::new(Mutex::new(Some(thread))),
            },
        );
        Ok(handle)
    }
    pub fn cancel(&self, handle: u64) -> Result<()> {
        self.open()?;
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        let job = state.jobs.get(&handle).ok_or(Error::Denied)?;
        let _gate = job.gate.0.lock().map_err(|_| Error::Failed)?;
        job.cancel.store(true, Ordering::Release);
        job.gate.1.notify_all();
        let clock_result = self.0.clock.interrupt();
        for object in state.sync.values() {
            // Pair cancellation with the waiter's value lock. A notification
            // between its flag check and atomic condvar release must not vanish.
            let _value = object.value.lock().map_err(|_| Error::Failed)?;
            object.changed.notify_all();
        }
        for object in state.mailboxes.values() {
            let _queue = object.messages.lock().map_err(|_| Error::Failed)?;
            object.changed.notify_all();
        }
        self.notify_waiters();
        clock_result?;
        Ok(())
    }
    /// Bounded copied messages between reviewed tasks in the same child.
    /// IDs never repeat; there is no cross-session or OS channel endpoint.
    #[allow(deprecated)] // Rust 1.89 has fetch_update, before the try_update rename.
    pub fn mailbox(&self, maximum: usize, depth: usize) -> Result<u64> {
        if !(1..=65536).contains(&maximum) || !(1..=32).contains(&depth) {
            return Err(Error::Invalid);
        }
        self.open()?;
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let handle = id()?;
        let charge = maximum * depth;
        let budget = &self.0.mailbox_budget;
        budget
            .slots
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |slots| {
                (slots < 16).then_some(slots + 1)
            })
            .map_err(|_| Error::Busy)?;
        if budget
            .bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |bytes| {
                bytes.checked_add(charge).filter(|n| *n <= MAILBOX_BYTES)
            })
            .is_err()
        {
            budget.slots.fetch_sub(1, Ordering::AcqRel);
            return Err(Error::Busy);
        }
        state.mailboxes.insert(
            handle,
            Arc::new(Mailbox {
                messages: Mutex::new(VecDeque::with_capacity(depth)),
                changed: Condvar::new(),
                closed: AtomicBool::new(false),
                maximum,
                depth,
                budget: budget.clone(),
            }),
        );
        Ok(handle)
    }
    fn mailbox_get(&self, handle: u64) -> Result<Arc<Mailbox>> {
        self.open()?;
        self.0
            .state
            .lock()
            .map_err(|_| Error::Failed)?
            .mailboxes
            .get(&handle)
            .cloned()
            .ok_or(Error::Denied)
    }
    /// Nonblocking: a full queue returns Busy and leaves it unchanged.
    pub fn mailbox_send(&self, handle: u64, bytes: &[u8]) -> Result<()> {
        let object = self.mailbox_get(handle)?;
        if bytes.len() > object.maximum {
            return Err(Error::Invalid);
        }
        let mut queue = object.messages.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        if object.closed.load(Ordering::Acquire) {
            return Err(Error::Denied);
        }
        if queue.len() >= object.depth {
            return Err(Error::Busy);
        }
        queue.push_back(bytes.to_vec());
        object.changed.notify_one();
        Ok(())
    }
    /// Only confined child tasks may wait. Capacity failures never consume the
    /// front message. Cancellation/release/runtime close wake a blocked receiver.
    pub fn mailbox_receive(
        &self,
        handle: u64,
        capacity: usize,
        milliseconds: u32,
    ) -> Result<Vec<u8>> {
        if capacity > 65536 || milliseconds > 1000 {
            return Err(Error::Invalid);
        }
        let object = self.mailbox_get(handle)?;
        let cancelled = self.task_cancellation()?;
        let end = Instant::now() + Duration::from_millis(milliseconds.into());
        let mut queue = object.messages.lock().map_err(|_| Error::Failed)?;
        loop {
            self.open()?;
            if object.closed.load(Ordering::Acquire)
                || cancelled
                    .as_ref()
                    .is_some_and(|flag| flag.load(Ordering::Acquire))
            {
                return Err(Error::Denied);
            }
            if let Some(message) = queue.front() {
                if message.len() > capacity {
                    return Err(Error::Invalid);
                }
                return Ok(queue.pop_front().unwrap());
            }
            let now = Instant::now();
            if now >= end {
                return Err(Error::Busy);
            }
            let (next, _) = object
                .changed
                .wait_timeout(queue, end - now)
                .map_err(|_| Error::Failed)?;
            queue = next;
        }
    }
    pub fn mailbox_release(&self, handle: u64) -> Result<()> {
        self.open()?;
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let object = state.mailboxes.remove(&handle).ok_or(Error::Denied)?;
        let mut queue = object.messages.lock().map_err(|_| Error::Failed)?;
        object.closed.store(true, Ordering::Release);
        queue.clear();
        object.changed.notify_all();
        Ok(())
    }
    /// Numeric reviewed task identity and its cooperative cancellation flag.
    /// Zero is the bootstrap/non-task thread; no OS thread handle escapes.
    pub fn current_task(&self) -> Result<(u64, bool)> {
        self.open()?;
        let thread = std::thread::current().id();
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        for (&id, job) in &state.jobs {
            if job
                .handle
                .lock()
                .map_err(|_| Error::Failed)?
                .as_ref()
                .is_some_and(|handle| handle.thread().id() == thread)
            {
                return Ok((id, job.cancel.load(Ordering::Acquire)));
            }
        }
        Ok((0, false))
    }
    /// Resume the initial bootstrap gate only. Does not suspend or manipulate
    /// an already-running native thread. Returns whether it was suspended.
    pub fn resume(&self, handle: u64) -> Result<bool> {
        self.open()?;
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        let job = state.jobs.get(&handle).ok_or(Error::Denied)?;
        let mut gate = job.gate.0.lock().map_err(|_| Error::Failed)?;
        let previous = *gate;
        *gate = false;
        job.gate.1.notify_all();
        Ok(previous)
    }
    /// Non-consuming completion snapshot for this runtime's reviewed task ID.
    /// Never joins, resumes, cancels or exposes a native thread handle/result.
    pub fn task_finished(&self, handle: u64) -> Result<bool> {
        self.open()?;
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        let job = state.jobs.get(&handle).ok_or(Error::Denied)?;
        let result = job
            .handle
            .lock()
            .map_err(|_| Error::Failed)?
            .as_ref()
            .ok_or(Error::Denied)?
            .is_finished();
        Ok(result)
    }
    /// Poll/consume only. Never blocks a window thread waiting for game code.
    pub fn join(&self, handle: u64) -> Result<u64> {
        self.open()?;
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        let job = state.jobs.get(&handle).ok_or(Error::Denied)?;
        let observed = job.handle.clone();
        let mut thread = observed.lock().map_err(|_| Error::Failed)?;
        if !thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Err(Error::Busy);
        }
        let finished = thread.take().ok_or(Error::Denied)?;
        state.jobs.remove(&handle);
        drop(thread);
        drop(state);
        self.notify_waiters();
        finished.join().map_err(|_| Error::Failed)
    }
    fn sync_new(&self, value: Kind) -> Result<u64> {
        self.open()?;
        let cancellation = if matches!(value, Kind::Semaphore { .. }) {
            self.task_cancellation()?
        } else {
            None
        };
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        if cancellation
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
        {
            return Err(Error::Denied);
        }
        if state.sync.len() >= 128 {
            return Err(Error::Busy);
        }
        let handle = id()?;
        state.sync.insert(
            handle,
            Arc::new(Sync {
                value: Mutex::new(value),
                changed: Condvar::new(),
                closed: AtomicBool::new(false),
            }),
        );
        Ok(handle)
    }
    pub fn event(&self, manual: bool, signaled: bool) -> Result<u64> {
        self.sync_new(Kind::Event { manual, signaled })
    }
    pub fn mutex(&self) -> Result<u64> {
        self.sync_new(Kind::Mutex {
            owner: None,
            depth: 0,
            abandoned: false,
        })
    }
    /// Counted synchronization, sharing the existing 128-object budget. Counts
    /// fit a signed 32-bit native semaphore on every supported target.
    pub fn semaphore(&self, count: u32, maximum: u32) -> Result<u64> {
        if maximum == 0 || maximum > i32::MAX as u32 || count > maximum {
            return Err(Error::Invalid);
        }
        self.sync_new(Kind::Semaphore { count, maximum })
    }
    /// Add permits atomically and return the previous count. Failed releases
    /// neither change the count nor wake waiters. A snapshot grants no lease.
    pub fn semaphore_release(&self, handle: u64, permits: u32) -> Result<u32> {
        let cancelled = self.task_cancellation()?;
        let object = self.sync_get(handle)?;
        let mut value = object.value.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        if object.closed.load(Ordering::Acquire)
            || cancelled
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Acquire))
        {
            return Err(Error::Denied);
        }
        let Kind::Semaphore { count, maximum } = &mut *value else {
            return Err(Error::Invalid);
        };
        let next = count
            .checked_add(permits)
            .filter(|next| permits != 0 && next <= maximum)
            .ok_or(Error::Invalid)?;
        let previous = *count;
        *count = next;
        object.changed.notify_all();
        self.notify_waiters();
        Ok(previous)
    }
    pub fn semaphore_state(&self, handle: u64) -> Result<SemaphoreState> {
        let cancelled = self.task_cancellation()?;
        let object = self.sync_get(handle)?;
        let value = object.value.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        if object.closed.load(Ordering::Acquire)
            || cancelled
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Acquire))
        {
            return Err(Error::Denied);
        }
        match *value {
            Kind::Semaphore { count, maximum } => Ok(SemaphoreState { count, maximum }),
            _ => Err(Error::Invalid),
        }
    }
    /// Typed, cancellable acquisition of exactly one permit. Never acquires a
    /// mutex or consumes an event, and never blocks the window owner thread.
    pub fn semaphore_wait(&self, handle: u64, milliseconds: u32, worker: bool) -> Result<()> {
        self.wait_sync(handle, milliseconds, worker, WaitKind::Semaphore)
    }
    fn sync_get(&self, handle: u64) -> Result<Arc<Sync>> {
        self.open()?;
        self.0
            .state
            .lock()
            .map_err(|_| Error::Failed)?
            .sync
            .get(&handle)
            .cloned()
            .ok_or(Error::Denied)
    }
    pub fn signal(&self, handle: u64, signaled: bool) -> Result<()> {
        let object = self.sync_get(handle)?;
        let mut value = object.value.lock().map_err(|_| Error::Failed)?;
        if object.closed.load(Ordering::Acquire) {
            return Err(Error::Denied);
        }
        let Kind::Event {
            signaled: current, ..
        } = &mut *value
        else {
            return Err(Error::Invalid);
        };
        *current = signaled;
        object.changed.notify_all();
        self.notify_waiters();
        Ok(())
    }
    /// Explicitly worker-only, bounded one-second wait. A recursive mutex is
    /// owned by the actual runtime thread; game-supplied owner IDs are ignored.
    pub fn wait(&self, handle: u64, milliseconds: u32, worker: bool) -> Result<()> {
        self.wait_sync(handle, milliseconds, worker, WaitKind::Any)
    }
    /// Typed acquisition: an event cannot stand in for a mutex. The kind check
    /// and recursive acquisition use the same retained object and value lock.
    pub fn mutex_lock(&self, handle: u64, milliseconds: u32, worker: bool) -> Result<()> {
        self.wait_sync(handle, milliseconds, worker, WaitKind::Mutex)
    }
    /// Typed cancellable event wait. Never acquires a mutex with a forged ID.
    pub fn event_wait(&self, handle: u64, milliseconds: u32, worker: bool) -> Result<()> {
        self.wait_sync(handle, milliseconds, worker, WaitKind::Event)
    }
    /// Non-consuming event-kind/state observation; not a lifetime extension.
    pub fn event_state(&self, handle: u64) -> Result<EventState> {
        let cancelled = self.task_cancellation()?;
        let object = self.sync_get(handle)?;
        let value = object.value.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        if object.closed.load(Ordering::Acquire)
            || cancelled
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Acquire))
        {
            return Err(Error::Denied);
        }
        match *value {
            Kind::Event { manual, signaled } => Ok(EventState { manual, signaled }),
            _ => Err(Error::Invalid),
        }
    }
    /// Observe only an actual mutex without acquiring, unlocking, or clearing
    /// abandonment. Ownership is relative to this actual runtime thread.
    pub fn mutex_state(&self, handle: u64) -> Result<MutexState> {
        let cancelled = self.task_cancellation()?;
        let object = self.sync_get(handle)?;
        let value = object.value.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        if object.closed.load(Ordering::Acquire)
            || cancelled
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Acquire))
        {
            return Err(Error::Denied);
        }
        let Kind::Mutex {
            owner,
            depth,
            abandoned,
        } = *value
        else {
            return Err(Error::Invalid);
        };
        // Detect internal inconsistency; never manufacture a valid observation.
        if owner.is_some() != (depth != 0) || (abandoned && owner.is_some()) {
            return Err(Error::Failed);
        }
        Ok(MutexState {
            locked: owner.is_some(),
            owned_by_caller: owner == Some(std::thread::current().id()),
            abandoned,
            depth,
        })
    }
    fn wait_sync(
        &self,
        handle: u64,
        milliseconds: u32,
        worker: bool,
        expected: WaitKind,
    ) -> Result<()> {
        if !worker {
            return Err(Error::Busy);
        }
        if milliseconds > 1000 {
            return Err(Error::Invalid);
        }
        let cancelled = if expected != WaitKind::Any {
            self.task_cancellation()?
        } else {
            None
        };
        let object = self.sync_get(handle)?;
        let end = Instant::now() + Duration::from_millis(milliseconds.into());
        let mut value = object.value.lock().map_err(|_| Error::Failed)?;
        if expected != WaitKind::Any {
            self.open()?;
            if object.closed.load(Ordering::Acquire)
                || cancelled
                    .as_ref()
                    .is_some_and(|flag| flag.load(Ordering::Acquire))
            {
                return Err(Error::Denied);
            }
            if !matches!(
                (expected, &*value),
                (WaitKind::Mutex, Kind::Mutex { .. })
                    | (WaitKind::Event, Kind::Event { .. })
                    | (WaitKind::Semaphore, Kind::Semaphore { .. })
            ) {
                return Err(Error::Invalid);
            }
        }
        loop {
            self.open()?;
            if object.closed.load(Ordering::Acquire)
                || cancelled
                    .as_ref()
                    .is_some_and(|flag| flag.load(Ordering::Acquire))
            {
                return Err(Error::Denied);
            }
            match &mut *value {
                Kind::Event { manual, signaled } => {
                    if *signaled {
                        if !*manual {
                            *signaled = false;
                        }
                        return Ok(());
                    }
                }
                Kind::Mutex {
                    owner,
                    depth,
                    abandoned,
                } => {
                    // Legacy unit-returning acquisition cannot report abandonment.
                    // Leave it untouched for the explicit wait-set result path.
                    if *abandoned {
                        return Err(Error::Denied);
                    }
                    if owner.is_none() || *owner == Some(std::thread::current().id()) {
                        *depth = depth.checked_add(1).ok_or(Error::Busy)?;
                        *owner = Some(std::thread::current().id());
                        return Ok(());
                    }
                }
                Kind::Semaphore { count, .. } => {
                    // Do not widen the legacy untyped, noncancellable op 23.
                    if expected != WaitKind::Semaphore {
                        return Err(Error::Invalid);
                    }
                    if *count != 0 {
                        *count -= 1;
                        return Ok(());
                    }
                }
            }
            let now = Instant::now();
            if now >= end {
                return Err(Error::Busy);
            }
            let (next, _) = object
                .changed
                .wait_timeout(value, end - now)
                .map_err(|_| Error::Failed)?;
            value = next;
        }
    }
    pub fn unlock(&self, handle: u64) -> Result<()> {
        let object = self.sync_get(handle)?;
        let mut value = object.value.lock().map_err(|_| Error::Failed)?;
        let Kind::Mutex { owner, depth, .. } = &mut *value else {
            return Err(Error::Invalid);
        };
        if *owner != Some(std::thread::current().id()) || *depth == 0 {
            return Err(Error::Denied);
        }
        *depth -= 1;
        if *depth == 0 {
            *owner = None;
            object.changed.notify_all();
            self.notify_waiters();
        }
        Ok(())
    }
    /// Cleanup may run after task cancellation, but only for the actual kind
    /// in this live runtime. It never unlocks a held mutex or acts on task IDs.
    pub fn release_sync_typed(&self, handle: u64, kind: u32) -> Result<()> {
        if !(1..=3).contains(&kind) {
            return Err(Error::Invalid);
        }
        self.open()?;
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        let object = state.sync.get(&handle).cloned().ok_or(Error::Denied)?;
        let value = object.value.lock().map_err(|_| Error::Failed)?;
        self.open()?;
        if object.closed.load(Ordering::Acquire) {
            return Err(Error::Denied);
        }
        if !matches!(
            (kind, &*value),
            (1, Kind::Event { .. }) | (2, Kind::Mutex { .. }) | (3, Kind::Semaphore { .. })
        ) {
            return Err(Error::Invalid);
        }
        if let Kind::Mutex {
            owner,
            depth,
            abandoned,
        } = *value
        {
            if owner.is_some() != (depth != 0) || (abandoned && owner.is_some()) {
                return Err(Error::Failed);
            }
        }
        if matches!(*value,Kind::Mutex{depth,..} if depth!=0) {
            return Err(Error::Busy);
        }
        state.sync.remove(&handle);
        object.closed.store(true, Ordering::Release);
        object.changed.notify_all();
        self.notify_waiters();
        Ok(())
    }
    pub fn release_sync(&self, handle: u64) -> Result<()> {
        self.open()?;
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        let object = state.sync.get(&handle).cloned().ok_or(Error::Denied)?;
        let value = object.value.lock().map_err(|_| Error::Failed)?;
        if matches!(*value,Kind::Mutex{depth,..} if depth!=0) {
            return Err(Error::Busy);
        }
        state.sync.remove(&handle);
        object.closed.store(true, Ordering::Release);
        object.changed.notify_all();
        self.notify_waiters();
        Ok(())
    }
    /// Revoke access and wake waiters. Returns live task count; the outer
    /// confined process owner MUST terminate the process if tasks fail to exit.
    /// No unsafe native thread termination or arbitrary callback is exposed.
    pub fn close(&self) -> Result<usize> {
        self.0.closed.store(true, Ordering::Release);
        self.notify_waiters();
        let clock_result = self.0.clock.close();
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        let objects = state.sync.values().cloned().collect::<Vec<_>>();
        let mailboxes = std::mem::take(&mut state.mailboxes);
        for job in state.jobs.values() {
            let _gate = job.gate.0.lock().map_err(|_| Error::Failed)?;
            job.cancel.store(true, Ordering::Release);
            job.gate.1.notify_all();
        }
        state.regions.clear();
        state.records.clear();
        state.record_bytes = 0;
        let mut live = 0usize;
        for job in state.jobs.values() {
            if job
                .handle
                .lock()
                .map_err(|_| Error::Failed)?
                .as_ref()
                .is_some_and(|handle| !handle.is_finished())
            {
                live += 1;
            }
        }
        drop(state);
        for object in objects {
            let _value = object.value.lock().map_err(|_| Error::Failed)?;
            object.closed.store(true, Ordering::Release);
            object.changed.notify_all();
        }
        for object in mailboxes.values() {
            let mut queue = object.messages.lock().map_err(|_| Error::Failed)?;
            object.closed.store(true, Ordering::Release);
            queue.clear();
            object.changed.notify_all();
        }
        clock_result?;
        Ok(live)
    }
}

#[cfg(test)]
mod cancellation_boundary_tests {
    use super::*;
    fn cancellable(context: TaskContext, ready: u64) -> u64 {
        context.runtime.mailbox_send(ready, &[1]).unwrap();
        while !context.cancelled() {
            std::thread::yield_now();
        }
        41
    }
    #[test]
    fn cancellation_notification_synchronizes_with_the_wait_value_lock() {
        for semaphore in [false, true] {
            let owner = Runtime::new(&[], vec![cancellable], 4 * 1024 * 1024, 1, 65536).unwrap();
            let event = if semaphore {
                owner.semaphore(0, 1).unwrap()
            } else {
                owner.event(false, false).unwrap()
            };
            let ready = owner.mailbox(1, 1).unwrap();
            let task = owner.spawn(0, ready, false).unwrap();
            assert_eq!(owner.mailbox_receive(ready, 1, 1000), Ok(vec![1]));
            let flag = owner.0.state.lock().unwrap().jobs[&task].cancel.clone();
            let object = owner.sync_get(event).unwrap();
            let held = object.value.lock().unwrap();
            let copy = owner.clone();
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            let thread = std::thread::spawn(move || tx.send(copy.cancel(task)).unwrap());
            let end = Instant::now() + Duration::from_secs(3);
            while !flag.load(Ordering::Acquire) {
                assert!(Instant::now() < end);
                std::thread::yield_now();
            }
            assert_eq!(
                rx.recv_timeout(Duration::from_millis(100)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            );
            drop(held);
            assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), Ok(()));
            thread.join().unwrap();
            while !owner.task_finished(task).unwrap() {
                assert!(Instant::now() < end);
                std::thread::yield_now();
            }
            assert_eq!(owner.join(task), Ok(41));
            owner.release_sync(event).unwrap();
            owner.mailbox_release(ready).unwrap();
        }
    }
}

#[cfg(test)]
mod mailbox_lifetime_tests {
    use super::*;

    #[test]
    fn released_inflight_mailboxes_keep_capacity_and_slot_charges() {
        let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
        let mailbox = owner.mailbox(65536, 16).unwrap();
        // The same Arc is held by a receive while it waits without the state
        // lock. A released handle cannot recycle that in-flight reservation.
        let inflight = owner.mailbox_get(mailbox).unwrap();
        owner.mailbox_release(mailbox).unwrap();
        assert_eq!(owner.mailbox(1, 1), Err(Error::Busy));
        drop(inflight);
        let mut held = Vec::new();
        for _ in 0..16 {
            let mailbox = owner.mailbox(1, 1).unwrap();
            held.push(owner.mailbox_get(mailbox).unwrap());
            owner.mailbox_release(mailbox).unwrap();
        }
        assert_eq!(owner.mailbox(1, 1), Err(Error::Busy));
        drop(held);
        assert!(owner.mailbox(65536, 16).is_ok());
    }
}

#[cfg(test)]
mod record_version_tests {
    use super::*;
    #[test]
    fn record_version_exhaustion_is_atomic_and_never_wraps() {
        let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
        let record = owner.record_create(&[1, 2, 3]).unwrap();
        owner
            .0
            .state
            .lock()
            .unwrap()
            .records
            .get_mut(&record)
            .unwrap()
            .version = u64::MAX;
        assert_eq!(
            owner.record_compare_exchange(record, u64::MAX, &[9, 9, 9]),
            Err(Error::Failed)
        );
        assert_eq!(
            owner.record_snapshot(record, 3).unwrap(),
            (u64::MAX, vec![1, 2, 3])
        );
    }
}

#[cfg(test)]
mod waitset_boundaries {
    use super::*;
    #[test]
    fn recursive_overflow_rejects_the_entire_acquisition_before_consuming() {
        let runtime = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
        let event = runtime.event(false, true).unwrap();
        let semaphore = runtime.semaphore(1, 1).unwrap();
        let mutex = runtime.mutex().unwrap();
        runtime.mutex_lock(mutex, 0, true).unwrap();
        let object = runtime.sync_get(mutex).unwrap();
        if let Kind::Mutex { depth, .. } = &mut *object.value.lock().unwrap() {
            *depth = u32::MAX;
        }
        assert_eq!(
            runtime.wait_many(
                &[
                    WaitObject { id: event, kind: 1 },
                    WaitObject {
                        id: semaphore,
                        kind: 3
                    },
                    WaitObject { id: mutex, kind: 2 }
                ],
                true,
                0,
                true
            ),
            Err(Error::Busy)
        );
        assert!(runtime.event_state(event).unwrap().signaled);
        assert_eq!(runtime.semaphore_state(semaphore).unwrap().count, 1);
        if let Kind::Mutex { depth, .. } = &mut *object.value.lock().unwrap() {
            *depth = 1;
        }
        runtime.unlock(mutex).unwrap();
        for id in [event, semaphore, mutex] {
            runtime.release_sync(id).unwrap();
        }
    }
}

#[cfg(test)]
mod mutex_observation_boundaries {
    use super::*;
    #[test]
    fn inconsistent_internal_mutex_states_are_reported_without_repair_or_acquisition() {
        let runtime = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
        let id = runtime.mutex().unwrap();
        let object = runtime.sync_get(id).unwrap();
        // Synthetic internal corruption: no game API exposes these fields.
        for held in [false, true] {
            for depth in [0, 1, u32::MAX] {
                for abandoned in [false, true] {
                    let owner = if held {
                        Some(std::thread::current().id())
                    } else {
                        None
                    };
                    *object.value.lock().unwrap() = Kind::Mutex {
                        owner,
                        depth,
                        abandoned,
                    };
                    let valid = held == (depth != 0) && !(abandoned && held);
                    assert_eq!(
                        runtime.mutex_state(id),
                        if valid {
                            Ok(MutexState {
                                locked: held,
                                owned_by_caller: held,
                                abandoned,
                                depth,
                            })
                        } else {
                            Err(Error::Failed)
                        }
                    );
                    if !valid {
                        assert_eq!(runtime.release_sync_typed(id, 2), Err(Error::Failed));
                    }
                    let value = object.value.lock().unwrap();
                    let Kind::Mutex {
                        owner: after,
                        depth: after_depth,
                        abandoned: after_abandoned,
                    } = *value
                    else {
                        panic!("kind changed")
                    };
                    assert_eq!(
                        (after, after_depth, after_abandoned),
                        (owner, depth, abandoned)
                    );
                }
            }
        }
        *object.value.lock().unwrap() = Kind::Mutex {
            owner: None,
            depth: 0,
            abandoned: false,
        };
        runtime.release_sync(id).unwrap();
    }
    #[test]
    fn revoked_retained_mutex_is_rechecked_after_its_value_lock_becomes_available() {
        let runtime = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
        let id = runtime.mutex().unwrap();
        let object = runtime.sync_get(id).unwrap();
        let guard = object.value.lock().unwrap();
        let copy = runtime.clone();
        let waiter = std::thread::spawn(move || copy.mutex_state(id));
        let end = Instant::now() + Duration::from_secs(3);
        while Arc::strong_count(&object) < 3 {
            assert!(Instant::now() < end);
            std::thread::yield_now();
        }
        // Deterministically reproduce release_sync's closed/remove prefix after
        // lookup, with the same actual storage retained by the waiting query.
        object.closed.store(true, Ordering::Release);
        runtime.0.state.lock().unwrap().sync.remove(&id).unwrap();
        drop(guard);
        assert_eq!(waiter.join().unwrap(), Err(Error::Denied));
    }

    #[test]
    fn typed_retirement_checks_kind_and_retains_revocation_notifications() {
        let runtime = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
        let objects = [
            (runtime.event(false, true).unwrap(), 1u32),
            (runtime.mutex().unwrap(), 2),
            (runtime.semaphore(1, 2).unwrap(), 3),
        ];
        for (id, kind) in objects {
            let object = runtime.sync_get(id).unwrap();
            let epoch = *runtime.0.wait_changed.0.lock().unwrap();
            for wrong in [0, 1, 2, 3, 4, u32::MAX] {
                if wrong == kind {
                    continue;
                }
                assert_eq!(runtime.release_sync_typed(id, wrong), Err(Error::Invalid));
                assert!(!object.closed.load(Ordering::Acquire));
                assert_eq!(*runtime.0.wait_changed.0.lock().unwrap(), epoch);
            }
            runtime.release_sync_typed(id, kind).unwrap();
            assert!(object.closed.load(Ordering::Acquire));
            assert_ne!(*runtime.0.wait_changed.0.lock().unwrap(), epoch);
            assert_eq!(runtime.release_sync_typed(id, kind), Err(Error::Denied));
        }
    }

    fn blocked_query(context: TaskContext, id: u64) -> u64 {
        if context.runtime().mutex_state(id) == Err(Error::Denied) {
            42
        } else {
            41
        }
    }
    #[test]
    fn cancellation_is_rechecked_after_the_observation_waits_for_value_storage() {
        let runtime = Runtime::new(&[], vec![blocked_query], 4 * 1024 * 1024, 1, 65536).unwrap();
        let id = runtime.mutex().unwrap();
        let object = runtime.sync_get(id).unwrap();
        let guard = object.value.lock().unwrap();
        let job = runtime.spawn(0, id, false).unwrap();
        let end = Instant::now() + Duration::from_secs(3);
        while Arc::strong_count(&object) < 3 {
            assert!(Instant::now() < end);
            std::thread::yield_now();
        }
        // Synthetic, deterministic public-cancel prefix: actual task flag is
        // set while the query retains the actual object and waits for its lock.
        runtime
            .0
            .state
            .lock()
            .unwrap()
            .jobs
            .get(&job)
            .unwrap()
            .cancel
            .store(true, Ordering::Release);
        drop(guard);
        runtime
            .wait_many(&[WaitObject { id: job, kind: 4 }], false, 1000, true)
            .unwrap();
        assert_eq!(runtime.join(job), Ok(42));
        runtime.release_sync_typed(id, 2).unwrap();
    }
}
