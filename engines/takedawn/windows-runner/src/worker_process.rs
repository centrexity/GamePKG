//! Confined stdio workers. No selected-disc, restart, parent-console or other
//! handle is inherited. File access remains an explicit Sandbox grant.
use super::*;
use windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_ACTIVE_PROCESS;

pub struct WorkerProcess {
    process: Process,
    io: WorkerPipe,
    errors: WorkerLog,
}
impl WorkerProcess {
    /// Launch a single game worker. The AppContainer has no network capability;
    /// only its verified runtime directory should have been granted read access.
    pub fn spawn(
        executable: &Path,
        args: &[OsString],
        cwd: &Path,
        values: &[(OsString, OsString)],
        sandbox: &Sandbox,
        memory_limit_bytes: u64,
    ) -> io::Result<Self> {
        Self::spawn_inner(
            executable,
            args,
            cwd,
            values,
            Some(sandbox),
            memory_limit_bytes,
            1,
        )
    }
    /// Compiler tools may run their linker, within the same confined memory/job
    /// boundary. This never grants files or networking; the host grants only
    /// the selected toolchain and private source/output workspace beforehand.
    pub fn spawn_tool(
        executable: &Path,
        args: &[OsString],
        cwd: &Path,
        values: &[(OsString, OsString)],
        sandbox: &Sandbox,
        memory_limit_bytes: u64,
    ) -> io::Result<Self> {
        Self::spawn_inner(
            executable,
            args,
            cwd,
            values,
            Some(sandbox),
            memory_limit_bytes,
            16,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn spawn_inner(
        executable: &Path,
        args: &[OsString],
        cwd: &Path,
        values: &[(OsString, OsString)],
        sandbox: Option<&Sandbox>,
        memory_limit_bytes: u64,
        process_limit: u32,
    ) -> io::Result<Self> {
        if !executable.is_absolute() || !cwd.is_absolute() {
            return Err(invalid("worker executable and directory must be absolute"));
        }
        let bytes = usize::try_from(memory_limit_bytes)
            .map_err(|_| invalid("worker memory limit exceeds this process architecture"))?;
        if bytes == 0 {
            return Err(invalid("worker memory limit must be positive"));
        }
        let exe = wide(executable.as_os_str())?;
        let cwd = wide(cwd.as_os_str())?;
        let mut command = Vec::new();
        quoted(executable.as_os_str(), &mut command)?;
        for arg in args {
            command.push(b' ' as u16);
            quoted(arg, &mut command)?;
        }
        command.push(0);
        if command.len() > 32767 {
            return Err(invalid("worker command line is too long"));
        }
        let (io, peer) = WorkerPipe::pair()?;
        let (errors, error_peer) = WorkerPipe::pair()?;
        let input = duplicate(peer.as_raw_handle(), FILE_GENERIC_READ)?;
        let output = duplicate(peer.as_raw_handle(), FILE_GENERIC_WRITE)?;
        let error = duplicate(error_peer.as_raw_handle(), FILE_GENERIC_WRITE)?;
        let mut handles = [
            input.as_raw_handle(),
            output.as_raw_handle(),
            error.as_raw_handle(),
        ];
        let mut capabilities = sandbox.map(Sandbox::capabilities);
        let attributes = Attributes::new(&mut handles, capabilities.as_mut())?;
        let env = environment_block(values, None, None)?;
        let raw = unsafe { CreateJobObjectW(null(), null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_JOB_MEMORY
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = process_limit;
        limits.JobMemoryLimit = bytes;
        if unsafe {
            SetInformationJobObject(
                raw,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = input.as_raw_handle();
        startup.StartupInfo.hStdOutput = output.as_raw_handle();
        startup.StartupInfo.hStdError = error.as_raw_handle();
        startup.lpAttributeList = attributes.ptr;
        let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
        // Only explicitly listed pipe endpoints are inherited. Suspend until
        // job assignment so neither game nor linker can escape its boundary.
        if unsafe {
            CreateProcessW(
                exe.as_ptr(),
                command.as_mut_ptr(),
                null(),
                null(),
                1,
                CREATE_SUSPENDED
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT
                    | CREATE_NO_WINDOW,
                env.as_ptr().cast(),
                cwd.as_ptr(),
                &startup.StartupInfo,
                &mut info,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
        let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
        if unsafe { AssignProcessToJobObject(raw, process.as_raw_handle()) } == 0 {
            let error = io::Error::last_os_error();
            unsafe {
                TerminateProcess(process.as_raw_handle(), 1);
                WaitForSingleObject(process.as_raw_handle(), 5000);
            }
            return Err(error);
        }
        let process = Process {
            process,
            job,
            restart: None,
        };
        if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            process,
            io,
            errors: WorkerLog::new(errors),
        })
    }
    /// Each pipe has one owner. Session cleanup retains the process/job until
    /// its worker is stopped and pending log reads can observe the closed peer.
    pub fn into_parts(self) -> (Process, WorkerPipe, WorkerLog) {
        (self.process, self.io, self.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn stdio_child_fixture() {
        if std::env::var_os("GP_STDIO_CHILD").is_none() {
            return;
        }
        // The Rust test harness prints a header, so the parent consumes this
        // marker before testing the byte protocol. Actual workers have no harness.
        println!("GP_STDIO_READY");
        std::io::stdout().flush().unwrap();
        let mut bytes = [0; 8];
        std::io::stdin().read_exact(&mut bytes).unwrap();
        std::io::stdout().write_all(&bytes).unwrap();
        std::io::stdout().flush().unwrap();
        eprintln!("STDERR_SEPARATE");
        if let Some(path) = std::env::var_os("GP_DENIED_FILE") {
            let control = std::env::var_os("GP_SECURITY_CONTROL").is_some();
            let allow_child = std::env::var_os("GP_ALLOW_TOOL_CHILD").is_some();
            assert_eq!(File::open(path).is_ok(), control, "ungranted file boundary");
            assert_eq!(
                std::net::TcpListener::bind("127.0.0.1:0").is_ok(),
                control,
                "direct networking boundary"
            );
            use std::os::windows::process::CommandExt;
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.args(["--list"]).creation_flags(CREATE_NO_WINDOW);
            let nested = child.spawn();
            assert_eq!(nested.is_ok(), allow_child, "job process boundary");
            if let Ok(mut nested) = nested {
                assert!(nested.wait().unwrap().success());
            }
            assert!(std::env::var_os("GAMEPKG_TAKEDAWN_DISC_HANDLE").is_none());
        }
    }
    #[test]
    fn process_owns_private_binary_streams_and_job_lifetime() {
        let dir = tempfile::tempdir().unwrap();
        let exe = std::env::current_exe().unwrap();
        let args = vec![
            "--exact".into(),
            "worker_process::tests::stdio_child_fixture".into(),
            "--nocapture".into(),
        ];
        // Transport/job tests can run under Wine; AppContainer itself is
        // separately exercised on actual Windows below.
        let worker = WorkerProcess::spawn_inner(
            &exe,
            &args,
            dir.path(),
            &[("GP_STDIO_CHILD".into(), "1".into())],
            None,
            128 * 1048576,
            1,
        )
        .unwrap();
        let (process, mut pipe, mut errors) = worker.into_parts();
        let end = Instant::now() + Duration::from_secs(5);
        let mut line = Vec::new();
        while !line.ends_with(b"GP_STDIO_READY\n") {
            let mut byte = [0];
            pipe.read_exact(&mut byte, end).unwrap();
            line.push(byte[0]);
            assert!(line.len() < 4096);
        }
        pipe.write_all(b"\0\r\n\xffabcd", end).unwrap();
        let mut bytes = [0; 8];
        pipe.read_exact(&mut bytes, end).unwrap();
        assert_eq!(&bytes, b"\0\r\n\xffabcd");
        let mut log = Vec::new();
        let mut chunk = [0; 128];
        loop {
            match errors.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => log.extend_from_slice(&chunk[..n]),
            }
        }
        assert!(String::from_utf8_lossy(&log).contains("STDERR_SEPARATE"));
        while process.poll().unwrap().is_none() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(process.poll().unwrap(), Some(0));
    }

    #[test]
    fn confined_worker_keeps_io_but_denies_ungranted_files_network_and_processes() {
        let root = tempfile::tempdir().unwrap();
        let runtime = root.path().join("runtime");
        std::fs::create_dir(&runtime).unwrap();
        let exe = runtime.join("worker.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        let secret = root.path().join("not-granted.txt");
        std::fs::write(&secret, b"private marker").unwrap();
        let name = format!("GamePKG.WorkerTest.p{}", std::process::id());
        let sandbox = Sandbox::open(&name).unwrap();
        sandbox.grant_runtime(&runtime).unwrap();
        let args = vec![
            "--exact".into(),
            "worker_process::tests::stdio_child_fixture".into(),
            "--nocapture".into(),
        ];
        let mut values = vec![
            ("GP_STDIO_CHILD".into(), "1".into()),
            ("GP_DENIED_FILE".into(), secret.as_os_str().to_owned()),
        ];
        if let Some(root) = std::env::var_os("SystemRoot") {
            values.push(("SystemRoot".into(), root));
        }
        for mode in ["control", "worker", "tool"] {
            let mut values = values.clone();
            if mode == "control" {
                values.push(("GP_SECURITY_CONTROL".into(), "1".into()));
            }
            if mode != "worker" {
                values.push(("GP_ALLOW_TOOL_CHILD".into(), "1".into()));
            }
            let worker = match mode {
                "control" => WorkerProcess::spawn_inner(
                    &exe,
                    &args,
                    &runtime,
                    &values,
                    None,
                    128 * 1048576,
                    16,
                ),
                "worker" => {
                    WorkerProcess::spawn(&exe, &args, &runtime, &values, &sandbox, 128 * 1048576)
                }
                _ => WorkerProcess::spawn_tool(
                    &exe,
                    &args,
                    &runtime,
                    &values,
                    &sandbox,
                    128 * 1048576,
                ),
            }
            .unwrap();
            let (process, mut pipe, mut errors) = worker.into_parts();
            let end = Instant::now() + Duration::from_secs(10);
            let mut header = Vec::new();
            while !header.ends_with(b"GP_STDIO_READY\n") {
                let mut byte = [0];
                pipe.read_exact(&mut byte, end).unwrap();
                header.push(byte[0]);
                assert!(header.len() < 4096);
            }
            pipe.write_all(b"nativeIO", end).unwrap();
            let mut bytes = [0; 8];
            pipe.read_exact(&mut bytes, end).unwrap();
            assert_eq!(&bytes, b"nativeIO");
            let mut log = Vec::new();
            let mut chunk = [0; 256];
            loop {
                match errors.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        log.extend_from_slice(&chunk[..n]);
                        assert!(log.len() < 8192);
                    }
                }
            }
            while process.poll().unwrap().is_none() {
                assert!(Instant::now() < end);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(
                process.poll().unwrap(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&log)
            );
        }
        assert_eq!(std::fs::read(&secret).unwrap(), b"private marker");
    }

    #[test]
    fn plain_worker_environment_is_double_terminated_and_has_no_disc_capability() {
        assert_eq!(environment_block(&[], None, None).unwrap(), [0, 0]);
        let env = environment_block(&[("MODE".into(), "native".into())], None, None).unwrap();
        assert_eq!(String::from_utf16(&env).unwrap(), "MODE=native\0\0");
    }
}
