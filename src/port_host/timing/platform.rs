//! Audited host clock reads. No platform handle/address is returned to games.
use super::super::{Error, Result};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Raw {
    pub counter: u64,
    pub frequency: u64,
    pub uptime_ms: u64,
}
#[cfg(all(
    unix,
    not(all(
        target_os = "linux",
        target_env = "gnu",
        any(
            all(target_arch = "x86", target_pointer_width = "32"),
            all(target_arch = "x86_64", target_pointer_width = "64")
        )
    ))
))]
pub(super) fn sample() -> Result<Raw> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid, writable timespec is supplied to a nonblocking read.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0
        || time.tv_sec < 0
        || !(0..1_000_000_000).contains(&time.tv_nsec)
    {
        return Err(Error::Failed);
    }
    let counter = (time.tv_sec as u64)
        .checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(time.tv_nsec as u64))
        .ok_or(Error::Failed)?;
    if counter > i64::MAX as u64 {
        return Err(Error::Failed);
    }
    Ok(Raw {
        counter,
        frequency: 1_000_000_000,
        uptime_ms: counter / 1_000_000,
    })
}
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
pub(super) fn sample() -> Result<Raw> {
    use super::super::local_runtime::{performance, Error as LocalError};
    let raw = performance::sample().map_err(|error| match error {
        LocalError::Invalid => Error::Invalid,
        LocalError::Busy => Error::Busy,
        LocalError::Failed => Error::Failed,
        LocalError::Denied => Error::Denied,
        LocalError::Unsupported => Error::Unsupported,
    })?;
    Ok(Raw {
        counter: raw.counter,
        frequency: raw.frequency,
        uptime_ms: raw.uptime_ms,
    })
}
#[cfg(not(any(unix, windows)))]
pub(super) fn sample() -> Result<Raw> {
    Err(Error::Unsupported)
}

pub(super) fn utc_100ns() -> Result<u64> {
    ticks(std::time::SystemTime::now())
}
fn ticks(time: std::time::SystemTime) -> Result<u64> {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(delta) => unix_ticks(false, delta.as_nanos()),
        Err(before) => unix_ticks(true, before.duration().as_nanos()),
    }
}
fn unix_ticks(before: bool, nanos: u128) -> Result<u64> {
    const EPOCH: u128 = 116_444_736_000_000_000;
    let value = if before {
        EPOCH.checked_sub(nanos.div_ceil(100))
    } else {
        EPOCH.checked_add(nanos / 100)
    }
    .ok_or(Error::Failed)?;
    u64::try_from(value).map_err(|_| Error::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};
    #[test]
    fn calendar_epoch_and_fraction_rounding_use_fixed_width_units() {
        const EPOCH: u64 = 116_444_736_000_000_000;
        assert_eq!(ticks(UNIX_EPOCH), Ok(EPOCH));
        assert_eq!(
            ticks(UNIX_EPOCH + Duration::from_secs(1)),
            Ok(EPOCH + 10_000_000)
        );
        assert_eq!(
            ticks(UNIX_EPOCH - Duration::from_secs(1)),
            Ok(EPOCH - 10_000_000)
        );
        // Exercise sub-100ns math independently of SystemTime's platform
        // precision; Windows SystemTime quantizes durations to 100ns first.
        assert_eq!(unix_ticks(false, 199), Ok(EPOCH + 1));
        assert_eq!(unix_ticks(true, 1), Ok(EPOCH - 1));
        assert_eq!(unix_ticks(true, 100), Ok(EPOCH - 1));
        assert_eq!(unix_ticks(true, 101), Ok(EPOCH - 2));
        assert_eq!(unix_ticks(true, u128::from(EPOCH) * 100), Ok(0));
        assert_eq!(
            unix_ticks(true, u128::from(EPOCH) * 100 + 1),
            Err(Error::Failed)
        );
        assert_eq!(
            unix_ticks(false, u128::from(u64::MAX) * 100),
            Err(Error::Failed)
        );
    }
    #[test]
    fn actual_platform_counter_is_monotonic_and_frequency_stays_fixed() {
        let first = sample().unwrap();
        let second = sample().unwrap();
        assert!(first.frequency > 0 && first.frequency <= i64::MAX as u64);
        assert_eq!(first.frequency, second.frequency);
        assert!(second.counter >= first.counter && second.uptime_ms >= first.uptime_ms);
    }
}
