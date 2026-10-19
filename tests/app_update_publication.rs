//! Uses the actual platform publication primitive; also compiles as a standalone
//! Windows test harness for API checks without pulling in the app or a GUI.
#[path = "../src/app_updates/publication.rs"]
mod publication;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        for _ in 0..1000 {
            let path = std::env::temp_dir().join(format!(
                "gamepkg-exclusive-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create private test directory: {error}"),
            }
        }
        panic!("private test directory names exhausted")
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn stage(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir(&path).unwrap();
    fs::write(path.join("payload"), b"new version").unwrap();
    path
}
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[test]
fn existing_files_and_empty_or_nonempty_directories_are_preserved() {
    let root = Root::new();
    let source = stage(root.path(), "staging");
    let target = root.path().join("target");
    fs::write(&target, b"old file").unwrap();
    assert!(publication::publish(&source, &target).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"old file");
    fs::remove_file(&target).unwrap();
    fs::create_dir(&target).unwrap();
    assert!(publication::publish(&source, &target).is_err());
    assert!(fs::read_dir(&target).unwrap().next().is_none());
    fs::write(target.join("old"), b"old version").unwrap();
    assert!(publication::publish(&source, &target).is_err());
    assert_eq!(fs::read(target.join("old")).unwrap(), b"old version");
    assert_eq!(fs::read(source.join("payload")).unwrap(), b"new version");
}
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[test]
fn unicode_publication_succeeds_and_invalid_paths_leave_source_intact() {
    let root = Root::new();
    let source = stage(root.path(), "staging-ゲーム");
    let target = root.path().join("new-é-🕹");
    publication::publish(&source, &target).unwrap();
    assert!(!source.exists());
    assert_eq!(fs::read(target.join("payload")).unwrap(), b"new version");
    let source = stage(root.path(), "another");
    assert!(publication::publish(&source, &root.path().join("bad\0name")).is_err());
    assert!(source.join("payload").is_file());
    assert!(!root.path().join("bad").exists());
}
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[test]
fn destination_creation_race_has_exactly_one_winner() {
    for _ in 0..32 {
        let root = Root::new();
        let source = stage(root.path(), "staging");
        let target = root.path().join("target");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let signal = barrier.clone();
        let created = target.clone();
        let creator = std::thread::spawn(move || {
            signal.wait();
            fs::create_dir(created).is_ok()
        });
        let signal = barrier.clone();
        let moved = target.clone();
        let staging = source.clone();
        let publisher = std::thread::spawn(move || {
            signal.wait();
            publication::publish(&staging, &moved).is_ok()
        });
        barrier.wait();
        let created = creator.join().unwrap();
        let published = publisher.join().unwrap();
        assert_ne!(
            created, published,
            "existing empty destination was replaced or both operations failed"
        );
        if created {
            assert!(fs::read_dir(&target).unwrap().next().is_none());
            assert!(source.join("payload").is_file());
        } else {
            assert!(!source.exists());
            assert_eq!(fs::read(target.join("payload")).unwrap(), b"new version");
        }
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn existing_symlink_is_preserved_even_when_its_target_is_missing() {
    use std::os::unix::fs::symlink;
    let root = Root::new();
    let source = stage(root.path(), "staging");
    let missing = root.path().join("missing");
    let target = root.path().join("target");
    symlink(&missing, &target).unwrap();
    assert!(publication::publish(&source, &target).is_err());
    assert_eq!(fs::read_link(&target).unwrap(), missing);
    assert!(source.join("payload").is_file());
}
#[cfg(windows)]
#[test]
fn windows_long_paths_keep_no_replace_behavior() {
    let root = Root::new();
    let mut parent = root.path().to_path_buf();
    for _ in 0..6 {
        parent.push("x".repeat(60));
        fs::create_dir(&parent).unwrap();
    }
    let source = stage(&parent, "staging");
    let target = parent.join("target");
    fs::create_dir(&target).unwrap();
    assert!(publication::publish(&source, &target).is_err());
    fs::remove_dir(&target).unwrap();
    publication::publish(&source, &target).unwrap();
    assert_eq!(fs::read(target.join("payload")).unwrap(), b"new version");
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
#[test]
fn unsupported_platform_does_not_attempt_publication() {
    let root = Root::new();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir(&source).unwrap();
    assert_eq!(
        publication::publish(&source, &target).unwrap_err().kind(),
        std::io::ErrorKind::Unsupported
    );
    assert!(source.is_dir() && !target.exists());
}
