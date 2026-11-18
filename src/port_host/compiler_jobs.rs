//! Private Linux compiler scheduling, never a game service or inherited fd.
//! rustc's jobserver has one implicit token. An empty private FIFO permits only
//! that worker; every CGU still receives the same independent optimization.
use anyhow::{ensure, Context, Result};
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    os::unix::{
        ffi::OsStrExt,
        fs::{FileTypeExt, OpenOptionsExt, PermissionsExt},
    },
    process::Command,
};
pub(super) struct Jobs {
    _keeper: File,
    directory: tempfile::TempDir,
}
impl Jobs {
    pub(super) fn new() -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("gamepkg-rustc-jobs-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()?;
        let path = directory.path().join("tokens");
        let name = CString::new(path.as_os_str().as_bytes())?;
        if unsafe { libc::mkfifo(name.as_ptr(), 0o600) } != 0 {
            return Err(std::io::Error::last_os_error())
                .context("create private compiler jobserver");
        }
        // Keep both ends open and empty until the confined compiler exits. No
        // token, outside descriptor, package-controlled path or host semaphore.
        let keeper = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(&path)?;
        ensure!(
            keeper.metadata()?.file_type().is_fifo(),
            "compiler jobserver is not a FIFO"
        );
        Ok(Self {
            _keeper: keeper,
            directory,
        })
    }
    pub(super) fn check_diagnostics(&self, stderr: &str) -> Result<()> {
        ensure!(
            !stderr.contains("failed to connect to jobserver"),
            "compiler could not join its private scheduling lane"
        );
        Ok(())
    }
    pub(super) fn configure(&self, command: &mut Command) {
        command
            .args(["--ro-bind"])
            .arg(self.directory.path())
            .arg("/compiler-control")
            .args([
                "--setenv",
                "CARGO_MAKEFLAGS",
                "--jobserver-auth=fifo:/compiler-control/tokens",
            ]);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        os::{fd::AsRawFd, unix::fs::PermissionsExt},
    };
    #[test]
    fn private_fifo_is_empty_noninherited_and_removed_with_its_guard() {
        let jobs = Jobs::new().unwrap();
        let root = jobs.directory.path().to_path_buf();
        let path = root.join("tokens");
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(std::fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_fifo());
        assert_ne!(
            unsafe { libc::fcntl(jobs._keeper.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        let mut inspect = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(&path)
            .unwrap();
        assert_eq!(
            inspect.read(&mut [0]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        inspect.write_all(b"+").unwrap();
        let mut byte = [0];
        inspect.read_exact(&mut byte).unwrap();
        assert_eq!(byte, [b'+']);
        let mut command = Command::new("bwrap");
        command.env("CARGO_MAKEFLAGS", "untrusted");
        jobs.configure(&mut command);
        let args: Vec<_> = command
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--ro-bind",
                root.to_str().unwrap(),
                "/compiler-control",
                "--setenv",
                "CARGO_MAKEFLAGS",
                "--jobserver-auth=fifo:/compiler-control/tokens"
            ]
        );
        jobs.check_diagnostics("ordinary compiler diagnostic")
            .unwrap();
        assert!(jobs
            .check_diagnostics("warning: failed to connect to jobserver from environment")
            .is_err());
        drop(inspect);
        drop(jobs);
        assert!(!root.exists());
    }
}
