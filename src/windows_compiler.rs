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
