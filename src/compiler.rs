//! Per-process Linux compiler limits and bounded diagnostic capture.
use anyhow::{Context, Result};
use std::{
    io::Read,
    process::{Command, ExitStatus, Stdio},
    thread,
};
pub const ADDRESS_SPACE: u64 = 2 * 1024 * 1024 * 1024;
pub const DIAGNOSTICS: usize = 1024 * 1024;
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}
pub fn command(max_file: u64) -> Command {
    let mut command = Command::new("prlimit");
    command.args([
        format!("--as={ADDRESS_SPACE}"),
        format!("--fsize={max_file}"),
        "--core=0".into(),
        "--".into(),
        "timeout".into(),
        "60".into(),
    ]);
    command
}
fn drain(mut pipe: impl Read) -> std::io::Result<(Vec<u8>, bool)> {
    let mut captured = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        let count = pipe.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let keep = count.min(DIAGNOSTICS - captured.len());
        captured.extend_from_slice(&buffer[..keep]);
        truncated |= keep < count;
    }
    Ok((captured, truncated))
}
pub fn run(command: &mut Command) -> Result<Output> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("isolated builds require prlimit, timeout and bubblewrap")?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || drain(stdout));
    let err = thread::spawn(move || drain(stderr));
    let status = child.wait()?;
    let (stdout, out_truncated) = out
        .join()
        .map_err(|_| anyhow::anyhow!("compiler output reader stopped"))??;
    let (bytes, err_truncated) = err
        .join()
        .map_err(|_| anyhow::anyhow!("compiler diagnostic reader stopped"))??;
    let mut stderr = String::from_utf8_lossy(&bytes).into_owned();
    if out_truncated || err_truncated {
        stderr.push_str("\n[compiler diagnostics truncated at 1 MiB per stream]");
    }
    Ok(Output {
        status,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excessive_diagnostics_are_drained_but_not_retained() {
        let input = vec![b'x'; DIAGNOSTICS * 3];
        let (bytes, truncated) = drain(input.as_slice()).unwrap();
        assert_eq!(bytes.len(), DIAGNOSTICS);
        assert!(truncated);
        assert_eq!(
            drain(b"short".as_slice()).unwrap(),
            (b"short".to_vec(), false)
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn child_inherits_enforced_address_space_and_file_limits() {
        let mut command = command(1024);
        command.args(["python3","-c","import resource; assert resource.getrlimit(resource.RLIMIT_AS) == (2147483648,2147483648); assert resource.getrlimit(resource.RLIMIT_FSIZE) == (1024,1024); assert resource.getrlimit(resource.RLIMIT_CORE) == (0,0)"]);
        let output = run(&mut command).unwrap();
        assert!(output.status.success(), "{}", output.stderr);
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn oversized_allocations_and_writes_are_refused_by_the_kernel() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("output");
        let mut command = command(1024);
        command
            .args([
                "python3",
                "-c",
                r#"import os,sys,signal,errno
try:
    bytearray(3*1024*1024*1024)
except MemoryError:
    pass
else:
    raise AssertionError('memory cap not enforced')
signal.signal(signal.SIGXFSZ,signal.SIG_IGN)
f=os.open(sys.argv[1],os.O_WRONLY|os.O_CREAT,0o600)
os.write(f,b'x'*2048)
try:
    os.write(f,b'x')
except OSError as e:
    assert e.errno==errno.EFBIG
else:
    raise AssertionError('file cap not enforced')
os.close(f)
"#,
            ])
            .arg(&file);
        let output = run(&mut command).unwrap();
        assert!(output.status.success(), "{}", output.stderr);
        assert_eq!(std::fs::metadata(file).unwrap().len(), 1024);
    }
}
