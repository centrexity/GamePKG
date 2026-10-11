//! Background signed-update checks. Downloads are discarded; installation stays explicit.
use crate::{manifest::Manifest, registry};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Available(String),
    Current,
    Failed(String),
}
struct Report {
    id: String,
    identity: String,
    root: PathBuf,
    notice: Notice,
}
#[derive(Default)]
pub struct Monitor {
    root: Option<PathBuf>,
    policy: Option<(bool, u64)>,
    due: HashMap<String, (String, Instant)>,
    pub notices: HashMap<String, Notice>,
    receiver: Option<mpsc::Receiver<Report>>,
}
fn identity(game: &Manifest) -> String {
    crate::verify::digest(
        format!(
            "{}\0{}\0{}\0{}",
            game.id,
            game.version,
            game.creator_key,
            game.update_url.as_deref().unwrap_or("")
        )
        .as_bytes(),
    )
}
impl Monitor {
    /// The GUI calls this only while the library is idle. One background request at a time.
    pub fn tick(
        &mut self,
        root: &Path,
        games: &[Manifest],
        enabled: bool,
        hours: u64,
        now: Instant,
    ) {
        if self.root.as_deref() != Some(root) {
            self.root = Some(root.to_owned());
            self.due.clear();
            self.notices.clear();
        }
        if self.policy != Some((enabled, hours)) {
            self.due.clear();
            self.policy = Some((enabled, hours));
        }
        let identities: HashMap<&str, String> = games
            .iter()
            .map(|game| (game.id.as_str(), identity(game)))
            .collect();
        self.due
            .retain(|id, _| identities.contains_key(id.as_str()));
        self.notices
            .retain(|id, _| identities.contains_key(id.as_str()));
        for (id, key) in &identities {
            if self.due.get(*id).is_some_and(|(old, _)| old != key) {
                self.due.remove(*id);
                self.notices.remove(*id);
            }
        }
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(report) => {
                    if enabled
                        && report.root == root
                        && identities
                            .get(report.id.as_str())
                            .is_some_and(|key| key == &report.identity)
                    {
                        self.notices.insert(report.id, report.notice);
                    }
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.receiver = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if !enabled || !(1..=168).contains(&hours) || self.receiver.is_some() {
            return;
        }
        if let Some(game) = games
            .iter()
            .filter(|game| {
                game.update_url.is_some()
                    && self.due.get(&game.id).is_none_or(|(_, due)| now >= *due)
            })
            .min_by_key(|game| self.due.get(&game.id).map(|(_, due)| *due))
        {
            let id = game.id.clone();
            let key = identity(game);
            let root = root.to_owned();
            self.due.insert(
                id.clone(),
                (key.clone(), now + Duration::from_secs(hours * 3600)),
            );
            let (tx, rx) = mpsc::channel();
            self.receiver = Some(rx);
            std::thread::spawn(move || {
                let notice = match registry::check_update(&root, &id) {
                    Ok(Some((manifest, _download))) => Notice::Available(manifest.version),
                    Ok(None) => Notice::Current,
                    Err(error) => Notice::Failed(format!("{error:#}").chars().take(1024).collect()),
                };
                let _ = tx.send(Report {
                    id,
                    identity: key,
                    root,
                    notice,
                });
            });
        }
    }
    pub fn checking(&self) -> bool {
        self.receiver.is_some()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Manifest {
        Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap()
    }
    #[test]
    fn completed_checks_are_discarded_after_library_identity_or_policy_changes() {
        let mut game = game();
        let mut monitor = Monitor::default();
        let root = Path::new("/tmp/library-a");
        let now = Instant::now();
        monitor.tick(root, std::slice::from_ref(&game), false, 24, now);
        for change in [0, 1, 2] {
            let (tx, rx) = mpsc::channel();
            let key = identity(&game);
            tx.send(Report {
                id: game.id.clone(),
                identity: key,
                root: root.to_owned(),
                notice: Notice::Available("0.2.0".into()),
            })
            .unwrap();
            monitor.receiver = Some(rx);
            if change == 0 {
                game.version = "0.3.0".into();
            }
            let next_root = if change == 1 {
                Path::new("/tmp/library-b")
            } else {
                root
            };
            monitor.tick(next_root, std::slice::from_ref(&game), change != 2, 24, now);
            assert!(monitor.notices.is_empty());
        }
    }
    #[test]
    fn matching_reports_are_visible_and_no_url_or_disabled_checks_do_not_start() {
        let mut game = game();
        let root = Path::new("/tmp/library");
        let now = Instant::now();
        let mut monitor = Monitor::default();
        monitor.tick(root, std::slice::from_ref(&game), true, 24, now);
        assert!(!monitor.checking());
        let (tx, rx) = mpsc::channel();
        tx.send(Report {
            id: game.id.clone(),
            identity: identity(&game),
            root: root.to_owned(),
            notice: Notice::Available("0.2.0".into()),
        })
        .unwrap();
        monitor.receiver = Some(rx);
        monitor.tick(root, std::slice::from_ref(&game), true, 24, now);
        assert_eq!(
            monitor.notices.get(&game.id),
            Some(&Notice::Available("0.2.0".into()))
        );
        game.update_url = Some("https://example.test/latest.gamepkg".into());
        monitor.tick(root, std::slice::from_ref(&game), false, 24, now);
        assert!(!monitor.checking());
    }
    #[test]
    fn checks_wait_for_deadlines_and_reschedule_failures_without_a_retry_loop() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let mut game = game();
        game.update_url = Some("https://example.test/unused".into());
        let now = Instant::now();
        let mut monitor = Monitor {
            root: Some(root.to_owned()),
            policy: Some((true, 1)),
            ..Default::default()
        };
        monitor.due.insert(
            game.id.clone(),
            (identity(&game), now + Duration::from_secs(3600)),
        );
        monitor.tick(root, std::slice::from_ref(&game), true, 1, now);
        assert!(!monitor.checking());
        let deadline = now + Duration::from_secs(3600);
        // Empty library fails before transport; this test never contacts the URL.
        monitor.tick(root, std::slice::from_ref(&game), true, 1, deadline);
        assert!(monitor.checking());
        assert_eq!(
            monitor.due.get(&game.id).unwrap().1,
            deadline + Duration::from_secs(3600)
        );
        monitor.tick(
            root,
            std::slice::from_ref(&game),
            true,
            1,
            deadline + Duration::from_secs(1),
        );
        assert_eq!(
            monitor.due.get(&game.id).unwrap().1,
            deadline + Duration::from_secs(3600)
        );
    }
    #[test]
    fn unchecked_games_are_not_starved_by_overdue_games() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let now = Instant::now();
        let mut old = game();
        old.update_url = Some("https://example.test/unused".into());
        let mut new = game();
        new.id = "com.gamepkg.second".into();
        new.update_url = old.update_url.clone();
        let mut monitor = Monitor {
            root: Some(root.to_owned()),
            policy: Some((true, 1)),
            ..Default::default()
        };
        monitor.due.insert(old.id.clone(), (identity(&old), now));
        monitor.tick(root, &[old, new], true, 1, now);
        assert!(monitor.due.contains_key("com.gamepkg.second"));
    }
}
