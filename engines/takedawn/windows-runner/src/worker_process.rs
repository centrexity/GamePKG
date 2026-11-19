//! Confined stdio workers. A compatibility executable can also receive the
//! explicitly selected ROM files as read-only handles.
use super::*;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
use windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_ACTIVE_PROCESS;

pub(super) fn require_profile_environment(values: &[(OsString, OsString)]) -> io::Result<()> {
    // AppContainer startup resolves its profile through LOCALAPPDATA. Keep the
    // required OS paths explicit instead of restoring the inherited environment.
    // https://learn.microsoft.com/windows/win32/secauthz/implementing-an-appcontainer
    for required in [b"LOCALAPPDATA".as_slice(), b"SystemRoot"] {
        let value = values
            .iter()
            .find(|(key, _)| key.as_encoded_bytes().eq_ignore_ascii_case(required))
            .map(|(_, value)| value);
        if !value.is_some_and(|value| Path::new(value).is_absolute()) {
            return Err(invalid(
                "confined workers require explicit absolute LOCALAPPDATA and SystemRoot paths",
            ));
        }
    }
    Ok(())
}

/// Verify the containment needed by the linked desktop compatibility entry.
pub fn require_compatibility_process(memory_limit_bytes: u64) -> io::Result<()> {
    use windows_sys::Win32::{
        Security::{GetTokenInformation, TokenIsAppContainer, TOKEN_QUERY},
        System::Threading::OpenProcessToken,
    };
    let mut token = null_mut();
    // SAFETY: the pseudo-handle is current process; token is valid output.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut container = 0u32;
    let mut length = 0;
    // SAFETY: TokenIsAppContainer returns a DWORD.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenIsAppContainer,
            (&mut container as *mut u32).cast(),
            size_of::<u32>() as u32,
            &mut length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if container != 1 || length != size_of::<u32>() as u32 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native compatibility requires a Windows AppContainer",
        ));
    }
    check_compatibility_job(memory_limit_bytes)
}

fn check_compatibility_job(memory_limit_bytes: u64) -> io::Result<()> {
    // A null handle queries the immediate job of this process.
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
    if unsafe {
        QueryInformationJobObject(
            null_mut(),
            JobObjectExtendedLimitInformation,
            (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    check_job_limits(&limits, memory_limit_bytes)
}

const COMPATIBILITY_JOB_FLAGS: u32 = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
    | JOB_OBJECT_LIMIT_JOB_MEMORY
    | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;

fn check_job_limits(
    limits: &JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    memory_limit_bytes: u64,
) -> io::Result<()> {
    let basic = limits.BasicLimitInformation;
    if basic.LimitFlags & COMPATIBILITY_JOB_FLAGS != COMPATIBILITY_JOB_FLAGS
        || basic.ActiveProcessLimit != 1
        || memory_limit_bytes == 0
        || limits.JobMemoryLimit as u64 != memory_limit_bytes
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native compatibility requires the signed memory limit and a one-process job",
        ));
    }
    Ok(())
}

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
            &[],
        )
    }

    /// Start a single confined compatibility executable with ROM handles in
    /// manifest order. No ROM path or directory is granted to the child.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_compatibility(
        executable: &Path,
        args: &[OsString],
        cwd: &Path,
        values: &[(OsString, OsString)],
        sandbox: &Sandbox,
        memory_limit_bytes: u64,
        roms: &[File],
    ) -> io::Result<Self> {
        Self::spawn_inner(
            executable,
            args,
            cwd,
            values,
            Some(sandbox),
            memory_limit_bytes,
            1,
            roms,
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
            &[],
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
        roms: &[File],
    ) -> io::Result<Self> {
        if !executable.is_absolute() || !cwd.is_absolute() {
            return Err(invalid("worker executable and directory must be absolute"));
        }
        if sandbox.is_some() {
            require_profile_environment(values)?;
        }
        let bytes = usize::try_from(memory_limit_bytes)
            .map_err(|_| invalid("worker memory limit exceeds this process architecture"))?;
        if bytes == 0 {
            return Err(invalid("worker memory limit must be positive"));
        }
        let exe = wide(executable.as_os_str())?;
        let cwd = wide(cwd.as_os_str())?;
        let mut command = Vec::new();
        // lpApplicationName below selects the exact absolute image. Keep the
        // extended Windows path out of argv[0]: MinGW/LLVM startup code may
        // interpret the device prefix as a command or filesystem namespace.
        // A basename here cannot redirect image selection or invoke PATH lookup.
        quoted(
            executable
                .file_name()
                .ok_or_else(|| invalid("worker executable has no filename"))?,
            &mut command,
        )?;
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
        // Native CRTs query pipe attributes when initializing stdout/stderr.
        // Match Rust's own inherited write end: writable data, readable
        // attributes, but no read-data access to the diagnostic channel.
        let output = duplicate(
            peer.as_raw_handle(),
            FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES,
        )?;
        let error = duplicate(
            error_peer.as_raw_handle(),
            FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES,
        )?;
        let roms = rom_grants::prepare(roms)?;
        let mut handles = vec![
            input.as_raw_handle(),
            output.as_raw_handle(),
            error.as_raw_handle(),
        ];
        handles.extend(roms.iter().map(AsRawHandle::as_raw_handle));
        let mut capabilities = sandbox.map(Sandbox::capabilities);
        let attributes = Attributes::new(&mut handles, capabilities.as_mut())?;
        let env = environment_block(values, None, None, &roms)?;
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
        // Only the listed pipes and ROMs are inherited. Suspend until the
        // job owns the process, before any child code runs.
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
            // Binding a socket does not establish network access. Attempt a
            // real connection to the parent's live listener; the unconfined
            // control proves the endpoint is reachable on this same machine.
            let address = std::env::var("GP_NETWORK_ENDPOINT")
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(
                std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_ok(),
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
            &[],
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
    fn compatibility_requires_every_job_limit_and_the_signed_memory_value() {
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = COMPATIBILITY_JOB_FLAGS;
        limits.BasicLimitInformation.ActiveProcessLimit = 1;
        limits.JobMemoryLimit = 128 * 1048576;
        assert!(check_job_limits(&limits, 128 * 1048576).is_ok());
        assert!(check_job_limits(&limits, 129 * 1048576).is_err());
        assert!(check_job_limits(&limits, 0).is_err());
        limits.BasicLimitInformation.ActiveProcessLimit = 2;
        assert!(check_job_limits(&limits, 128 * 1048576).is_err());
        limits.BasicLimitInformation.ActiveProcessLimit = 1;
        for flag in [
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOB_OBJECT_LIMIT_JOB_MEMORY,
            JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        ] {
            limits.BasicLimitInformation.LimitFlags = COMPATIBILITY_JOB_FLAGS & !flag;
            assert!(check_job_limits(&limits, 128 * 1048576).is_err());
        }
    }

    #[test]
    fn rom_handoff_child_fixture() {
        if std::env::var_os("GP_ROM_HANDOFF_CHILD").is_none() {
            return;
        }
        if std::env::var_os("GP_ROM_ORIGINAL_PATH").is_some() {
            require_compatibility_process(128 * 1048576).unwrap();
            assert!(require_compatibility_process(129 * 1048576).is_err());
        }
        // The fixture parent passes two distinct handles through spawn_inner.
        let mut files = open_inherited_roms(2).unwrap();
        let mut bytes = [0; 4];
        files[0].read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"IWAD");
        files[1].read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"PWAD");
        for file in &mut files {
            assert!(file.write_all(b"no").is_err());
            let mut flags = 0;
            assert_ne!(
                unsafe {
                    windows_sys::Win32::Foundation::GetHandleInformation(
                        file.as_raw_handle(),
                        &mut flags,
                    )
                },
                0
            );
            assert_eq!(
                flags & windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
                0
            );
        }
        if let Some(path) = std::env::var_os("GP_ROM_ORIGINAL_PATH") {
            assert!(
                File::open(path).is_err(),
                "the ROM parent directory must remain outside the AppContainer grant"
            );
        }
        println!("GP_ROM_HANDOFF_OK");
        std::io::stdout().flush().unwrap();
    }

    fn check_rom_handoff(confined: bool) {
        let root = tempfile::tempdir().unwrap();
        let runtime = root.path().join("runtime");
        std::fs::create_dir(&runtime).unwrap();
        let exe = runtime.join("runtime.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        let mut paths = Vec::new();
        let mut files = Vec::new();
        for (name, bytes) in [("base", b"IWAD"), ("patch", b"PWAD")] {
            let path = root.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            // Deliberately pass writable parent handles to prove the child
            // receives reduced access independently of how the parent opened.
            files.push(
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .unwrap(),
            );
            paths.push(path);
        }

        let args = [
            "--exact".into(),
            "worker_process::tests::rom_handoff_child_fixture".into(),
            "--nocapture".into(),
        ];
        let mut values = vec![("GP_ROM_HANDOFF_CHILD".into(), "1".into())];
        let sandbox;
        let worker = if confined {
            values.push((
                "GP_ROM_ORIGINAL_PATH".into(),
                paths[0].as_os_str().to_owned(),
            ));
            for key in ["SystemRoot", "LOCALAPPDATA"] {
                values.push((key.into(), std::env::var_os(key).unwrap()));
            }
            let name = format!("GamePKG.RomGrantTest.p{}", std::process::id());
            sandbox = Sandbox::open(&name).unwrap();
            sandbox.grant_runtime(&runtime).unwrap();
            WorkerProcess::spawn_compatibility(
                &exe,
                &args,
                &runtime,
                &values,
                &sandbox,
                128 * 1048576,
                &files,
            )
        } else {
            WorkerProcess::spawn_inner(
                &exe,
                &args,
                &runtime,
                &values,
                None,
                128 * 1048576,
                1,
                &files,
            )
        }
        .unwrap();
        let (process, mut pipe, mut errors) = worker.into_parts();
        let end = Instant::now() + Duration::from_secs(10);
        let mut line = Vec::new();
        while !line.ends_with(b"GP_ROM_HANDOFF_OK\n") {
            let mut byte = [0];
            if let Err(error) = pipe.read_exact(&mut byte, end) {
                let mut diagnostic = Vec::new();
                // Reading the separate error channel is safe after the child exits.
                if process.poll().unwrap().is_some() {
                    let _ = errors.by_ref().take(8192).read_to_end(&mut diagnostic);
                }
                panic!(
                    "ROM child failed: {error}, stdout: {} stderr: {}",
                    String::from_utf8_lossy(&line),
                    String::from_utf8_lossy(&diagnostic)
                );
            }
            line.push(byte[0]);
            assert!(line.len() < 4096);
        }
        while process.poll().unwrap().is_none() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(process.poll().unwrap(), Some(0));
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"IWAD");
        assert_eq!(std::fs::read(&paths[1]).unwrap(), b"PWAD");
    }

    #[test]
    fn compatibility_process_keeps_rom_order_and_read_only_access() {
        check_rom_handoff(false);
    }

    #[test]
    #[ignore = "needs actual Windows AppContainer support"]
    fn appcontainer_compatibility_process_keeps_rom_order_and_read_only_access() {
        check_rom_handoff(true);
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
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let args = vec![
            "--exact".into(),
            "worker_process::tests::stdio_child_fixture".into(),
            "--nocapture".into(),
        ];
        let mut values = vec![
            ("GP_STDIO_CHILD".into(), "1".into()),
            ("GP_DENIED_FILE".into(), secret.as_os_str().to_owned()),
            (
                "GP_NETWORK_ENDPOINT".into(),
                listener.local_addr().unwrap().to_string().into(),
            ),
        ];
        for key in ["SystemRoot", "LOCALAPPDATA"] {
            values.push((
                key.into(),
                std::env::var_os(key).expect("native Windows profile environment"),
            ));
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
                    &[],
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
            .unwrap_or_else(|error| panic!("worker launch mode {mode}: {error}"));
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
                "worker mode {mode}: {}",
                String::from_utf8_lossy(&log)
            );
        }
        assert_eq!(std::fs::read(&secret).unwrap(), b"private marker");
    }

    #[test]
    fn plain_worker_environment_is_double_terminated_and_has_no_disc_capability() {
        assert_eq!(environment_block(&[], None, None, &[]).unwrap(), [0, 0]);
        let env = environment_block(&[("MODE".into(), "native".into())], None, None, &[]).unwrap();
        assert_eq!(String::from_utf16(&env).unwrap(), "MODE=native\0\0");
    }
    #[test]
    fn confined_worker_requires_only_explicit_profile_paths() {
        assert!(require_profile_environment(&[]).is_err());
        assert!(
            require_profile_environment(&[("SystemRoot".into(), r"C:\Windows".into())]).is_err()
        );
        assert!(require_profile_environment(&[
            ("SystemRoot".into(), r"C:\Windows".into()),
            ("LOCALAPPDATA".into(), "relative".into())
        ])
        .is_err());
        assert!(require_profile_environment(&[
            ("systemroot".into(), r"C:\Windows".into()),
            (
                "localappdata".into(),
                r"C:\Users\Tester\AppData\Local".into()
            )
        ])
        .is_ok());
    }
}
