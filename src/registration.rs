//! Persistent first-run timestamp and unregistered trial banner.
//!
//! Trial play is never blocked. The banner offers the project's registration
//! link and a code-entry dialog, but no redemption service is configured yet,
//! so entered codes are never accepted or persisted. Registration state is kept
//! separate from user-editable settings so a normal Settings save cannot
//! restart the 30-day countdown.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const TRIAL_DAYS: u64 = 30;
pub const REGISTRATION_PRICE: &str = "$9.99";
const SECONDS_PER_DAY: u64 = 86_400;
const STATE_FILE: &str = "gamepkg-registration.json";
const MAX_STATE_BYTES: u64 = 4096;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
    pub installed_at_unix: Option<i64>,
    pub registered: bool,
}

fn unix_time(now: SystemTime) -> i64 {
    now.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

fn recorded_installation_time(modified: Option<SystemTime>, now: SystemTime) -> SystemTime {
    modified.filter(|time| *time <= now).unwrap_or(now)
}

impl State {
    pub fn starting_at_installation() -> Self {
        let now = SystemTime::now();
        let modified = std::env::current_exe()
            .ok()
            .and_then(|path| path.metadata().ok())
            .and_then(|metadata| metadata.modified().ok());
        Self::starting_at(recorded_installation_time(modified, now))
    }

    fn starting_at(installed_at: SystemTime) -> Self {
        Self {
            installed_at_unix: Some(unix_time(installed_at)),
            registered: false,
        }
    }

    pub fn days_remaining(&self, now: SystemTime) -> u64 {
        let installed = self.installed_at_unix.unwrap_or_else(|| unix_time(now));
        let elapsed = unix_time(now).saturating_sub(installed).max(0) as u64;
        TRIAL_DAYS.saturating_sub(elapsed / SECONDS_PER_DAY)
    }

    pub fn banner(&self, now: SystemTime) -> Option<String> {
        if self.registered {
            return None;
        }
        let days = self.days_remaining(now);
        if days == 0 {
            Some(format!(
                "Unregistered - Over due. Registration is {REGISTRATION_PRICE}."
            ))
        } else if days == 1 {
            Some(format!(
                "Unregistered - 1 day left. Registration is {REGISTRATION_PRICE}."
            ))
        } else {
            Some(format!(
                "Unregistered - {days} days left. Registration is {REGISTRATION_PRICE}."
            ))
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(&serde_json::to_vec(self)?)?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
}

pub fn path() -> Result<PathBuf> {
    Ok(crate::settings::path()?.with_file_name(STATE_FILE))
}

pub fn load() -> Result<State> {
    load_at(&path()?)
}

fn load_at(path: &Path) -> Result<State> {
    if path.exists() {
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(MAX_STATE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            bail!("registration state is too large");
        }
        if let Ok(mut state) = serde_json::from_slice::<State>(&bytes) {
            if state.installed_at_unix.is_none() {
                state = State::starting_at_installation();
                state.save(path)?;
            }
            return Ok(state);
        }
    }
    let state = State::starting_at_installation();
    state.save(path)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn state_round_trips_and_missing_file_records_first_run() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config/gamepkg-registration.json");
        let first = load_at(&path).unwrap();
        let installed = first.installed_at_unix.unwrap();
        assert!(installed <= unix_time(SystemTime::now()));
        assert_eq!(load_at(&path).unwrap(), first);
        assert!(!first.registered);
    }

    #[test]
    fn installation_time_uses_executable_timestamp_but_never_the_future() {
        let now = UNIX_EPOCH + Duration::from_secs(2_000_000_000);
        let installed = now - Duration::from_secs(3 * SECONDS_PER_DAY);
        assert_eq!(recorded_installation_time(Some(installed), now), installed);
        assert_eq!(
            recorded_installation_time(Some(now + Duration::from_secs(60)), now),
            now
        );
        assert_eq!(recorded_installation_time(None, now), now);

        let state = State::starting_at(installed);
        assert_eq!(state.installed_at_unix, Some(unix_time(installed)));
        assert_eq!(state.days_remaining(now), TRIAL_DAYS - 3);
    }

    #[test]
    fn countdown_is_clamped_and_switches_to_over_due() {
        let now = SystemTime::now();
        let mut state = State {
            installed_at_unix: Some(unix_time(now)),
            registered: false,
        };
        assert_eq!(state.days_remaining(now), TRIAL_DAYS);
        assert_eq!(
            state.banner(now).as_deref(),
            Some("Unregistered - 30 days left. Registration is $9.99.")
        );

        state.installed_at_unix = Some(unix_time(now) - (29 * SECONDS_PER_DAY) as i64);
        assert_eq!(
            state.banner(now).as_deref(),
            Some("Unregistered - 1 day left. Registration is $9.99.")
        );

        for days in [30, 31, 365] {
            state.installed_at_unix = Some(unix_time(now) - (days * SECONDS_PER_DAY) as i64);
            assert_eq!(state.days_remaining(now), 0);
            assert_eq!(
                state.banner(now).as_deref(),
                Some("Unregistered - Over due. Registration is $9.99.")
            );
        }

        state.installed_at_unix = Some(unix_time(now) + SECONDS_PER_DAY as i64);
        assert_eq!(state.days_remaining(now), TRIAL_DAYS);
    }

    #[test]
    fn registered_state_hides_banner() {
        let state = State {
            installed_at_unix: Some(unix_time(SystemTime::now())),
            registered: true,
        };
        assert_eq!(state.banner(SystemTime::now()), None);
    }

    #[test]
    fn oversized_or_invalid_state_starts_fresh() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("gamepkg-registration.json");
        fs::write(&path, vec![b'x'; (MAX_STATE_BYTES + 1) as usize]).unwrap();
        assert!(load_at(&path).is_err());
        fs::write(&path, b"not json").unwrap();
        let state = load_at(&path).unwrap();
        assert!(state.installed_at_unix.is_some());
        assert_eq!(load_at(&path).unwrap(), state);

        let future = SystemTime::now() + Duration::from_secs(60);
        assert_eq!(state.days_remaining(future), TRIAL_DAYS);
    }
}
