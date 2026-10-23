//! One validated archive, pinned to a host-only directory, per signed identity.
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

fn read_archive(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = crate::files::open_regular(path, limit, false)?;
    ensure!(
        file.metadata()?.nlink() == 1,
        "Saved archives must not be hard-linked to another file"
    );
    let mut bytes = Vec::new();
    file.take(limit.checked_add(1).context("Invalid save size limit")?)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "Saved archive grew beyond its quota"
    );
    Ok(bytes)
}

pub(super) struct Store {
    _directory: fs::File,
    _writer: Option<fs::File>,
    directory: PathBuf,
    name: std::ffi::OsString,
    pub seed: Vec<u8>,
}
impl Store {
    pub fn open(root: &Path, id: &str, creator: &str, schema: u32) -> Result<Self> {
        Self::open_mode(root, id, creator, schema, true)
    }
    pub fn read_snapshot(root: &Path, id: &str, creator: &str, schema: u32) -> Result<Vec<u8>> {
        Ok(Self::open_mode(root, id, creator, schema, false)?.seed)
    }
    fn open_mode(root: &Path, id: &str, creator: &str, schema: u32, writer: bool) -> Result<Self> {
        ensure!(schema > 0, "save schema must be positive");
        let root_fd = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)?;
        let result = unsafe { libc::mkdirat(root_fd.as_raw_fd(), c"saves".as_ptr(), 0o700) };
        if result != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
            return Err(std::io::Error::last_os_error().into());
        }
        let fd = unsafe {
            libc::openat(
                root_fd.as_raw_fd(),
                c"saves".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        ensure!(fd >= 0, "Cube 2 save directory must be a real directory");
        use std::os::fd::FromRawFd;
        let directory_fd = unsafe { fs::File::from_raw_fd(fd) };
        let directory = PathBuf::from(format!("/proc/self/fd/{fd}"));
        let name = super::save_path(root, id, creator, schema)
            .file_name()
            .unwrap()
            .to_os_string();
        let path = directory.join(&name);
        let lease = if writer {
            let lock = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(directory.join(format!("{}.writer", name.to_string_lossy())))?;
            let metadata = lock.metadata()?;
            ensure!(
                metadata.is_file() && metadata.nlink() == 1 && metadata.len() == 0,
                "Invalid game save writer lock"
            );
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(std::io::Error::last_os_error()).context("This game's save is in use. Close the running game before modifying it or opening another session");
            }
            Some(lock)
        } else {
            None
        };
        let seed = match read_archive(&path, super::protocol::MAX_SAVE_TREE as u64) {
            Ok(bytes) => {
                super::protocol::save_files(&bytes)?;
                bytes
            }
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                // Migrate the previous three-slot format using the same pinned
                // directory. Never resolve an untrusted parent a second time.
                let scope = if schema == 1 {
                    format!("GamePKG save v1\0{id}\0{creator}")
                } else {
                    format!("GamePKG save v1\0{id}\0{creator}\0schema={schema}")
                };
                let legacy =
                    directory.join(format!("{}.blob", crate::verify::digest(scope.as_bytes())));
                match read_archive(&legacy, crate::saves::MAX_SAVE as u64) {
                    Ok(bytes) => {
                        super::protocol::save_seed(&bytes)?;
                        bytes
                    }
                    Err(error)
                        if error
                            .downcast_ref::<std::io::Error>()
                            .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                    {
                        Vec::new()
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        };
        Ok(Self {
            _directory: directory_fd,
            _writer: lease,
            directory,
            name,
            seed,
        })
    }
    pub fn commit(&mut self, bytes: &[u8]) -> Result<()> {
        ensure!(
            self._writer.is_some(),
            "Save commit requires a writer lease"
        );
        super::protocol::save_files(bytes)?;
        let mut temp = tempfile::NamedTempFile::new_in(&self.directory)?;
        temp.write_all(bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(self.directory.join(&self.name))
            .map_err(|error| error.error)?;
        self._directory.sync_all()?;
        self.seed = bytes.to_vec();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube2::protocol::test_tree;
    #[test]
    fn simultaneous_writers_cannot_replace_each_others_game_state() {
        let root = tempfile::tempdir().unwrap();
        let mut first = Store::open(root.path(), "one", "creator", 1).unwrap();
        let original = test_tree(&[("config.cfg", b"original")]);
        first.commit(&original).unwrap();
        assert!(
            Store::open(root.path(), "one", "creator", 1).is_err(),
            "A second writer can erase the active game's latest save"
        );
        assert_eq!(
            Store::read_snapshot(root.path(), "one", "creator", 1).unwrap(),
            original
        );
        assert_eq!(
            Store::read_snapshot(root.path(), "one", "creator", 1).unwrap(),
            original
        );
        let probe = |blocked: bool| {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "cube2::save::tests::writer_lease_child_probe",
                    "--ignored",
                    "--nocapture",
                ])
                .env("GAMEPKG_SAVE_LEASE_PROBE_ROOT", root.path())
                .env(
                    "GAMEPKG_SAVE_LEASE_PROBE_BLOCKED",
                    if blocked { "1" } else { "0" },
                )
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        probe(true);
        drop(first);
        probe(false);
        assert_eq!(
            Store::open(root.path(), "one", "creator", 1).unwrap().seed,
            original
        );
    }
    #[test]
    #[ignore = "subprocess helper for the writer lease regression"]
    fn writer_lease_child_probe() {
        let root = std::env::var_os("GAMEPKG_SAVE_LEASE_PROBE_ROOT").expect("private probe root");
        let blocked = std::env::var("GAMEPKG_SAVE_LEASE_PROBE_BLOCKED").unwrap() == "1";
        assert_eq!(
            Store::open(Path::new(&root), "one", "creator", 1).is_err(),
            blocked
        );
        assert!(!Store::read_snapshot(Path::new(&root), "one", "creator", 1)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn writer_lease_rejects_links_and_wrong_file_types_without_changing_targets() {
        for attack in ["symlink", "hardlink", "fifo", "directory", "nonempty"] {
            let root = tempfile::tempdir().unwrap();
            fs::create_dir(root.path().join("saves")).unwrap();
            let save = super::super::save_path(root.path(), "one", "creator", 1);
            let lock = save.with_file_name(format!(
                "{}.writer",
                save.file_name().unwrap().to_string_lossy()
            ));
            let private = root.path().join("private");
            fs::write(&private, b"").unwrap();
            match attack {
                "symlink" => std::os::unix::fs::symlink(&private, &lock).unwrap(),
                "hardlink" => fs::hard_link(&private, &lock).unwrap(),
                "fifo" => {
                    let path = std::ffi::CString::new(std::os::unix::ffi::OsStrExt::as_bytes(
                        lock.as_os_str(),
                    ))
                    .unwrap();
                    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
                }
                "directory" => fs::create_dir(&lock).unwrap(),
                _ => fs::write(&lock, b"preserve lock contents").unwrap(),
            }
            assert!(
                Store::open(root.path(), "one", "creator", 1).is_err(),
                "{attack}"
            );
            assert_eq!(fs::read(&private).unwrap(), b"");
            if attack == "nonempty" {
                assert_eq!(fs::read(&lock).unwrap(), b"preserve lock contents");
            }
        }
    }
    #[test]
    fn scoped_archive_migrates_legacy_preferences_and_preserves_previous_good_save() {
        let root = tempfile::tempdir().unwrap();
        let mut legacy = crate::saves::Save::open(root.path(), "one", "creator", 1).unwrap();
        legacy.bytes = b"GPS1\0\0\0\0\0\0\0\0\0\0\0\0".to_vec();
        legacy.dirty = true;
        legacy.commit().unwrap();
        let mut store = Store::open(root.path(), "one", "creator", 1).unwrap();
        assert_eq!(store.seed, legacy.bytes);
        let tree = test_tree(&[
            ("config.cfg", b"name test"),
            ("packages/base/map.ogz", b"map"),
        ]);
        store.commit(&tree).unwrap();
        assert!(store
            .commit(&test_tree(&[("../outside.cfg", b"bad")]))
            .is_err());
        assert_eq!(
            Store::read_snapshot(root.path(), "one", "creator", 1).unwrap(),
            tree
        );
        for (id, key, schema) in [
            ("two", "creator", 1),
            ("one", "other", 1),
            ("one", "creator", 2),
        ] {
            assert!(Store::open(root.path(), id, key, schema)
                .unwrap()
                .seed
                .is_empty());
        }
        assert_eq!(
            crate::saves::Save::open(root.path(), "one", "creator", 1)
                .unwrap()
                .bytes,
            legacy.bytes
        );
    }
    #[test]
    fn library_storage_counts_the_scoped_archive_without_counting_other_creators() {
        let root = tempfile::tempdir().unwrap();
        let mut manifest =
            crate::manifest::Manifest::parse(include_bytes!("../../ports/cube2/manifest.toml"))
                .unwrap();
        manifest.creator_key = hex::encode([4; 32]);
        fs::write(
            root.path().join(format!("{}.gamepkg", manifest.id)),
            b"package",
        )
        .unwrap();
        let tree = test_tree(&[("packages/base/map.ogz", b"edited map")]);
        Store::open(root.path(), &manifest.id, &manifest.creator_key, 1)
            .unwrap()
            .commit(&tree)
            .unwrap();
        Store::open(root.path(), &manifest.id, "other", 1)
            .unwrap()
            .commit(&tree)
            .unwrap();
        assert_eq!(
            crate::storage::usage(root.path(), &manifest)
                .unwrap()
                .save_bytes,
            tree.len() as u64
        );
    }
    #[test]
    fn hardlinked_current_and_legacy_archives_cannot_cross_save_scopes() {
        for legacy in [false, true] {
            let root = tempfile::tempdir().unwrap();
            fs::create_dir(root.path().join("saves")).unwrap();
            let private = root.path().join("another-scope");
            let bytes = if legacy {
                b"GPS1\0\0\0\0\0\0\0\0\0\0\0\0".to_vec()
            } else {
                test_tree(&[("Private.AVI", b"private recording")])
            };
            fs::write(&private, &bytes).unwrap();
            let target = if legacy {
                root.path().join("saves").join(format!(
                    "{}.blob",
                    crate::verify::digest(b"GamePKG save v1\0one\0creator")
                ))
            } else {
                super::super::save_path(root.path(), "one", "creator", 1)
            };
            fs::hard_link(&private, &target).unwrap();
            assert!(
                Store::read_snapshot(root.path(), "one", "creator", 1).is_err(),
                "Hardlinked {legacy} archive was readable under another scope"
            );
            assert!(Store::open(root.path(), "one", "creator", 1).is_err());
            assert_eq!(fs::read(&private).unwrap(), bytes);
        }
    }
    #[test]
    fn directory_replacement_and_archive_links_cannot_redirect_reads_or_commits() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let tree = test_tree(&[("config.cfg", b"test")]);
        let mut store = Store::open(root.path(), "one", "creator", 1).unwrap();
        fs::rename(root.path().join("saves"), root.path().join("pinned")).unwrap();
        symlink(outside.path(), root.path().join("saves")).unwrap();
        store.commit(&tree).unwrap();
        assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
        assert!(Store::open(root.path(), "one", "creator", 1).is_err());
        assert_eq!(
            fs::read(root.path().join("pinned").join(&store.name)).unwrap(),
            tree
        );
        fs::remove_file(root.path().join("saves")).unwrap();
        fs::create_dir(root.path().join("saves")).unwrap();
        let private = outside.path().join("private");
        fs::write(&private, &tree).unwrap();
        symlink(
            &private,
            super::super::save_path(root.path(), "one", "creator", 1),
        )
        .unwrap();
        assert!(Store::open(root.path(), "one", "creator", 1).is_err());
        assert_eq!(fs::read(private).unwrap(), tree);
    }
}
