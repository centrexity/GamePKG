//! Host-owned play history, scoped to a signed game and creator.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub played_ms: u64,
    pub unlocked: BTreeMap<String, u64>,
}
pub(crate) fn path(root: &Path, id: &str, creator: &str) -> Result<PathBuf> {
    let directory = root.join("progress");
    if directory
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir())
    {
        bail!("progress folder must be a directory, not a symlink");
    }
    Ok(directory.join(format!(
        "{}.json",
        crate::verify::digest(format!("GamePKG progress v1\0{id}\0{creator}").as_bytes())
    )))
}
fn read(path: &Path) -> Result<Record> {
    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink() || !m.is_file())
    {
        bail!("progress must be a regular file");
    }
    let mut bytes = Vec::new();
    match std::fs::File::open(path) {
        Ok(file) => {
            file.take(65537).read_to_end(&mut bytes)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Record::default()),
        Err(e) => return Err(e.into()),
    }
    if bytes.len() > 65536 {
        bail!("progress exceeds 64 KiB");
    }
    let record: Record = serde_json::from_slice(&bytes)?;
    if record.unlocked.len() > 1024 || record.unlocked.keys().any(|id| !valid_id(id)) {
        bail!("invalid achievement history");
    }
    Ok(record)
}
pub fn load(root: &Path, id: &str, creator: &str) -> Result<Record> {
    read(&path(root, id, creator)?)
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
pub fn duration(ms: u64) -> String {
    let seconds = ms / 1000;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds / 60) % 60)
    }
}
struct HistoryLock(std::fs::File);
impl Drop for HistoryLock {
    fn drop(&mut self) {
        // Explicitly release the OS lock even if a cloned/inherited descriptor
        // outlives this writer. Merely closing our descriptor may retain it.
        let _ = self.0.unlock();
    }
}
pub(crate) struct Session {
    path: PathBuf,
    last: Option<Instant>,
    pending_ms: u64,
    remainder_ns: u128,
    pending: BTreeMap<String, u64>,
    known: BTreeMap<String, u64>,
    checkpoint: Instant,
}
impl Session {
    pub fn open(root: &Path, id: &str, creator: &str) -> Result<Self> {
        let path = path(root, id, creator)?;
        let known = read(&path)?.unlocked;
        Ok(Self {
            path,
            last: None,
            pending_ms: 0,
            remainder_ns: 0,
            pending: BTreeMap::new(),
            known,
            checkpoint: Instant::now(),
        })
    }
    pub fn start(&mut self) {
        if self.last.is_none() {
            self.last = Some(Instant::now());
        }
    }
    fn accrue(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last.replace(now) {
            let nanos = self.remainder_ns.saturating_add(last.elapsed().as_nanos());
            self.pending_ms = self
                .pending_ms
                .saturating_add((nanos / 1_000_000).min(u128::from(u64::MAX)) as u64);
            self.remainder_ns = nanos % 1_000_000;
        }
    }
    pub fn tick(&mut self, achievements: &[String]) -> Result<()> {
        self.accrue();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        for id in achievements {
            if !self.known.contains_key(id) {
                self.pending.entry(id.clone()).or_insert(timestamp);
            }
        }
        if !self.pending.is_empty() || self.checkpoint.elapsed().as_secs() >= 10 {
            self.commit()?;
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Result<()> {
        if self.last.is_some() {
            self.accrue();
            self.last = None;
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.commit()?;
            if self.pending_ms == 0 && self.pending.is_empty() {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!("play history is busy; unable to flush this session");
            }
            std::thread::sleep(remaining.min(Duration::from_millis(5)));
        }
    }
    fn commit(&mut self) -> Result<()> {
        if self.pending_ms == 0 && self.pending.is_empty() {
            return Ok(());
        }
        let directory = self.path.parent().unwrap();
        std::fs::create_dir_all(directory)?;
        let lock = self.path.with_extension("lock");
        if lock
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink() || !m.is_file())
        {
            bail!("progress lock must be a regular file");
        }
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock)?;
        // OS locks release automatically when a process closes or crashes.
        match handle.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
        let _lock = HistoryLock(handle);
        let mut record = read(&self.path)?;
        record.played_ms = record.played_ms.saturating_add(self.pending_ms);
        for (id, when) in &self.pending {
            record.unlocked.entry(id.clone()).or_insert(*when);
        }
        if record.unlocked.len() > 1024 {
            bail!("too many historical achievements");
        }
        let bytes = serde_json::to_vec(&record)?;
        if bytes.len() > 65536 {
            bail!("progress exceeds 64 KiB");
        }
        let mut file = tempfile::NamedTempFile::new_in(directory)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        file.persist(&self.path).map_err(|e| e.error)?;
        self.pending_ms = 0;
        self.pending.clear();
        self.known = record.unlocked;
        self.checkpoint = Instant::now();
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Err(e) = self.finish() {
            eprintln!("Unable to save play history: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_sessions_merge_time_and_keep_first_unlock() {
        let root = tempfile::tempdir().unwrap();
        let mut first = Session::open(root.path(), "game", "creator").unwrap();
        let mut second = Session::open(root.path(), "game", "creator").unwrap();
        first.start();
        second.start();
        std::thread::sleep(std::time::Duration::from_millis(20));
        first.tick(&["first".into()]).unwrap();
        first.finish().unwrap();
        let initial = load(root.path(), "game", "creator").unwrap();
        second.tick(&["first".into(), "second".into()]).unwrap();
        second.finish().unwrap();
        let record = load(root.path(), "game", "creator").unwrap();
        assert!(record.played_ms >= initial.played_ms + 10);
        assert_eq!(record.unlocked["first"], initial.unlocked["first"]);
        assert_eq!(record.unlocked.len(), 2);
    }
    #[test]
    fn busy_history_defers_without_losing_unlocks() {
        let root = tempfile::tempdir().unwrap();
        let mut session = Session::open(root.path(), "game", "creator").unwrap();
        std::fs::create_dir_all(session.path.parent().unwrap()).unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(session.path.with_extension("lock"))
            .unwrap();
        lock.lock().unwrap();
        session.start();
        session.tick(&["started".into()]).unwrap();
        assert!(!session.pending.is_empty());
        drop(lock);
        session.finish().unwrap();
        assert!(load(root.path(), "game", "creator")
            .unwrap()
            .unlocked
            .contains_key("started"));
    }
    #[test]
    fn simultaneous_writers_preserve_every_session_and_first_unlock() {
        let root = tempfile::tempdir().unwrap();
        let mut first = Session::open(root.path(), "game", "creator").unwrap();
        first.pending.insert("first".into(), 111);
        first.finish().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        std::thread::scope(|scope| {
            for index in 0..8 {
                let barrier = barrier.clone();
                let root = root.path();
                scope.spawn(move || {
                    let mut session = Session::open(root, "game", "creator").unwrap();
                    session.pending_ms = 1000 + index;
                    session.pending.insert("first".into(), 999);
                    session.pending.insert(format!("worker_{index}"), 222);
                    barrier.wait();
                    session.finish().unwrap();
                });
            }
        });
        let record = load(root.path(), "game", "creator").unwrap();
        assert_eq!(
            record.played_ms,
            (0..8).map(|index| 1000 + index).sum::<u64>()
        );
        assert_eq!(record.unlocked["first"], 111);
        assert_eq!(record.unlocked.len(), 9);
        for index in 0..8 {
            assert_eq!(record.unlocked[&format!("worker_{index}")], 222);
        }
    }
    #[test]
    fn writer_unlocks_even_if_a_duplicate_descriptor_outlives_it() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("history.lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.lock().unwrap();
        let duplicate = file.try_clone().unwrap();
        let writer = HistoryLock(file);
        let next = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert!(matches!(
            next.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        drop(writer);
        next.try_lock().unwrap();
        next.unlock().unwrap();
        drop(duplicate);
    }
    #[test]
    fn finish_waits_for_brief_contention_and_preserves_pending_history() {
        let root = tempfile::tempdir().unwrap();
        let mut session = Session::open(root.path(), "game", "creator").unwrap();
        std::fs::create_dir_all(session.path.parent().unwrap()).unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(session.path.with_extension("lock"))
            .unwrap();
        lock.lock().unwrap();
        session.pending_ms = 1234;
        session.tick(&["started".into()]).unwrap();
        // tick must defer immediately; finishing may wait for the other writer.
        assert_eq!(session.pending_ms, 1234);
        assert!(!session.pending.is_empty());
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            lock.unlock().unwrap();
        });
        let result = session.finish();
        release.join().unwrap();
        result.unwrap();
        let record = load(root.path(), "game", "creator").unwrap();
        assert_eq!(record.played_ms, 1234);
        assert!(record.unlocked.contains_key("started"));
        // Retrying a completed flush must not count the same session twice.
        session.finish().unwrap();
        assert_eq!(
            load(root.path(), "game", "creator").unwrap().played_ms,
            1234
        );
    }
    #[cfg(unix)]
    #[test]
    fn progress_symlinks_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("progress")).unwrap();
        assert!(load(root.path(), "game", "creator").is_err());
    }
}
