//! Per-process Linux compiler limits and bounded diagnostic capture.
use anyhow::{Context, Result};
use std::{
    io::Read,
    process::{Command, ExitStatus, Stdio},
    thread,
};
pub const ADDRESS_SPACE: u64 = 2 * 1024 * 1024 * 1024;
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
pub(crate) const SERVICE_MEMORY_BUDGET: std::ops::RangeInclusive<u64> =
    256 * 1024 * 1024..=48 * 1024 * 1024 * 1024;
/// Maximum stored size of one compiler tool or its pinned download.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
pub(crate) const MAX_TOOL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const DIAGNOSTICS: usize = 1024 * 1024;
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stdout_bytes: Vec<u8>,
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
/// Error messages often follow large warning streams. Keep a bounded prefix
/// and suffix for stderr; machine-readable stdout keeps its original prefix
/// policy and truncation indicator so incomplete tool output cannot be accepted.
pub(crate) fn drain_diagnostics(
    mut read: impl FnMut(&mut [u8]) -> std::io::Result<usize>,
) -> std::io::Result<(Vec<u8>, bool)> {
    let half = DIAGNOSTICS / 2;
    let mut head = Vec::with_capacity(half);
    let mut tail = std::collections::VecDeque::<u8>::with_capacity(half);
    let mut total = 0usize;
    let mut buffer = [0u8; 8192];
    loop {
        let count = match read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        total = total.saturating_add(count);
        let prefix = count.min(half - head.len());
        head.extend_from_slice(&buffer[..prefix]);
        let rest = &buffer[prefix..count];
        if rest.len() >= half {
            tail.clear();
            tail.extend(&rest[rest.len() - half..]);
        } else {
            let discard = (tail.len() + rest.len()).saturating_sub(half);
            tail.drain(..discard);
            tail.extend(rest);
        }
    }
    head.extend(tail);
    Ok((head, total > DIAGNOSTICS))
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
    let err = thread::spawn(move || {
        let mut stderr = stderr;
        drain_diagnostics(|bytes| stderr.read(bytes))
    });
    let status = child.wait()?;
    let (stdout, out_truncated) = out
        .join()
        .map_err(|_| anyhow::anyhow!("compiler output reader stopped"))??;
    let (bytes, err_truncated) = err
        .join()
        .map_err(|_| anyhow::anyhow!("compiler diagnostic reader stopped"))??;
    let mut stderr = String::from_utf8_lossy(&bytes).into_owned();
    if out_truncated || err_truncated {
        stderr.push_str("\n[compiler diagnostics truncated at 1 MiB per stream; stderr beginning and end retained]");
    }
    Ok(Output {
        status,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stdout_bytes: stdout,
        stderr,
    })
}
/// Bound launcher execution and pipe draining independently of guest timeouts.
pub(crate) fn run_timed(command: &mut Command, limit: std::time::Duration) -> Result<Output> {
    let deadline = std::time::Instant::now() + limit;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let (send, receive) = std::sync::mpsc::channel();
    let out_send = send.clone();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    thread::spawn(move || {
        let _ = out_send.send((true, drain(stdout)));
    });
    thread::spawn(move || {
        let mut stderr = stderr;
        let _ = send.send((false, drain_diagnostics(|bytes| stderr.read(bytes))));
    });
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("compiler launcher exceeded its time limit");
        }
        thread::sleep(std::time::Duration::from_millis(20));
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut truncated = false;
    for _ in 0..2 {
        let (is_stdout, result) = receive
            .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .context("compiler output exceeded its time limit")?;
        let (bytes, cut) = result?;
        truncated |= cut;
        if is_stdout {
            stdout = bytes;
        } else {
            stderr = bytes;
        }
    }
    let mut stderr = String::from_utf8_lossy(&stderr).into_owned();
    if truncated {
        stderr.push_str("\n[compiler diagnostics truncated at 1 MiB per stream; stderr beginning and end retained]");
    }
    Ok(Output {
        status,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stdout_bytes: stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn launcher_and_inherited_pipes_have_a_deadline() {
        use std::time::{Duration, Instant};
        for script in ["sleep 2", "sleep 2 & exit 0"] {
            let start = Instant::now();
            let mut command = Command::new("sh");
            command.args(["-c", script]);
            assert!(run_timed(&mut command, Duration::from_millis(80)).is_err());
            assert!(start.elapsed() < Duration::from_secs(1));
        }
        let mut command = Command::new("sh");
        command.args(["-c", "printf complete; printf diagnostic >&2"]);
        let output = run_timed(&mut command, Duration::from_secs(1)).unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, "complete");
        assert_eq!(output.stderr, "diagnostic");
    }
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
    #[test]
    fn warning_floods_keep_final_errors_without_changing_machine_stdout() {
        let mut input = vec![b'w'; DIAGNOSTICS * 3];
        input[..5].copy_from_slice(b"START");
        let end = input.len();
        input[end - 11..].copy_from_slice(b"FINAL ERROR");
        for chunk in [1, 17, 8192] {
            let mut position = 0;
            let (bytes, truncated) = drain_diagnostics(|out| {
                let count = (input.len() - position).min(chunk).min(out.len());
                out[..count].copy_from_slice(&input[position..position + count]);
                position += count;
                Ok(count)
            })
            .unwrap();
            assert!(truncated);
            assert_eq!(bytes.len(), DIAGNOSTICS);
            assert!(bytes.starts_with(b"START"));
            assert!(bytes.ends_with(b"FINAL ERROR"));
        }
        let (stdout, truncated) = drain(input.as_slice()).unwrap();
        assert!(truncated);
        assert!(!stdout.ends_with(b"FINAL ERROR"));
        for length in [0, 1, DIAGNOSTICS / 2, DIAGNOSTICS - 1, DIAGNOSTICS] {
            let mut slice = &input[..length];
            assert_eq!(
                drain_diagnostics(|bytes| slice.read(bytes)).unwrap(),
                (input[..length].to_vec(), false)
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn failed_process_keeps_exit_status_and_final_diagnostic_after_warning_flood() {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "printf START >&2; head -c 2097152 /dev/zero >&2; printf FINAL_ERROR >&2; exit 7",
        ]);
        let output = run(&mut command).unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(output.stderr.starts_with("START"));
        assert!(output.stderr.contains("FINAL_ERROR"));
        assert!(output.stderr.contains("truncated"));
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
