//! Pin selected directories before compilation; never re-resolve their paths at launch.
use anyhow::{ensure, Context, Result};
use std::{
    fs::{self, File, OpenOptions},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::OpenOptionsExt,
    },
    path::Path,
};
pub(super) struct Directories {
    pub data: File,
    pub packages: File,
}
impl Directories {
    pub fn open(path: &Path) -> Result<Self> {
        let path =
            fs::canonicalize(path).context("select the original Sauerbraten data installation")?;
        let root = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_PATH | libc::O_NOFOLLOW)
            .open(path)?;
        let open = |name: &std::ffi::CStr| -> Result<File> {
            let fd = unsafe {
                libc::openat(
                    root.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            ensure!(
                fd >= 0,
                "Cube 2 game data must contain real data and packages directories: {}",
                std::io::Error::last_os_error()
            );
            let file = unsafe { File::from_raw_fd(fd) };
            // Reserve descriptors 3 and 4 for the engine's two byte channels.
            let high = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
            ensure!(high >= 0, "cannot pin Cube 2 data descriptor");
            Ok(unsafe { File::from_raw_fd(high) })
        };
        let data = open(c"data")?;
        let packages = open(c"packages")?;
        crate::game_data::validate_parts(
            Path::new(&format!("/proc/self/fd/{}", data.as_raw_fd())),
            Path::new(&format!("/proc/self/fd/{}", packages.as_raw_fd())),
        )?;
        Ok(Self { data, packages })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    fn fixture(root: &Path) {
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(root.join("packages/textures")).unwrap();
        fs::write(root.join("data/stdlib.cfg"), b"original").unwrap();
        fs::write(root.join("packages/textures/notexture.png"), b"texture").unwrap();
    }
    #[test]
    fn replacement_after_selection_cannot_redirect_directory_handles() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        let selected = Directories::open(root.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("stdlib.cfg"), b"private").unwrap();
        fs::rename(root.path().join("data"), root.path().join("old-data")).unwrap();
        symlink(outside.path(), root.path().join("data")).unwrap();
        let pinned = format!("/proc/self/fd/{}/stdlib.cfg", selected.data.as_raw_fd());
        assert_eq!(fs::read(pinned).unwrap(), b"original");
        assert!(Directories::open(root.path()).is_err());
    }
    #[test]
    fn symlinked_packages_and_missing_core_data_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        fs::remove_file(root.path().join("data/stdlib.cfg")).unwrap();
        assert!(Directories::open(root.path()).is_err());
        fs::write(root.path().join("data/stdlib.cfg"), b"original").unwrap();
        fs::rename(
            root.path().join("packages"),
            root.path().join("old-packages"),
        )
        .unwrap();
        symlink(
            root.path().join("old-packages"),
            root.path().join("packages"),
        )
        .unwrap();
        assert!(Directories::open(root.path()).is_err());
    }
}
