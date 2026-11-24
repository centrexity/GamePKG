//! One confined-child timing lane, independent of the remote connection.
use super::{performance, Error, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex, MutexGuard,
};
#[cfg(windows)]
#[path = "local_timing_windows.rs"]
mod windows;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub counter: u64,
    pub frequency: u64,
    pub uptime_ms: u64,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitResult {
    pub snapshot: Snapshot,
    pub reason: u32,
}
#[derive(Default)]
struct State {
    generation: u64,
    waiting: bool,
    closed: bool,
    last: Option<(u64, u64)>,
    #[cfg(windows)]
    native: Option<Arc<windows::Native>>,
}
pub(super) struct Clock {
    state: Mutex<State>,
    changed: Condvar,
}
impl Clock {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }
    }
    fn read(state: &State) -> Result<performance::Raw> {
        let sample = performance::sample()?;
        validate_sample(state.last, sample.counter, sample.frequency)?;
        Ok(sample)
    }
    fn snapshot_value(state: &State, raw: performance::Raw) -> Snapshot {
        Snapshot {
            counter: raw.counter,
            frequency: raw.frequency,
            uptime_ms: raw.uptime_ms,
            generation: state.generation,
        }
    }
    pub(super) fn snapshot(&self) -> Result<Snapshot> {
        let mut state = self.state.lock().map_err(|_| Error::Failed)?;
        if state.closed {
            return Err(Error::Denied);
        }
        let sample = Self::read(&state)?;
        state.last = Some((sample.counter, sample.frequency));
        Ok(Self::snapshot_value(&state, sample))
    }
    fn notify(&self, state: &State) -> Result<()> {
        self.changed.notify_all();
        #[cfg(windows)]
        if let Some(native) = &state.native {
            native.signal()?;
        }
        #[cfg(not(windows))]
        let _ = state;
        Ok(())
    }
    pub(super) fn wake(&self) -> Result<u64> {
        let mut state = self.state.lock().map_err(|_| Error::Failed)?;
        if state.closed {
            return Err(Error::Denied);
        }
        state.generation = state.generation.checked_add(1).ok_or(Error::Failed)?;
        self.notify(&state)?;
        Ok(state.generation)
    }
    pub(super) fn interrupt(&self) -> Result<()> {
        let state = self.state.lock().map_err(|_| Error::Failed)?;
        self.notify(&state)
    }
    pub(super) fn close(&self) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Failed)?;
        state.closed = true;
        let result = self.notify(&state);
        // A pending native waiter retains its Arc until its wait returns.
        #[cfg(windows)]
        {
            state.native.take();
        }
        result
    }
    pub(super) fn wait(
        &self,
        deadline: u64,
        frequency: u64,
        generation: u64,
        cancelled: Option<Arc<AtomicBool>>,
        worker: bool,
    ) -> Result<WaitResult> {
        if !worker {
            return Err(Error::Busy);
        }
        if deadline > i64::MAX as u64 || frequency == 0 || frequency > i64::MAX as u64 {
            return Err(Error::Invalid);
        }
        let mut state = self.state.lock().map_err(|_| Error::Failed)?;
        if state.closed || is_cancelled(&cancelled) {
            return Err(Error::Denied);
        }
        if generation > state.generation {
            return Err(Error::Invalid);
        }
        if state.waiting {
            return Err(Error::Busy);
        }
        let sample = Self::read(&state)?;
        if sample.frequency != frequency || deadline.saturating_sub(sample.counter) > frequency {
            return Err(Error::Invalid);
        }
        state.last = Some((sample.counter, sample.frequency));
        if generation != state.generation || sample.counter >= deadline {
            return Ok(WaitResult {
                snapshot: Self::snapshot_value(&state, sample),
                reason: u32::from(generation != state.generation),
            });
        }
        state.waiting = true;
        let result = self.wait_inner(state, deadline, frequency, generation, &cancelled);
        // wait_inner drops its guard on every normal error/result. Never clear
        // the slot before the native/condition-variable wait has ended.
        self.state.lock().map_err(|_| Error::Failed)?.waiting = false;
        result
    }
    fn wait_inner<'a>(
        &'a self,
        mut state: MutexGuard<'a, State>,
        deadline: u64,
        frequency: u64,
        generation: u64,
        cancelled: &Option<Arc<AtomicBool>>,
    ) -> Result<WaitResult> {
        loop {
            if state.closed || is_cancelled(cancelled) {
                return Err(Error::Denied);
            }
            let sample = Self::read(&state)?;
            if sample.frequency != frequency {
                return Err(Error::Failed);
            }
            state.last = Some((sample.counter, sample.frequency));
            if generation != state.generation || sample.counter >= deadline {
                return Ok(WaitResult {
                    snapshot: Self::snapshot_value(&state, sample),
                    reason: u32::from(generation != state.generation),
                });
            }
            let nanoseconds = remaining_ns(deadline - sample.counter, frequency)?;
            #[cfg(windows)]
            {
                if state.native.is_none() {
                    state.native = Some(Arc::new(windows::Native::new()?));
                    continue;
                }
                let native = state.native.as_ref().ok_or(Error::Failed)?.clone();
                // Reset under the same mutex that protects generation/signals.
                native.reset()?;
                drop(state);
                native.wait(nanoseconds)?;
                state = self.state.lock().map_err(|_| Error::Failed)?;
            }
            #[cfg(not(windows))]
            {
                state = self
                    .changed
                    .wait_timeout(state, std::time::Duration::from_nanos(nanoseconds))
                    .map_err(|_| Error::Failed)?
                    .0;
            }
        }
    }
}
fn is_cancelled(flag: &Option<Arc<AtomicBool>>) -> bool {
    flag.as_ref().is_some_and(|v| v.load(Ordering::Acquire))
}
fn validate_sample(last: Option<(u64, u64)>, counter: u64, frequency: u64) -> Result<()> {
    if counter > i64::MAX as u64
        || frequency == 0
        || frequency > i64::MAX as u64
        || last.is_some_and(|(prior, rate)| counter < prior || frequency != rate)
    {
        return Err(Error::Failed);
    }
    Ok(())
}
fn remaining_ns(counts: u64, frequency: u64) -> Result<u64> {
    if counts == 0 || frequency == 0 || frequency > i64::MAX as u64 || counts > frequency {
        return Err(Error::Invalid);
    }
    let value = (u128::from(counts) * 1_000_000_000).div_ceil(u128::from(frequency));
    u64::try_from(value).map_err(|_| Error::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        thread,
        time::{Duration, Instant},
    };
    #[cfg(any(
        all(
            target_os = "linux",
            target_env = "gnu",
            any(
                all(target_arch = "x86", target_pointer_width = "32"),
                all(target_arch = "x86_64", target_pointer_width = "64")
            )
        ),
        windows
    ))]
    const CLOCK_SUPPORTED: bool = true;
    #[cfg(not(any(
        all(
            target_os = "linux",
            target_env = "gnu",
            any(
                all(target_arch = "x86", target_pointer_width = "32"),
                all(target_arch = "x86_64", target_pointer_width = "64")
            )
        ),
        windows
    )))]
    const CLOCK_SUPPORTED: bool = false;
    #[cfg(not(any(
        all(
            target_os = "linux",
            target_env = "gnu",
            any(
                all(target_arch = "x86", target_pointer_width = "32"),
                all(target_arch = "x86_64", target_pointer_width = "64")
            )
        ),
        windows
    )))]
    #[test]
    fn unsupported_clock_is_explicit() {
        assert!(matches!(performance::sample(), Err(Error::Unsupported)));
    }
    fn waiting(clock: &Clock) {
        let end = Instant::now() + Duration::from_secs(5);
        while !clock.state.lock().unwrap().waiting {
            assert!(Instant::now() < end);
            thread::yield_now();
        }
    }
    #[test]
    fn raw_deadline_scaling_rounds_up_in_fixed_width_units() {
        assert_eq!(remaining_ns(1, 3), Ok(333333334));
        assert_eq!(remaining_ns(1, 1_000_000_000), Ok(1));
        for frequency in [1u64, 3, 10_000_000, 1_000_000_000, i64::MAX as u64] {
            for counts in [1, frequency / 2 + 1, frequency] {
                let value = remaining_ns(counts, frequency).unwrap();
                assert!((1..=1_000_000_000).contains(&value));
                assert!(
                    u128::from(value) * u128::from(frequency) >= u128::from(counts) * 1_000_000_000
                );
                assert!(
                    u128::from(value - 1) * u128::from(frequency)
                        < u128::from(counts) * 1_000_000_000
                );
            }
        }
        for (counts, frequency) in [(0, 1), (1, 0), (2, 1), (u64::MAX, 1), (1, u64::MAX)] {
            assert_eq!(remaining_ns(counts, frequency), Err(Error::Invalid));
        }
        assert_eq!(validate_sample(Some((10, 3)), 9, 3), Err(Error::Failed));
        assert_eq!(validate_sample(Some((10, 3)), 11, 4), Err(Error::Failed));
        assert_eq!(validate_sample(None, 0, 0), Err(Error::Failed));
        assert_eq!(validate_sample(None, u64::MAX, 1), Err(Error::Failed));
        assert_eq!(validate_sample(Some((10, 3)), 10, 3), Ok(()));
    }
    #[test]
    fn invalid_requests_and_owner_dispatch_do_not_claim_or_allocate_a_wait() {
        if !CLOCK_SUPPORTED {
            return;
        }
        let clock = Clock::new();
        let now = clock.snapshot().unwrap();
        let last = clock.state.lock().unwrap().last;
        assert_eq!(
            clock.wait(now.counter, now.frequency, 0, None, false),
            Err(Error::Busy)
        );
        for (deadline, frequency, generation) in [
            (u64::MAX, now.frequency, 0),
            (0, 0, 0),
            (0, u64::MAX, 0),
            (0, now.frequency, 1),
            (now.counter + now.frequency * 3, now.frequency, 0),
            (0, now.frequency + 1, 0),
        ] {
            assert_eq!(
                clock.wait(deadline, frequency, generation, None, true),
                Err(Error::Invalid)
            );
            let state = clock.state.lock().unwrap();
            assert!(!state.waiting);
            assert_eq!(state.last, last);
            #[cfg(windows)]
            assert!(state.native.is_none());
        }
        let result = clock.wait(0, now.frequency, 0, None, true).unwrap();
        assert_eq!(result.reason, 0);
        assert!(result.snapshot.counter >= now.counter);
    }
    #[test]
    fn early_wakes_are_retained_and_deadlines_never_return_early() {
        if !CLOCK_SUPPORTED {
            return;
        }
        let clock = Clock::new();
        let initial = clock.snapshot().unwrap();
        assert_eq!(clock.wake(), Ok(1));
        let wake = clock.wait(0, initial.frequency, 0, None, true).unwrap();
        assert_eq!(wake.reason, 1);
        assert_eq!(wake.snapshot.generation, 1);
        let state = clock.snapshot().unwrap();
        let deadline = state.counter + state.frequency.div_ceil(500); // 2ms request, no wall-time upper-bound claim.
        let result = clock
            .wait(deadline, state.frequency, state.generation, None, true)
            .unwrap();
        assert_eq!(result.reason, 0);
        assert!(result.snapshot.counter >= deadline);
        assert!(!clock.state.lock().unwrap().waiting);
        let start = clock.snapshot().unwrap();
        clock.state.lock().unwrap().generation = u64::MAX;
        assert_eq!(clock.wake(), Err(Error::Failed));
        assert_eq!(clock.snapshot().unwrap().generation, u64::MAX);
        assert!(clock.snapshot().unwrap().counter >= start.counter);
    }
    #[test]
    fn one_waiter_spurious_interrupts_wakeup_close_and_pair_lifetimes() {
        if !CLOCK_SUPPORTED {
            return;
        }
        let clock = Arc::new(Clock::new());
        let initial = clock.snapshot().unwrap();
        let run = clock.clone();
        let waiter = thread::spawn(move || {
            run.wait(
                initial.counter + initial.frequency,
                initial.frequency,
                0,
                None,
                true,
            )
        });
        waiting(&clock);
        assert_eq!(
            clock.wait(0, initial.frequency, 0, None, true),
            Err(Error::Busy)
        );
        clock.interrupt().unwrap();
        thread::sleep(Duration::from_millis(2));
        assert!(!waiter.is_finished());
        assert_eq!(clock.wake(), Ok(1));
        let result = waiter.join().unwrap().unwrap();
        assert_eq!(result.reason, 1);
        assert_eq!(result.snapshot.generation, 1);
        assert!(!clock.state.lock().unwrap().waiting);
        let initial = clock.snapshot().unwrap();
        let run = clock.clone();
        let waiter = thread::spawn(move || {
            run.wait(
                initial.counter + initial.frequency,
                initial.frequency,
                1,
                None,
                true,
            )
        });
        waiting(&clock);
        #[cfg(windows)]
        let retained = clock.state.lock().unwrap().native.as_ref().unwrap().clone();
        clock.close().unwrap();
        assert_eq!(waiter.join().unwrap(), Err(Error::Denied));
        assert_eq!(clock.snapshot(), Err(Error::Denied));
        assert_eq!(clock.wake(), Err(Error::Denied));
        assert!(!clock.state.lock().unwrap().waiting);
        #[cfg(windows)]
        {
            assert!(clock.state.lock().unwrap().native.is_none());
            assert_eq!(Arc::strong_count(&retained), 1);
        }
    }
    #[test]
    fn cancellation_interrupts_without_fabricating_a_wake_generation() {
        if !CLOCK_SUPPORTED {
            return;
        }
        let clock = Arc::new(Clock::new());
        let cancelled = Arc::new(AtomicBool::new(false));
        let initial = clock.snapshot().unwrap();
        let run = clock.clone();
        let flag = cancelled.clone();
        let waiter = thread::spawn(move || {
            run.wait(
                initial.counter + initial.frequency,
                initial.frequency,
                0,
                Some(flag),
                true,
            )
        });
        waiting(&clock);
        cancelled.store(true, Ordering::Release);
        clock.interrupt().unwrap();
        assert_eq!(waiter.join().unwrap(), Err(Error::Denied));
        assert_eq!(clock.snapshot().unwrap().generation, 0);
        assert!(!clock.state.lock().unwrap().waiting);
        let state = clock.snapshot().unwrap();
        assert_eq!(
            clock.wait(state.counter, state.frequency, 0, Some(cancelled), true),
            Err(Error::Denied)
        );
        assert_eq!(
            clock
                .wait(state.counter, state.frequency, 0, None, true)
                .unwrap()
                .reason,
            0
        );
    }
}

#[cfg(test)]
mod runtime_clock_tests {
    use super::super::{Runtime, TaskContext};
    use super::*;
    use std::time::{Duration, Instant};
    #[cfg(any(
        all(
            target_os = "linux",
            target_env = "gnu",
            any(
                all(target_arch = "x86", target_pointer_width = "32"),
                all(target_arch = "x86_64", target_pointer_width = "64")
            )
        ),
        windows
    ))]
    const CLOCK_SUPPORTED: bool = true;
    #[cfg(not(any(
        all(
            target_os = "linux",
            target_env = "gnu",
            any(
                all(target_arch = "x86", target_pointer_width = "32"),
                all(target_arch = "x86_64", target_pointer_width = "64")
            )
        ),
        windows
    )))]
    const CLOCK_SUPPORTED: bool = false;
    fn actor(context: TaskContext, _: u64) -> u64 {
        let state = context.runtime().clock_snapshot().unwrap();
        assert_eq!(
            context.runtime().clock_wait(
                state.counter + state.frequency,
                state.frequency,
                state.generation,
                true
            ),
            Err(Error::Denied)
        );
        assert_eq!(context.runtime().clock_snapshot(), Err(Error::Denied));
        assert_eq!(context.runtime().clock_wake(), Err(Error::Denied));
        17
    }
    #[test]
    fn reviewed_task_cancellation_reaches_the_local_wait_without_generation_change() {
        if !CLOCK_SUPPORTED {
            return;
        }
        let runtime = Runtime::new(&[], vec![actor], 4 * 1024 * 1024, 2, 65536).unwrap();
        let task = runtime.spawn(0, 0, false).unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        while !runtime.0.clock.state.lock().unwrap().waiting {
            assert!(Instant::now() < end);
            std::thread::yield_now();
        }
        runtime.cancel(task).unwrap();
        while !runtime.task_finished(task).unwrap() {
            assert!(Instant::now() < end);
            std::thread::yield_now();
        }
        assert_eq!(runtime.join(task), Ok(17));
        assert_eq!(runtime.clock_snapshot().unwrap().generation, 0);
    }
}
