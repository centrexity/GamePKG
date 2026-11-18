//! Session deadlines, host-owned raw clock samples and bounded worker waits.
use super::{Error, Result};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
mod platform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub counter: u64,
    pub frequency: u64,
    pub uptime_ms: u64,
    pub utc_100ns: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PerformanceSample {
    pub counter: u64,
    pub frequency: u64,
    pub uptime_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Anchor {
    counter: u64,
    frequency: u64,
    utc: u64,
    last: u64,
}

#[derive(Clone)]
pub struct Clock(Arc<Inner>);
struct Inner {
    start: Instant,
    state: Mutex<State>,
    changed: Condvar,
}
#[derive(Default)]
struct State {
    generation: u64,
    closed: bool,
    waiting: bool,
    calendar: Option<Anchor>,
}
impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}
impl Clock {
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            start: Instant::now(),
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }))
    }
    pub fn snapshot(&self) -> Result<(u64, u64)> {
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        if state.closed {
            return Err(Error::Denied);
        }
        Ok((self.now(), state.generation))
    }
    /// Raw host counter/frequency/uptime and once-anchored UTC. The private
    /// transport invokes this on its worker, never the window owner thread.
    pub fn sample(&self) -> Result<Sample> {
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        if state.closed {
            return Err(Error::Denied);
        }
        let utc = if state.calendar.is_none() {
            Some(platform::utc_100ns()?)
        } else {
            None
        };
        let raw = platform::sample()?;
        let utc_100ns = calendar(&mut state.calendar, raw.counter, raw.frequency, utc)?;
        Ok(Sample {
            counter: raw.counter,
            frequency: raw.frequency,
            uptime_ms: raw.uptime_ms,
            utc_100ns,
        })
    }
    /// Counter-only reads never read wall time or initialize/advance the
    /// calendar anchor. A caller's first calendar read retains its own origin.
    pub fn performance(&self) -> Result<PerformanceSample> {
        let state = self.0.state.lock().map_err(|_| Error::Failed)?;
        if state.closed {
            return Err(Error::Denied);
        }
        let raw = platform::sample()?;
        Ok(PerformanceSample {
            counter: raw.counter,
            frequency: raw.frequency,
            uptime_ms: raw.uptime_ms,
        })
    }
    fn now(&self) -> u64 {
        self.0.start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }
    /// Wakeups are generation counted: a wake before wait is not lost.
    pub fn wake(&self) -> Result<()> {
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        if state.closed {
            return Err(Error::Denied);
        }
        state.generation = state.generation.checked_add(1).ok_or(Error::Failed)?;
        self.0.changed.notify_all();
        Ok(())
    }
    pub fn close(&self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.closed = true;
            self.0.changed.notify_all();
        }
    }
    /// Call only on a dedicated connection worker. Absolute session ns deadline;
    /// at most one waiter and at most one second into the future. Spurious wakes
    /// do not complete a wait. Returns (time, generation, wake reason 0/1).
    pub fn wait(&self, deadline: u64, generation: u64) -> Result<(u64, u64, u32)> {
        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;
        if state.closed {
            return Err(Error::Denied);
        }
        if deadline.saturating_sub(self.now()) > 1_000_000_000 || generation > state.generation {
            return Err(Error::Invalid);
        }
        if state.waiting {
            return Err(Error::Busy);
        }
        state.waiting = true;
        loop {
            if state.closed {
                state.waiting = false;
                return Err(Error::Denied);
            }
            let now = self.now();
            if state.generation != generation || now >= deadline {
                state.waiting = false;
                return Ok((
                    now,
                    state.generation,
                    u32::from(state.generation != generation),
                ));
            }
            state = self
                .0
                .changed
                .wait_timeout(state, Duration::from_nanos(deadline - now))
                .map_err(|_| Error::Failed)?
                .0;
        }
    }
    pub(crate) fn call(&self, op: u32, bytes: &[u8], cap: u32, worker: bool) -> Result<Vec<u8>> {
        match op {
            60 if bytes.is_empty() && cap >= 16 => {
                let (now, generation) = self.snapshot()?;
                Ok([now.to_le_bytes(), generation.to_le_bytes()].concat())
            }
            61 if bytes.len() == 16 && cap >= 20 => {
                if !worker {
                    return Err(Error::Busy);
                }
                let deadline = u64::from_le_bytes(bytes[..8].try_into().unwrap());
                let generation = u64::from_le_bytes(bytes[8..].try_into().unwrap());
                let (now, generation, reason) = self.wait(deadline, generation)?;
                Ok([
                    now.to_le_bytes().as_slice(),
                    &generation.to_le_bytes(),
                    &reason.to_le_bytes(),
                ]
                .concat())
            }
            62 if bytes.is_empty() => {
                self.wake()?;
                Ok(Vec::new())
            }
            65 if bytes.is_empty() && cap >= 32 => {
                if !worker {
                    return Err(Error::Busy);
                }
                let sample = self.sample()?;
                Ok([
                    sample.counter.to_le_bytes(),
                    sample.frequency.to_le_bytes(),
                    sample.uptime_ms.to_le_bytes(),
                    sample.utc_100ns.to_le_bytes(),
                ]
                .concat())
            }
            66 if bytes.is_empty() && cap >= 24 => {
                if !worker {
                    return Err(Error::Busy);
                }
                let sample = self.performance()?;
                Ok([
                    sample.counter.to_le_bytes(),
                    sample.frequency.to_le_bytes(),
                    sample.uptime_ms.to_le_bytes(),
                ]
                .concat())
            }
            _ => Err(Error::Invalid),
        }
    }
}

// Do not mutate the anchor when invalid/regressing/frequency-changing input or
// arithmetic overflow is detected. Absolute clock jumps after initialization
// cannot make this session's calendar go backwards.
fn calendar(
    anchor: &mut Option<Anchor>,
    counter: u64,
    frequency: u64,
    utc: Option<u64>,
) -> Result<u64> {
    if frequency == 0 || frequency > i64::MAX as u64 || counter > i64::MAX as u64 {
        return Err(Error::Failed);
    }
    if let Some(value) = anchor {
        if value.frequency != frequency || counter < value.last {
            return Err(Error::Failed);
        }
        let delta = u128::from(counter - value.counter) * 10_000_000 / u128::from(frequency);
        let result = value
            .utc
            .checked_add(u64::try_from(delta).map_err(|_| Error::Failed)?)
            .ok_or(Error::Failed)?;
        value.last = counter;
        Ok(result)
    } else {
        let utc = utc.ok_or(Error::Failed)?;
        *anchor = Some(Anchor {
            counter,
            frequency,
            utc,
            last: counter,
        });
        Ok(utc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadlines_wakes_revocation_and_owner_thread_denial() {
        let clock = Clock::new();
        let (now, generation) = clock.snapshot().unwrap();
        assert_eq!(
            clock.wait(now + 2_000_000_000, generation),
            Err(Error::Invalid)
        );
        assert_eq!(clock.wait(now, generation + 1), Err(Error::Invalid));
        assert_eq!(clock.wait(now, generation).unwrap().2, 0);
        clock.wake().unwrap();
        assert_eq!(clock.wait(now + 500_000_000, generation).unwrap().2, 1);
        assert_eq!(clock.call(61, &[0; 16], 20, false), Err(Error::Busy));
        let clone = clock.clone();
        let (now, generation) = clock.snapshot().unwrap();
        let worker = std::thread::spawn(move || clone.wait(now + 500_000_000, generation));
        clock.close();
        assert_eq!(worker.join().unwrap(), Err(Error::Denied));
        assert_eq!(clock.snapshot(), Err(Error::Denied));
    }
    #[test]
    fn raw_calendar_scaling_is_anchored_checked_and_never_accumulates_rounding() {
        for frequency in [
            1u64,
            3,
            10_000_000,
            1_000_000_000,
            3_579_545,
            i64::MAX as u64,
        ] {
            let origin = 100u64;
            let mut anchor = None;
            assert_eq!(calendar(&mut anchor, origin, frequency, Some(500)), Ok(500));
            for delta in [0u64, 1, 2, 3, 99, 10_000, 10_000_000] {
                let expected = 500
                    + u64::try_from(u128::from(delta) * 10_000_000 / u128::from(frequency))
                        .unwrap();
                assert_eq!(
                    calendar(&mut anchor, origin + delta, frequency, Some(1)),
                    Ok(expected)
                );
            }
            let before = anchor;
            assert_eq!(
                calendar(&mut anchor, origin + 2, frequency, None),
                Err(Error::Failed)
            );
            assert_eq!(anchor, before);
        }
        let mut anchor = None;
        for (counter, frequency) in [(0, 0), (u64::MAX, 1), (0, u64::MAX)] {
            assert_eq!(
                calendar(&mut anchor, counter, frequency, Some(1)),
                Err(Error::Failed)
            );
            assert!(anchor.is_none());
        }
        assert_eq!(calendar(&mut anchor, 0, 1, None), Err(Error::Failed));
        assert_eq!(calendar(&mut anchor, 0, 1, Some(u64::MAX)), Ok(u64::MAX));
        let before = anchor;
        assert_eq!(calendar(&mut anchor, 1, 1, None), Err(Error::Failed));
        assert_eq!(anchor, before);
        assert_eq!(calendar(&mut anchor, 0, 2, None), Err(Error::Failed));
        assert_eq!(anchor, before);
        // More than a day at 10 MHz exceeds the naive u64 count*10^7 product.
        let mut anchor = None;
        calendar(&mut anchor, 1, 10_000_000, Some(10)).unwrap();
        assert_eq!(
            calendar(
                &mut anchor,
                1 + 10_000_000 * 60 * 60 * 24 * 365,
                10_000_000,
                None
            ),
            Ok(10 + 10_000_000 * 60 * 60 * 24 * 365)
        );
    }
    #[test]
    fn worker_only_clock_frames_validate_before_anchor_and_revoke() {
        let clock = Clock::new();
        for (bytes, capacity) in [(&[1u8][..], 32), (&[][..], 31)] {
            assert_eq!(clock.call(65, bytes, capacity, true), Err(Error::Invalid));
        }
        assert_eq!(clock.call(65, &[], 32, false), Err(Error::Busy));
        assert!(clock.0.state.lock().unwrap().calendar.is_none());
        let first = clock.sample().unwrap();
        let reply = clock.call(65, &[], 32, true).unwrap();
        assert_eq!(reply.len(), 32);
        let word =
            |offset: usize| u64::from_le_bytes(reply[offset..offset + 8].try_into().unwrap());
        assert!(word(0) >= first.counter);
        assert_eq!(word(8), first.frequency);
        assert!(word(16) >= first.uptime_ms && word(24) >= first.utc_100ns);
        let anchor = clock.0.state.lock().unwrap().calendar.unwrap();
        assert_eq!(
            word(24),
            anchor.utc
                + u64::try_from(
                    u128::from(word(0) - anchor.counter) * 10_000_000
                        / u128::from(anchor.frequency)
                )
                .unwrap()
        );
        clock.close();
        assert_eq!(clock.call(65, &[], 32, true), Err(Error::Denied));
        assert_eq!(clock.sample(), Err(Error::Denied));
    }
    #[test]
    fn performance_reads_never_initialize_or_advance_calendar_and_revoke() {
        let clock = Clock::new();
        for capacity in [0, 23] {
            assert_eq!(clock.call(66, &[], capacity, true), Err(Error::Invalid));
        }
        assert_eq!(clock.call(66, &[0], 24, true), Err(Error::Invalid));
        assert_eq!(clock.call(66, &[], 24, false), Err(Error::Busy));
        let before = clock.performance().unwrap();
        let packet = clock.call(66, &[], 24, true).unwrap();
        let after = clock.performance().unwrap();
        assert_eq!(packet.len(), 24);
        let word = |at| u64::from_le_bytes(packet[at..at + 8].try_into().unwrap());
        assert!(word(0) >= before.counter && word(0) <= after.counter);
        assert_eq!(word(8), before.frequency);
        assert!(word(16) >= before.uptime_ms && word(16) <= after.uptime_ms);
        assert!(clock.0.state.lock().unwrap().calendar.is_none());
        // The first calendar sample anchors at its own counter, even after
        // successful performance reads and rejected malformed packets.
        let first = clock.sample().unwrap();
        let anchor = clock.0.state.lock().unwrap().calendar.unwrap();
        assert_eq!(anchor.counter, first.counter);
        assert_eq!(anchor.utc, first.utc_100ns);
        for _ in 0..16 {
            assert!(clock.performance().unwrap().counter >= first.counter);
        }
        assert_eq!(clock.0.state.lock().unwrap().calendar, Some(anchor));
        let other = Clock::new();
        other.performance().unwrap();
        assert!(other.0.state.lock().unwrap().calendar.is_none());
        clock.close();
        assert_eq!(clock.performance(), Err(Error::Denied));
        assert_eq!(clock.call(66, &[], 24, true), Err(Error::Denied));
        assert!(other.performance().is_ok());
    }
    #[test]
    fn timeout_uses_monotonic_clock() {
        let clock = Clock::new();
        let (now, generation) = clock.snapshot().unwrap();
        let end = now + 2_000_000;
        let (actual, _, reason) = clock.wait(end, generation).unwrap();
        assert!(actual >= end);
        assert_eq!(reason, 0);
    }
}
