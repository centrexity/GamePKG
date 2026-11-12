//! Read-only audit of literal startup-script includes, without executing CubeScript.
//! This does not establish matching media versions or evaluate dynamic commands.
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use std::{
    collections::{BTreeSet, VecDeque},
    fs::File,
    os::fd::{AsRawFd, FromRawFd},
    path::Path,
};

const STARTUP: &[&str] = &[
    "data/stdlib.cfg",
    "data/font.cfg",
    "data/keymap.cfg",
    "data/stdedit.cfg",
    "data/sounds.cfg",
    "data/menus.cfg",
    "data/brushes_heightmap.cfg",
    "data/brushes_blendmap.cfg",
    "data/defaults.cfg",
    "data/game_fps.cfg",
];

#[derive(Debug, Serialize)]
pub struct Issue {
    pub script: String,
    pub reference: String,
    pub reason: String,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub checked_scripts: usize,
    pub missing_or_unreadable: Vec<Issue>,
    pub unresolved: Vec<Issue>,
    pub upstream_missing_references: Vec<Issue>,
    pub matching_asset_version_verified: bool,
}
impl Report {
    pub fn summary(&self) -> String {
        let mut lines = vec![format!(
            "Checked {} scripts for literal includes. {} missing or unreadable; {} unresolved.",
            self.checked_scripts,
            self.missing_or_unreadable.len(),
            self.unresolved.len()
        )];
        for issue in &self.missing_or_unreadable {
            lines.push(format!(
                "{} — referenced by {}: {}",
                issue.reference, issue.script, issue.reason
            ));
        }
        for issue in &self.upstream_missing_references {
            lines.push(format!("Known upstream stale include: {} ({}). The original texture browser is built into the engine.", issue.reference,issue.script));
        }
        lines.push("Dynamic commands, conditional execution, complete media and matching asset versions still require verification.".into());
        lines.join("\n")
    }
}

fn path_parts(path: &str) -> Result<Vec<&str>> {
    let parts: Vec<_> = path.split('/').collect();
    ensure!(
        parts.len() >= 2
            && parts.len() <= 6
            && matches!(parts[0], "data" | "packages")
            && path.len() <= 1024
            && path.ends_with(".cfg")
            && parts.iter().all(|part| !part.is_empty()
                && *part != "."
                && *part != ".."
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_- .".contains(&c))),
        "reference is not a literal scoped script path"
    );
    Ok(parts)
}

fn read_script(directories: &super::data::Directories, path: &str) -> Result<Vec<u8>> {
    let parts = path_parts(path)?;
    let mut parent = if parts[0] == "data" {
        directories.data.try_clone()?
    } else {
        directories.packages.try_clone()?
    };
    for part in &parts[1..parts.len() - 1] {
        let name = std::ffi::CString::new(*part)?;
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        ensure!(
            fd >= 0,
            "script directory is missing or not a real directory"
        );
        parent = unsafe { File::from_raw_fd(fd) };
    }
    let path = std::path::PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd()))
        .join(parts.last().unwrap());
    let bytes = crate::files::read_regular(&path, 1048576, false)?;
    ensure!(!bytes.is_empty(), "script is empty");
    Ok(bytes)
}

fn reference(line: &str) -> Option<Result<String>> {
    let line = line.trim();
    let rest = line.strip_prefix("exec")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start();
    let (name, tail) = if let Some(quoted) = rest.strip_prefix('"') {
        match quoted.find('"') {
            Some(end) => (&quoted[..end], &quoted[end + 1..]),
            None => return Some(Err(anyhow::anyhow!("unterminated or computed include"))),
        }
    } else {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        (&rest[..end], &rest[end..])
    };
    Some((|| {
        ensure!(
            tail.trim().is_empty() || tail.trim_start().starts_with("//"),
            "include has additional expressions"
        );
        path_parts(name)?;
        Ok(name.to_owned())
    })())
}

// Verified with svn cat/list -r 6889 against the official repository.
// Reporting provenance only: file reads retain all normal scope/link checks.
const UPSTREAM_MENUS_SHA256: &str =
    "24f0d85f1d77ea075d40677141f07d4104d606df128db4c20d8313b6f4730f46";
fn known_upstream_reference(parent: &str, line: usize, target: &str, digest: &str) -> bool {
    parent == "data/menus.cfg"
        && line == 28
        && target == "data/menus/browse_textures2.cfg"
        && digest == UPSTREAM_MENUS_SHA256
}

pub fn audit(path: &Path) -> Result<Report> {
    let directories = super::data::Directories::open(path)?;
    let mut report = Report {
        checked_scripts: 0,
        missing_or_unreadable: Vec::new(),
        unresolved: Vec::new(),
        upstream_missing_references: Vec::new(),
        matching_asset_version_verified: false,
    };
    let mut pending: VecDeque<_> = STARTUP
        .iter()
        .map(|name| ("engine startup".to_owned(), (*name).to_owned(), false))
        .collect();
    let mut visited = BTreeSet::new();
    let mut total = 0usize;
    while let Some((parent, path, upstream)) = pending.pop_front() {
        if !visited.insert(path.clone()) {
            continue;
        }
        ensure!(
            visited.len() <= 256,
            "startup include audit exceeds 256 scripts"
        );
        let bytes = match read_script(&directories, &path) {
            Ok(bytes) => bytes,
            Err(error) => {
                let known = upstream
                    && error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound);
                let issues = if known {
                    &mut report.upstream_missing_references
                } else {
                    &mut report.missing_or_unreadable
                };
                issues.push(Issue {
                    script: parent,
                    reference: path,
                    reason: error.to_string(),
                });
                continue;
            }
        };
        total = total
            .checked_add(bytes.len())
            .context("script byte count overflow")?;
        ensure!(
            total <= 32 * 1048576,
            "startup include audit exceeds 32 MiB"
        );
        report.checked_scripts += 1;
        let text = match std::str::from_utf8(&bytes) {
            Ok(text) => text,
            Err(_) => {
                report.unresolved.push(Issue {
                    script: path,
                    reference: String::new(),
                    reason: "script is not UTF-8; includes were not evaluated".into(),
                });
                continue;
            }
        };
        let digest = crate::verify::digest(&bytes);
        for (number, line) in text.lines().enumerate() {
            match reference(line) {
                Some(Ok(name)) => {
                    if !visited.contains(&name)
                        && !pending.iter().any(|(_, queued, _)| queued == &name)
                    {
                        ensure!(
                            pending.len() + visited.len() < 256,
                            "startup include audit exceeds 256 scripts"
                        );
                        let upstream = known_upstream_reference(&path, number + 1, &name, &digest);
                        pending.push_back((format!("{path}:{}", number + 1), name, upstream));
                    }
                }
                Some(Err(error)) => {
                    ensure!(
                        report.unresolved.len() < 256,
                        "startup include audit exceeds 256 unresolved references"
                    );
                    report.unresolved.push(Issue {
                        script: format!("{path}:{}", number + 1),
                        reference: line.trim().chars().take(160).collect(),
                        reason: error.to_string(),
                    });
                }
                None => {}
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_classification_requires_the_exact_verified_parent_and_reference() {
        assert!(known_upstream_reference(
            "data/menus.cfg",
            28,
            "data/menus/browse_textures2.cfg",
            UPSTREAM_MENUS_SHA256
        ));
        for (parent, line, target, digest) in [
            (
                "data/custom.cfg",
                28,
                "data/menus/browse_textures2.cfg",
                UPSTREAM_MENUS_SHA256,
            ),
            (
                "data/menus.cfg",
                29,
                "data/menus/browse_textures2.cfg",
                UPSTREAM_MENUS_SHA256,
            ),
            (
                "data/menus.cfg",
                28,
                "data/menus/missing.cfg",
                UPSTREAM_MENUS_SHA256,
            ),
            (
                "data/menus.cfg",
                28,
                "data/menus/browse_textures2.cfg",
                "modified",
            ),
        ] {
            assert!(!known_upstream_reference(parent, line, target, digest));
        }
    }
    #[test]
    fn include_queue_and_unresolved_reports_are_bounded() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        std::fs::create_dir_all(root.path().join("packages/textures")).unwrap();
        std::fs::write(
            root.path().join("packages/textures/notexture.png"),
            b"texture",
        )
        .unwrap();
        for name in STARTUP {
            std::fs::write(root.path().join(name), b"// original\n").unwrap();
        }
        let menus = root.path().join("data/menus.cfg");
        std::fs::write(&menus, "exec data/stdlib.cfg\n".repeat(10000)).unwrap();
        assert!(audit(root.path()).unwrap().missing_or_unreadable.is_empty());
        let unique = (0..257)
            .map(|n| format!("exec data/extra{n}.cfg\n"))
            .collect::<String>();
        std::fs::write(&menus, unique).unwrap();
        assert!(audit(root.path())
            .unwrap_err()
            .to_string()
            .contains("256 scripts"));
        std::fs::write(&menus, "exec $computed\n".repeat(257)).unwrap();
        assert!(audit(root.path())
            .unwrap_err()
            .to_string()
            .contains("256 unresolved"));
    }
    #[test]
    fn include_syntax_never_evaluates_commands_or_escapes_scope() {
        for line in [
            "exec data/a.cfg",
            " exec \"packages/fonts/default.cfg\" // font",
        ] {
            assert!(reference(line).unwrap().is_ok());
        }
        for line in [
            "exec ../private.cfg",
            "exec /tmp/private.cfg",
            "exec $script",
            "exec (concat data/a .cfg)",
            "exec data/a.cfg; quit",
            "exec \"data/a.cfg\"; quit",
            "exec data//a.cfg",
            "exec data/../a.cfg",
            "exec \"data/a^\".cfg\"",
        ] {
            assert!(reference(line).unwrap().is_err(), "{line}");
        }
        for line in [
            "// exec data/missing.cfg",
            "execute data/a.cfg",
            "myexec data/a.cfg",
        ] {
            assert!(reference(line).is_none());
        }
    }
    #[test]
    fn recursive_audit_is_read_only_cycle_bounded_and_rejects_links() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        std::fs::create_dir_all(root.path().join("packages/textures")).unwrap();
        std::fs::write(
            root.path().join("packages/textures/notexture.png"),
            b"texture",
        )
        .unwrap();
        for name in STARTUP {
            std::fs::write(root.path().join(name), b"// original\n").unwrap();
        }
        std::fs::write(
            root.path().join("data/menus.cfg"),
            b"exec data/cycle.cfg\nexec data/missing.cfg\nexec $computed\nexec data/linked.cfg\n",
        )
        .unwrap();
        std::fs::write(root.path().join("data/cycle.cfg"), b"exec data/menus.cfg\n").unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), b"exec data/leaked.cfg\n").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("data/linked.cfg")).unwrap();
        let report = audit(root.path()).unwrap();
        assert_eq!(report.checked_scripts, STARTUP.len() + 1);
        assert_eq!(report.missing_or_unreadable.len(), 2);
        assert_eq!(report.unresolved.len(), 1);
        assert!(!report.matching_asset_version_verified);
        assert!(!report
            .missing_or_unreadable
            .iter()
            .any(|issue| issue.reference.contains("leaked")));
        assert_eq!(
            std::fs::read(outside.path()).unwrap(),
            b"exec data/leaked.cfg\n"
        );
        let directories = super::super::data::Directories::open(root.path()).unwrap();
        std::os::unix::fs::symlink(
            root.path().join("data"),
            root.path().join("data/linked-dir"),
        )
        .unwrap();
        assert!(read_script(&directories, "data/linked-dir/stdlib.cfg").is_err());
        let fifo = std::ffi::CString::new(
            root.path()
                .join("data/fifo.cfg")
                .as_os_str()
                .as_encoded_bytes(),
        )
        .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(read_script(&directories, "data/fifo.cfg").is_err());
    }
}
