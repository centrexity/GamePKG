//! Relative monotonic time sampled once at the beginning of each guest tick.
//! Reading the sample neither queries the clock again nor exposes calendar time.
use std::time::Instant;

#[derive(Default)]
pub(crate) struct Clock {
    origin: Option<Instant>,
    sample: u64,
}
impl Clock {
    pub fn begin(&mut self) {
        self.sample_at(Instant::now());
    }
    fn sample_at(&mut self, now: Instant) {
        let origin = *self.origin.get_or_insert(now);
        let elapsed = now.checked_duration_since(origin).unwrap_or_default();
        self.sample = self
            .sample
            .max(elapsed.as_micros().min(u128::from(u64::MAX)) as u64);
    }
    pub fn micros(&self) -> u64 {
        self.sample
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn samples_are_relative_stable_monotonic_and_wider_than_u32() {
        let start = Instant::now();
        let mut clock = Clock::default();
        assert_eq!(clock.micros(), 0);
        clock.sample_at(start);
        assert_eq!(clock.micros(), 0);
        clock.sample_at(start + Duration::from_micros((1u64 << 32) + 19));
        assert_eq!(clock.micros(), (1u64 << 32) + 19);
        clock.sample_at(start);
        assert_eq!(clock.micros(), (1u64 << 32) + 19);
        let mut independent = Clock::default();
        independent.sample_at(start + Duration::from_secs(99));
        assert_eq!(independent.micros(), 0);
    }
}
