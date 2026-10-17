//! Explicit player-selected files. Guests receive bytes and portable basenames, never paths.
use crate::manifest::UserFile;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
pub const MAX_IMPORT: usize = 64 * 1048576;
pub const MAX_EXPORT: usize = 1048576;
pub const MAX_REQUEST: usize = MAX_EXPORT + 264;
pub const MAX_CHUNK: usize = 65536;
static SESSION: AtomicU64 = AtomicU64::new(1);
pub fn validate(policies: &[UserFile]) -> anyhow::Result<()> {
    anyhow::ensure!(policies.len() <= 32, "too many user-file permissions");
    let mut ids = std::collections::HashSet::new();
    for policy in policies {
        anyhow::ensure!(
            !policy.id.is_empty()
                && policy.id.len() <= 64
                && policy
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                && ids.insert(&policy.id),
            "invalid or duplicate user-file identifier"
        );
        anyhow::ensure!(
            !policy.title.trim().is_empty()
                && policy.title.len() <= 128
                && !policy.title.chars().any(char::is_control),
            "invalid user-file title"
        );
        anyhow::ensure!(
            policy.read || policy.write,
            "user-file permission needs an operation"
        );
        anyhow::ensure!(
            policy.max_bytes > 0 && policy.max_bytes <= MAX_IMPORT as u64,
            "invalid user-file size limit"
        );
        anyhow::ensure!(
            !policy.write || policy.max_bytes <= MAX_EXPORT as u64,
            "export permission exceeds the export size limit"
        );
        anyhow::ensure!(
            !policy.extensions.is_empty() && policy.extensions.len() <= 8,
            "user-file extensions required"
        );
        let mut seen = std::collections::HashSet::new();
        for extension in &policy.extensions {
            anyhow::ensure!(
                !extension.is_empty()
                    && extension.len() <= 16
                    && extension
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                    && seen.insert(extension),
                "invalid or duplicate user-file extension"
            );
        }
    }
    Ok(())
}
pub fn portable_name(name: &str) -> bool {
    crate::archive::valid_path(name) && !name.contains('/')
}
fn accepted_name(policy: &UserFile, name: &str) -> bool {
    portable_name(name)
        && name.rsplit_once('.').is_some_and(|(_, ext)| {
            policy
                .extensions
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(ext))
        })
}
#[derive(Clone)]
pub enum Kind {
    Import,
    Export { name: String, bytes: Vec<u8> },
}
#[derive(Clone)]
pub struct Job {
    pub session: u64,
    pub handle: u32,
    pub policy: UserFile,
    pub kind: Kind,
    alive: Arc<AtomicBool>,
}
impl Job {
    pub fn active(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}
pub enum Outcome {
    Imported { name: String, bytes: Vec<u8> },
    Exported,
    Cancelled,
    Failed,
}
struct Entry {
    job: Job,
    staged: bool,
    dispatched: bool,
    outcome: Option<Outcome>,
}
pub struct Service {
    policies: Vec<UserFile>,
    enabled: bool,
    gesture: bool,
    next: u32,
    session: u64,
    alive: Arc<AtomicBool>,
    entry: Option<Entry>,
}
impl Drop for Service {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}
impl Service {
    pub fn new(policies: Vec<UserFile>) -> Self {
        let policies = if validate(&policies).is_ok() {
            policies
        } else {
            Vec::new()
        };
        Self {
            policies,
            enabled: false,
            gesture: false,
            next: 1,
            session: SESSION.fetch_add(1, Ordering::Relaxed),
            alive: Arc::new(AtomicBool::new(true)),
            entry: None,
        }
    }
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    pub fn begin(&mut self, gesture: bool) {
        self.gesture = gesture;
    }
    pub fn finish(&mut self, success: bool) {
        if !success
            && self
                .entry
                .as_ref()
                .is_some_and(|e| !e.dispatched && e.outcome.is_none())
        {
            self.entry = None;
        } else if success {
            if let Some(entry) = &mut self.entry {
                entry.staged = false;
            }
        }
        self.gesture = false;
    }
    pub fn take_job(&mut self) -> Option<Job> {
        let e = self.entry.as_mut()?;
        if e.staged || e.dispatched || e.outcome.is_some() {
            return None;
        }
        e.dispatched = true;
        Some(e.job.clone())
    }
    pub fn complete(&mut self, job: &Job, outcome: Outcome) -> bool {
        let Some(e) = &mut self.entry else {
            return false;
        };
        if job.session != self.session
            || job.handle != e.job.handle
            || !job.active()
            || !e.dispatched
            || e.outcome.is_some()
        {
            return false;
        }
        let valid = match (&e.job.kind, &outcome) {
            (Kind::Import, Outcome::Imported { name, bytes }) => {
                accepted_name(&e.job.policy, name)
                    && bytes.len() as u64 <= e.job.policy.max_bytes
                    && bytes.len() <= MAX_IMPORT
            }
            (Kind::Export { .. }, Outcome::Exported) => true,
            (_, Outcome::Cancelled | Outcome::Failed) => true,
            _ => false,
        };
        e.outcome = Some(if valid { outcome } else { Outcome::Failed });
        true
    }
    pub fn call(&mut self, operation: u32, request: &[u8], capacity: usize) -> Option<Vec<u8>> {
        if request.len() > MAX_REQUEST || capacity > MAX_CHUNK {
            return None;
        }
        let number = |at: usize| -> Option<u32> {
            Some(u32::from_le_bytes(
                request.get(at..at + 4)?.try_into().ok()?,
            ))
        };
        if operation <= 1 {
            if !self.enabled || !self.gesture || self.entry.is_some() || capacity < 4 {
                return None;
            }
            let index = number(0)? as usize;
            let policy = self.policies.get(index)?.clone();
            let kind = if operation == 0 {
                if request.len() != 4 || !policy.read {
                    return None;
                }
                Kind::Import
            } else {
                if !policy.write || request.len() < 8 {
                    return None;
                }
                let length = number(4)? as usize;
                if length > 256 {
                    return None;
                }
                let name = std::str::from_utf8(request.get(8..8 + length)?).ok()?;
                let bytes = request.get(8 + length..)?;
                if !accepted_name(&policy, name)
                    || bytes.len() > MAX_EXPORT
                    || bytes.len() as u64 > policy.max_bytes
                {
                    return None;
                }
                Kind::Export {
                    name: name.to_owned(),
                    bytes: bytes.to_vec(),
                }
            };
            let handle = self.next;
            self.next = self.next.checked_add(1)?;
            self.gesture = false;
            self.entry = Some(Entry {
                job: Job {
                    session: self.session,
                    handle,
                    policy,
                    kind,
                    alive: self.alive.clone(),
                },
                staged: true,
                dispatched: false,
                outcome: None,
            });
            return Some(handle.to_le_bytes().to_vec());
        }
        let handle = number(0)?;
        let e = self.entry.as_ref()?;
        if e.job.handle != handle {
            return None;
        }
        match operation {
            2 if request.len() == 4 && capacity >= 12 => {
                let (status, size, length) = match &e.outcome {
                    None => (1, 0, 0),
                    Some(Outcome::Imported { name, bytes }) => {
                        (2, bytes.len() as u32, name.len() as u32)
                    }
                    Some(Outcome::Exported) => (3, 0, 0),
                    Some(Outcome::Cancelled) => (4, 0, 0),
                    Some(Outcome::Failed) => (5, 0, 0),
                };
                Some(
                    [status, size, length]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect(),
                )
            }
            3 if request.len() == 4 => {
                let Some(Outcome::Imported { name, .. }) = &e.outcome else {
                    return None;
                };
                if capacity < name.len() {
                    return None;
                }
                Some(name.as_bytes().to_vec())
            }
            4 if request.len() == 8 => {
                let Some(Outcome::Imported { bytes, .. }) = &e.outcome else {
                    return None;
                };
                let offset = number(4)? as usize;
                let bytes = bytes.get(offset..)?;
                Some(bytes[..bytes.len().min(capacity)].to_vec())
            }
            5 if request.len() == 4 && capacity == 0 && e.outcome.is_some() => {
                self.entry = None;
                Some(Vec::new())
            }
            _ => None,
        }
    }
}
pub fn gesture(events: &[crate::input_events::Event]) -> bool {
    events.iter().any(|e| {
        e.kind == 2
            || (e.kind == 5 && e.code & 65536 == 0)
            || (e.kind == 9 && e.code & 0x80000000 == 0)
    })
}
/// Only the trusted UI supplies `path`; it is never decoded from guest bytes.
pub fn execute(job: &Job, path: Option<&std::path::Path>) -> Outcome {
    if !job.active() {
        return Outcome::Cancelled;
    }
    let Some(path) = path else {
        return Outcome::Cancelled;
    };
    let result = (|| -> anyhow::Result<Outcome> {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid selected filename"))?;
        anyhow::ensure!(
            accepted_name(&job.policy, name),
            "selected file type is not permitted"
        );
        let metadata = path.symlink_metadata();
        if let Ok(metadata) = &metadata {
            anyhow::ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "selected path must be a regular file"
            );
        }
        match &job.kind {
            Kind::Import => {
                use std::io::Read;
                let mut options = std::fs::OpenOptions::new();
                options.read(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::OpenOptionsExt;
                    options.custom_flags(0x00200000);
                }
                let file = options.open(path)?;
                let info = file.metadata()?;
                anyhow::ensure!(
                    info.is_file()
                        && !info.file_type().is_symlink()
                        && info.len() <= job.policy.max_bytes
                        && info.len() <= MAX_IMPORT as u64,
                    "selected file is too large or not regular"
                );
                let mut bytes = Vec::new();
                file.take(job.policy.max_bytes + 1)
                    .read_to_end(&mut bytes)?;
                anyhow::ensure!(
                    bytes.len() as u64 <= job.policy.max_bytes,
                    "selected file grew beyond its limit"
                );
                Ok(Outcome::Imported {
                    name: name.to_owned(),
                    bytes,
                })
            }
            Kind::Export { bytes, .. } => {
                anyhow::ensure!(
                    bytes.len() <= MAX_EXPORT && bytes.len() as u64 <= job.policy.max_bytes,
                    "export exceeds its limit"
                );
                let parent = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(std::path::Path::new("."));
                let mut file = tempfile::NamedTempFile::new_in(parent)?;
                use std::io::Write;
                file.write_all(bytes)?;
                file.as_file().sync_all()?;
                anyhow::ensure!(job.active(), "game closed during file selection");
                file.persist(path).map_err(|error| error.error)?;
                Ok(Outcome::Exported)
            }
        }
    })();
    result.unwrap_or(Outcome::Failed)
}
