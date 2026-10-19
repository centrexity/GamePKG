//! One validated archive, pinned to a host-only directory, per signed identity.
use anyhow::{ensure, Result};
use std::{
    fs,
    io::Write,
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
};

pub(super) struct Store {
    _directory: fs::File,
    directory: PathBuf,
    name: std::ffi::OsString,
    pub seed: Vec<u8>,
}
impl Store {
    pub fn open(root: &Path, id: &str, creator: &str, schema: u32) -> Result<Self> {
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
        let seed =
            match crate::files::read_regular(&path, super::protocol::MAX_SAVE_TREE as u64, false) {
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
                    match crate::files::read_regular(&legacy, crate::saves::MAX_SAVE as u64, false)
                    {
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
            directory,
            name,
            seed,
        })
    }
    pub fn commit(&mut self, bytes: &[u8]) -> Result<()> {
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
            Store::open(root.path(), "one", "creator", 1).unwrap().seed,
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
