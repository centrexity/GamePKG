//! Bounded native compiler processes inside a Windows AppContainer and owned job.
use anyhow::{ensure, Context, Result};
use gamepkg_takedawn_windows_runner::{Sandbox, WorkerProcess};
use std::{
    ffi::OsString,
    io::{self, Read},
    path::Path,
    time::{Duration, Instant},
};

const DIAGNOSTICS: usize = 1048576;
fn capture(mut read: impl FnMut(&mut [u8]) -> io::Result<usize>) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        match read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                let keep = n.min(DIAGNOSTICS - output.len());
                output.extend_from_slice(&buffer[..keep]);
                truncated |= keep < n;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => break,
            Err(e) => return Err(e),
        }
    }
    if truncated {
        output.extend_from_slice(b"\n[compiler diagnostics truncated at 1 MiB per stream]\n");
    }
    Ok(output)
}

pub(crate) fn run(
    compiler: &Path,
    args: &[OsString],
    directory: &Path,
    values: &[(OsString, OsString)],
    sandbox: &Sandbox,
    end: Instant,
) -> Result<Vec<u8>> {
    ensure!(
        Instant::now() < end,
        "isolated compiler exceeded its time limit"
    );
    let worker = WorkerProcess::spawn_tool(
        compiler,
        args,
        directory,
        values,
        sandbox,
        crate::compiler::ADDRESS_SPACE,
    )
    .context("compilation requires Windows AppContainer isolation")?;
    let (process, mut pipe, mut errors) = worker.into_parts();
    let (send, receive) = std::sync::mpsc::channel();
    let output_send = send.clone();
    std::thread::spawn(move || {
        let _ = output_send.send(capture(|bytes| pipe.read_some(bytes, end)));
    });
    std::thread::spawn(move || {
        let _ = send.send(capture(|bytes| errors.read(bytes)));
    });
    let status = loop {
        if let Some(code) = process.poll()? {
            break code;
        }
        ensure!(
            Instant::now() < end,
            "isolated compiler exceeded its time limit"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    // Close any inherited pipe writers in a leftover contained child too.
    process.stop()?;
    let mut diagnostics = Vec::new();
    for _ in 0..2 {
        diagnostics.extend(
            receive
                .recv_timeout(end.saturating_duration_since(Instant::now()))
                .context("isolated compiler output exceeded its time limit")??,
        );
    }
    ensure!(
        Instant::now() < end,
        "isolated compiler exceeded its time limit"
    );
    ensure!(
        status == 0,
        "isolated build failed (Windows exit {status}): {}",
        String::from_utf8_lossy(&diagnostics)
    );
    Ok(diagnostics)
}

/// A temporary name rooted at an already verified toolchain. LLVM expands its
/// image name before parsing options, so ancestors outside its granted folder
/// must not be part of that name. This adds no ACL or process capabilities.
pub(crate) struct PrivateToolRoot {
    path: std::path::PathBuf,
    name: Vec<u16>,
    target: Vec<u16>,
    active: bool,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn DefineDosDeviceW(flags: u32, name: *const u16, target: *const u16) -> i32;
    fn QueryDosDeviceW(name: *const u16, target: *mut u16, length: u32) -> u32;
}
impl PrivateToolRoot {
    pub(crate) fn open(directory: &Path) -> Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        ensure!(
            directory.is_absolute() && directory.is_dir(),
            "compiler root must be an absolute directory"
        );
        let target = crate::takedawn_windows_sdk::compiler_path(&directory.canonicalize()?)?;
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce)
            .map_err(|error| anyhow::anyhow!("private compiler name: {error}"))?;
        let name = format!("GamePKGCompiler{}", hex::encode(nonce));
        let mut root = Self {
            path: std::path::PathBuf::from(format!(r"\\?\{name}\")),
            name: std::ffi::OsStr::new(&name)
                .encode_wide()
                .chain(Some(0))
                .collect(),
            target: target.as_os_str().encode_wide().chain(Some(0)).collect(),
            active: false,
        };
        let mut previous = [0u16; 32768];
        // SAFETY: terminated name and writable buffer with its exact length.
        let exists = unsafe {
            QueryDosDeviceW(
                root.name.as_ptr(),
                previous.as_mut_ptr(),
                previous.len() as u32,
            )
        };
        let error = io::Error::last_os_error();
        ensure!(
            exists == 0 && error.raw_os_error() == Some(2),
            "private compiler name already exists or cannot be queried: {error}"
        );
        // SAFETY: terminated buffers and a fresh random name. No drive letter,
        // mount point, existing device, file grant or broadcast is changed.
        ensure!(
            unsafe { DefineDosDeviceW(8, root.name.as_ptr(), root.target.as_ptr()) } != 0,
            "private compiler root: {}",
            io::Error::last_os_error()
        );
        root.active = true;
        Ok(root)
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn close(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        // SAFETY: exact-match removal only removes our own mapping. The active
        // flag is set only after successful creation and cleared after removal.
        ensure!(
            unsafe { DefineDosDeviceW(2 | 4 | 8, self.name.as_ptr(), self.target.as_ptr()) } != 0,
            "private compiler root cleanup: {}",
            io::Error::last_os_error()
        );
        self.active = false;
        Ok(())
    }
}
impl Drop for PrivateToolRoot {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod private_root_tests {
    use super::*;
    #[test]
    fn private_tool_roots_are_unique_and_removed_independently() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("marker"), b"private root marker").unwrap();
        let mut first = PrivateToolRoot::open(temp.path()).unwrap();
        let second = PrivateToolRoot::open(temp.path()).unwrap();
        let first_path = first.path().join("marker");
        let second_path = second.path().join("marker");
        assert_ne!(first.path(), second.path());
        assert_eq!(std::fs::read(&first_path).unwrap(), b"private root marker");
        assert_eq!(std::fs::read(&second_path).unwrap(), b"private root marker");
        first.close().unwrap();
        let mut mapping = [0u16; 32768];
        // SAFETY: the name is terminated and the output buffer has its stated length.
        let exists = unsafe {
            QueryDosDeviceW(
                first.name.as_ptr(),
                mapping.as_mut_ptr(),
                mapping.len() as u32,
            )
        };
        let error = io::Error::last_os_error();
        assert_eq!(exists, 0);
        assert_eq!(error.raw_os_error(), Some(2));
        assert!(!first_path.exists());
        assert_eq!(std::fs::read(&second_path).unwrap(), b"private root marker");
        drop(second);
        assert!(!second_path.exists());
        first.close().unwrap();
        assert!(PrivateToolRoot::open(Path::new("relative")).is_err());
    }
}
