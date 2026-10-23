//! Host-owned Windows process lifetime and selected ISO handoff.
//! This is not filesystem/network isolation or a package admission API. Callers
//! must verify the executable and arrange its confinement before player use.
#![cfg(windows)]
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
    mem::{size_of, zeroed},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
            JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
            TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Memory::{GetProcessHeap, HeapAlloc, HeapFree},
        Threading::{
            CreateEventW, CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess,
            GetExitCodeProcess, InitializeProcThreadAttributeList, ResumeThread, TerminateProcess,
            UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
            CREATE_UNICODE_ENVIRONMENT, EVENT_MODIFY_STATE, EXTENDED_STARTUPINFO_PRESENT,
            LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            STARTF_USESTDHANDLES, STARTUPINFOEXW,
        },
    },
};
const FILE_GENERIC_READ: u32 = 0x0012_0089;
const FILE_GENERIC_WRITE: u32 = 0x0012_0116;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut out: Vec<_> = value.encode_wide().collect();
    if out.contains(&0) {
        return Err(invalid("Windows arguments cannot contain NUL"));
    }
    out.push(0);
    Ok(out)
}
fn quoted(value: &OsStr, out: &mut Vec<u16>) -> io::Result<()> {
    // MS C-runtime quoting, including empty arguments and backslashes before
    // quotes/the closing quote. No shell or command interpreter is involved.
    let value = wide(value)?;
    out.push(b'"' as u16);
    let mut slashes = 0;
    for &c in &value[..value.len() - 1] {
        if c == b'\\' as u16 {
            slashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            b'\\' as u16,
            if c == b'"' as u16 {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        out.push(c);
        slashes = 0;
    }
    out.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
    out.push(b'"' as u16);
    Ok(())
}
fn environment(
    values: &[(OsString, OsString)],
    disc: HANDLE,
    restart: Option<HANDLE>,
) -> io::Result<Vec<u16>> {
    let mut entries = Vec::new();
    for (key, value) in values {
        let key = wide(key)?;
        let value = wide(value)?;
        if key.len() == 1 || key.contains(&(b'=' as u16)) {
            return Err(invalid("invalid environment name"));
        }
        let name = String::from_utf16(&key[..key.len() - 1])
            .map_err(|_| invalid("invalid environment name"))?
            .to_uppercase();
        if matches!(
            name.as_str(),
            "GAMEPKG_TAKEDAWN_DISC_HANDLE"
                | "GAMEPKG_TAKEDAWN_MANAGED_RESTART"
                | "GAMEPKG_TAKEDAWN_RESTART_HANDLE"
        ) || entries.iter().any(|(k, _)| k == &name)
        {
            return Err(invalid("duplicate or reserved environment name"));
        }
        let mut pair = key[..key.len() - 1].to_vec();
        pair.push(b'=' as u16);
        pair.extend(value);
        entries.push((name, pair));
    }
    let mut pair = wide(OsStr::new(&format!(
        "GAMEPKG_TAKEDAWN_DISC_HANDLE={}",
        disc as usize
    )))?;
    entries.push((
        "GAMEPKG_TAKEDAWN_DISC_HANDLE".into(),
        std::mem::take(&mut pair),
    ));
    if let Some(handle) = restart {
        entries.push((
            "GAMEPKG_TAKEDAWN_RESTART_HANDLE".into(),
            wide(OsStr::new(&format!(
                "GAMEPKG_TAKEDAWN_RESTART_HANDLE={}",
                handle as usize
            )))?,
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out: Vec<u16> = entries.into_iter().flat_map(|(_, pair)| pair).collect();
    out.push(0);
    Ok(out)
}
fn duplicate(source: HANDLE, access: u32) -> io::Result<OwnedHandle> {
    let mut copy = null_mut();
    // SAFETY: source is borrowed from a live File; Windows validates it and
    // creates a distinct handle. Only the successful result becomes owned.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            source,
            GetCurrentProcess(),
            &mut copy,
            access,
            1,
            if access == 0 {
                DUPLICATE_SAME_ACCESS
            } else {
                0
            },
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(copy) })
}
struct Attributes {
    ptr: LPPROC_THREAD_ATTRIBUTE_LIST,
}
impl Attributes {
    fn new(handles: &mut [HANDLE]) -> io::Result<Self> {
        let mut bytes = 0;
        // SAFETY: Windows reports the allocation size through a valid output.
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut bytes);
        }
        if bytes == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: process-heap memory has Win32's required native alignment.
        let ptr = unsafe { HeapAlloc(GetProcessHeap(), 0, bytes) } as LPPROC_THREAD_ATTRIBUTE_LIST;
        if ptr.is_null() {
            return Err(io::Error::other("unable to allocate process attributes"));
        }
        if unsafe { InitializeProcThreadAttributeList(ptr, 1, 0, &mut bytes) } == 0 {
            let error = io::Error::last_os_error();
            unsafe {
                HeapFree(GetProcessHeap(), 0, ptr.cast());
            }
            return Err(error);
        }
        let result = Self { ptr };
        // SAFETY: list is initialized; handles remain live and at the same
        // addresses until CreateProcessW has returned.
        if unsafe {
            UpdateProcThreadAttribute(
                ptr,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_mut_ptr().cast(),
                std::mem::size_of_val(handles),
                null_mut(),
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(result)
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: this initialized list owns its matching heap allocation.
        unsafe {
            DeleteProcThreadAttributeList(self.ptr);
            HeapFree(GetProcessHeap(), 0, self.ptr.cast());
        }
    }
}

/// An owned process tree. Drop stops it; callers must retain this object through
/// every launch/restart so tracking and save ownership outlive the game window.
pub struct Process {
    process: OwnedHandle,
    job: OwnedHandle,
    restart: Option<OwnedHandle>,
}
impl Process {
    /// Start the exact native executable, suspended until job assignment. Only
    /// the read-only disc, output log and NUL stdin are inherited. The caller's
    /// originals retain their ownership and inheritance flags.
    pub fn spawn(
        executable: &Path,
        args: &[OsString],
        cwd: &Path,
        values: &[(OsString, OsString)],
        disc: &File,
        output: &File,
    ) -> io::Result<Self> {
        Self::spawn_inner(executable, args, cwd, values, disc, output, false)
    }
    fn spawn_inner(
        executable: &Path,
        args: &[OsString],
        cwd: &Path,
        values: &[(OsString, OsString)],
        disc: &File,
        output: &File,
        managed: bool,
    ) -> io::Result<Self> {
        if !executable.is_absolute() || !cwd.is_absolute() {
            return Err(invalid(
                "native executable and working directory must be absolute",
            ));
        }
        if !disc.metadata()?.is_file() || !output.metadata()?.is_file() {
            return Err(invalid("disc and output must be regular files"));
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
            return Err(invalid("native command line is too long"));
        }
        let source = duplicate(disc.as_raw_handle(), FILE_GENERIC_READ)?;
        let log = duplicate(output.as_raw_handle(), FILE_GENERIC_WRITE)?;
        let nul = File::open("NUL")?;
        let input = duplicate(nul.as_raw_handle(), FILE_GENERIC_READ)?;
        let restart = if managed {
            // SAFETY: private, noninheritable manual-reset event, initially clear.
            let event = unsafe { CreateEventW(null(), 1, 0, null()) };
            if event.is_null() {
                return Err(io::Error::last_os_error());
            }
            Some(unsafe { OwnedHandle::from_raw_handle(event) })
        } else {
            None
        };
        let inherited_restart = restart
            .as_ref()
            .map(|event| duplicate(event.as_raw_handle(), EVENT_MODIFY_STATE))
            .transpose()?;
        let mut handles = vec![
            source.as_raw_handle(),
            log.as_raw_handle(),
            input.as_raw_handle(),
        ];
        if let Some(event) = &inherited_restart {
            handles.push(event.as_raw_handle());
        }
        let attributes = Attributes::new(&mut handles)?;
        let env = environment(
            values,
            source.as_raw_handle(),
            inherited_restart.as_ref().map(AsRawHandle::as_raw_handle),
        )?;
        // SAFETY: no security attributes/name: the job is private, noninheritable.
        let raw = unsafe { CreateJobObjectW(null(), null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // No new memory/time policy is introduced by process ownership.
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
        startup.StartupInfo.hStdOutput = log.as_raw_handle();
        startup.StartupInfo.hStdError = log.as_raw_handle();
        startup.lpAttributeList = attributes.ptr;
        let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
        // SAFETY: every UTF-16 buffer is terminated, command is writable, the
        // attribute list and all handle owners outlive this synchronous call.
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
        // The suspended child cannot execute game code before assignment.
        if unsafe { AssignProcessToJobObject(raw, process.as_raw_handle()) } == 0 {
            let error = io::Error::last_os_error();
            unsafe {
                TerminateProcess(process.as_raw_handle(), 1);
                WaitForSingleObject(process.as_raw_handle(), 5000);
            }
            return Err(error);
        }
        let owner = Self {
            process,
            job,
            restart,
        };
        if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        Ok(owner)
    }
    pub fn poll(&self) -> io::Result<Option<u32>> {
        // Wait, rather than interpreting STILL_ACTIVE (259), since 259 is also
        // a possible real exit code.
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(Some(code))
            }
            _ => Err(io::Error::last_os_error()),
        }
    }
    fn restart_requested(&self) -> io::Result<bool> {
        let Some(event) = &self.restart else {
            return Ok(false);
        };
        match unsafe { WaitForSingleObject(event.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
    }
    pub fn stop(&self) -> io::Result<()> {
        if unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { WaitForSingleObject(self.process.as_raw_handle(), 5000) } != WAIT_OBJECT_0 {
            return Err(io::Error::other("native game did not stop"));
        }
        let began = Instant::now();
        loop {
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
            // SAFETY: the live job and correctly sized output remain valid.
            if unsafe {
                QueryInformationJobObject(
                    self.job.as_raw_handle(),
                    JobObjectBasicAccountingInformation,
                    (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if info.ActiveProcesses == 0 {
                return Ok(());
            }
            if began.elapsed() >= Duration::from_secs(5) {
                return Err(io::Error::other("native game descendants did not stop"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Exact launch inputs retained for the whole session. In particular, the disc
/// is an owned open file, not a filename reopened between runs.
pub struct Launch {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub disc: File,
    pub output: File,
}
impl Launch {
    fn spawn(&self) -> io::Result<Process> {
        Process::spawn_inner(
            &self.executable,
            &self.args,
            &self.cwd,
            &self.environment,
            &self.disc,
            &self.output,
            true,
        )
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Restarted { count: u64 },
    Exited(u32),
}
/// One tracked session across ordinary upstream shutdowns and managed restarts.
/// Restart notification lets the app reset window/readiness observations while
/// its play-time clock and save-directory ownership continue uninterrupted.
pub struct Session {
    process: Option<Process>,
    launch: Launch,
    exit: Option<u32>,
    failure: Option<(io::ErrorKind, String)>,
    restarts: u64,
}
impl Session {
    pub fn start(launch: Launch) -> io::Result<Self> {
        let process = launch.spawn()?;
        Ok(Self {
            launch,
            process: Some(process),
            exit: None,
            failure: None,
            restarts: 0,
        })
    }
    pub fn poll(&mut self) -> io::Result<Option<Event>> {
        if let Some((kind, message)) = &self.failure {
            return Err(io::Error::new(*kind, message.clone()));
        }
        if let Some(code) = self.exit {
            return Ok(Some(Event::Exited(code)));
        }
        let Some(code) = self
            .process
            .as_ref()
            .expect("live session process")
            .poll()?
        else {
            return Ok(None);
        };
        let previous = self.process.take().expect("exited session process");
        let result: io::Result<Option<Event>> = (|| {
            // Wait for every descendant too before another run can acquire the
            // saves. Stop/relaunch failures are terminal, with no hidden retry.
            let restart = previous.restart_requested()?;
            previous.stop()?;
            drop(previous);
            if code == 0 && restart {
                let process = self.launch.spawn()?;
                self.process = Some(process);
                self.restarts += 1;
                Ok(Some(Event::Restarted {
                    count: self.restarts,
                }))
            } else {
                self.exit = Some(code);
                Ok(Some(Event::Exited(code)))
            }
        })();
        if let Err(error) = &result {
            self.failure = Some((error.kind(), error.to_string()));
        }
        result
    }
    pub fn stop(&mut self) -> io::Result<()> {
        if let Some(process) = self.process.take() {
            if let Err(error) = process.stop() {
                self.failure = Some((error.kind(), error.to_string()));
                return Err(error);
            }
            self.exit = Some(1);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Seek, SeekFrom, Write},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CompareObjectHandles, GetHandleInformation, HANDLE_FLAG_INHERIT},
        Security::SECURITY_ATTRIBUTES,
        System::Threading::{
            CreateEventW, OpenEventW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, SYNCHRONIZATION_SYNCHRONIZE,
        },
    };

    #[test]
    fn child() {
        let Ok(mode) = std::env::var("GP_PROCESS_TEST_CHILD") else {
            return;
        };
        let raw: usize = std::env::var("GAMEPKG_TAKEDAWN_DISC_HANDLE")
            .unwrap()
            .parse()
            .unwrap();
        // This fixture owns the handle inherited into its own child process.
        let mut disc = unsafe { File::from_raw_handle(raw as HANDLE) };
        let mut flags = 0;
        assert_ne!(
            unsafe { GetHandleInformation(disc.as_raw_handle(), &mut flags) },
            0
        );
        assert_ne!(flags & HANDLE_FLAG_INHERIT, 0);
        disc.seek(SeekFrom::Start(0)).unwrap();
        let mut contents = String::new();
        disc.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "selected ISO fixture");
        assert_eq!(
            disc.write(b"mutation").unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        let unrelated: usize = std::env::var("GP_UNRELATED_HANDLE")
            .unwrap()
            .parse()
            .unwrap();
        // Capture any object currently occupying that numeric slot before
        // OpenEventW itself can allocate/reuse the slot for the named event.
        let candidate = duplicate(unrelated as HANDLE, 0).ok();
        let event_name = wide(&std::env::var_os("GP_EVENT_NAME").unwrap()).unwrap();
        let event = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, event_name.as_ptr()) };
        assert!(!event.is_null());
        let event = unsafe { OwnedHandle::from_raw_handle(event) };
        if let Some(candidate) = candidate {
            assert_eq!(
                unsafe { CompareObjectHandles(candidate.as_raw_handle(), event.as_raw_handle()) },
                0
            );
        }
        let expected = [
            "with spaces",
            "quote\"inside",
            "trailing\\",
            "two\\\\\"quotes",
            "caf\u{e9}",
        ];
        let args: Vec<_> = std::env::args().collect();
        for value in expected {
            assert!(
                args.iter().any(|arg| arg == value),
                "lost argument {value:?}"
            );
        }
        println!(
            "CHILD READY: original ISO read-only; unrelated handle excluded; arguments intact"
        );
        io::stdout().flush().unwrap();
        if matches!(mode.as_str(), "restart" | "request" | "request_error") {
            let handle: usize = std::env::var("GAMEPKG_TAKEDAWN_RESTART_HANDLE")
                .unwrap()
                .parse()
                .unwrap();
            let request = || {
                assert_ne!(
                    unsafe { windows_sys::Win32::System::Threading::SetEvent(handle as HANDLE) },
                    0
                );
            };
            if mode == "request" || mode == "request_error" {
                request();
                std::process::exit(if mode == "request_error" { 23 } else { 0 });
            }
            let marker = std::env::var_os("GP_RESTART_MARKER").unwrap();
            match std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(marker)
            {
                Ok(mut file) => {
                    file.write_all(b"first run finished").unwrap();
                    drop(file);
                    request();
                    std::process::exit(0);
                }
                Err(error) => assert_eq!(error.kind(), io::ErrorKind::AlreadyExists),
            }
            std::process::exit(0);
        }
        if mode == "wait" {
            let mut grandchild = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "tests::grandchild", "--nocapture"])
                .env("GP_PROCESS_TEST_GRANDCHILD", "1")
                .spawn()
                .unwrap();
            println!("GRANDCHILD_PID={}", grandchild.id());
            io::stdout().flush().unwrap();
            grandchild.wait().unwrap();
        }
        std::process::exit(259);
    }
    #[test]
    fn grandchild() {
        if std::env::var_os("GP_PROCESS_TEST_GRANDCHILD").is_some() {
            std::thread::sleep(Duration::from_secs(30));
        }
    }
    struct Fixture {
        _folder: tempfile::TempDir,
        exe: std::path::PathBuf,
        disc: File,
        log: File,
        event: OwnedHandle,
        values: Vec<(OsString, OsString)>,
    }
    impl Fixture {
        fn new(mode: &str) -> Self {
            let folder = tempfile::tempdir().unwrap();
            let exe = folder.path().join("native runner caf\u{e9} fixture.exe");
            std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
            let mut disc = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(folder.path().join("selected.iso"))
                .unwrap();
            disc.write_all(b"selected ISO fixture").unwrap();
            disc.seek(SeekFrom::Start(0)).unwrap();
            let log = tempfile::tempfile().unwrap();
            let name = OsString::from(format!(
                "Local\\GamePKG-runner-{}-{}",
                std::process::id(),
                folder.path().file_name().unwrap().to_string_lossy()
            ));
            let wname = wide(&name).unwrap();
            let security = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: null_mut(),
                bInheritHandle: 1,
            };
            let event = unsafe { CreateEventW(&security, 1, 0, wname.as_ptr()) };
            assert!(!event.is_null());
            let event = unsafe { OwnedHandle::from_raw_handle(event) };
            let mut values = vec![
                ("GP_PROCESS_TEST_CHILD".into(), mode.into()),
                ("GP_EVENT_NAME".into(), name),
                (
                    "GP_UNRELATED_HANDLE".into(),
                    (event.as_raw_handle() as usize).to_string().into(),
                ),
            ];
            for key in ["SystemRoot", "TEMP", "TMP"] {
                if let Some(value) = std::env::var_os(key) {
                    values.push((key.into(), value));
                }
            }
            Self {
                _folder: folder,
                exe,
                disc,
                log,
                event,
                values,
            }
        }
        fn spawn(&self) -> Process {
            let mut args: Vec<OsString> = ["--exact", "tests::child", "--nocapture"]
                .into_iter()
                .map(Into::into)
                .collect();
            for value in [
                "with spaces",
                "quote\"inside",
                "trailing\\",
                "two\\\\\"quotes",
                "caf\u{e9}",
            ] {
                args.push("--skip".into());
                args.push(value.into());
            }
            Process::spawn(
                &self.exe,
                &args,
                self._folder.path(),
                &self.values,
                &self.disc,
                &self.log,
            )
            .unwrap()
        }
        fn log(&mut self) -> String {
            self.log.seek(SeekFrom::Start(0)).unwrap();
            let mut out = String::new();
            self.log.read_to_string(&mut out).unwrap();
            out
        }
    }
    fn flags(file: &impl AsRawHandle) -> u32 {
        let mut flags = 0;
        assert_ne!(
            unsafe { GetHandleInformation(file.as_raw_handle(), &mut flags) },
            0
        );
        flags
    }
    #[test]
    fn real_child_gets_only_selected_handles_and_preserves_exit_259() {
        let mut fixture = Fixture::new("exit");
        let old = flags(&fixture.disc);
        assert_ne!(flags(&fixture.event) & HANDLE_FLAG_INHERIT, 0);
        let process = fixture.spawn();
        let began = Instant::now();
        let code = loop {
            if let Some(code) = process.poll().unwrap() {
                break code;
            }
            assert!(
                began.elapsed() < Duration::from_secs(15),
                "{}",
                fixture.log()
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(code, 259, "{}", fixture.log());
        assert!(fixture.log().contains("CHILD READY"));
        assert_eq!(flags(&fixture.disc), old);
        drop(process);
        fixture.disc.seek(SeekFrom::Start(0)).unwrap();
        fixture.disc.write_all(b"caller retains original").unwrap();
        assert_ne!(flags(&fixture.event) & HANDLE_FLAG_INHERIT, 0);
    }
    #[test]
    fn dropping_owner_stops_the_running_child() {
        let mut fixture = Fixture::new("wait");
        let process = fixture.spawn();
        let observed = duplicate(process.process.as_raw_handle(), 0).unwrap();
        let began = Instant::now();
        while !fixture.log().contains("GRANDCHILD_PID=") {
            assert!(process.poll().unwrap().is_none(), "{}", fixture.log());
            assert!(began.elapsed() < Duration::from_secs(15));
            std::thread::sleep(Duration::from_millis(20));
        }
        let log = fixture.log();
        let grandchild_pid: u32 = log
            .lines()
            .find_map(|line| line.strip_prefix("GRANDCHILD_PID="))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let raw = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                grandchild_pid,
            )
        };
        assert!(!raw.is_null());
        let grandchild = unsafe { OwnedHandle::from_raw_handle(raw) };
        assert_eq!(
            unsafe { WaitForSingleObject(grandchild.as_raw_handle(), 0) },
            WAIT_TIMEOUT
        );
        drop(process);
        assert_eq!(
            unsafe { WaitForSingleObject(grandchild.as_raw_handle(), 5000) },
            WAIT_OBJECT_0
        );
        assert_eq!(
            unsafe { WaitForSingleObject(observed.as_raw_handle(), 5000) },
            WAIT_OBJECT_0
        );
        let mut code = 0;
        assert_ne!(
            unsafe { GetExitCodeProcess(observed.as_raw_handle(), &mut code) },
            0
        );
        assert_eq!(code, 1);
    }
    impl Fixture {
        fn launch(&self) -> Launch {
            let mut args: Vec<OsString> = ["--exact", "tests::child", "--nocapture"]
                .into_iter()
                .map(Into::into)
                .collect();
            for value in [
                "with spaces",
                "quote\"inside",
                "trailing\\",
                "two\\\\\"quotes",
                "caf\u{e9}",
            ] {
                args.push("--skip".into());
                args.push(value.into());
            }
            let mut environment = self.values.clone();
            environment.push((
                "GP_RESTART_MARKER".into(),
                self._folder.path().join("restart.marker").into_os_string(),
            ));
            Launch {
                executable: self.exe.clone(),
                args,
                cwd: self._folder.path().into(),
                environment,
                disc: self.disc.try_clone().unwrap(),
                output: self.log.try_clone().unwrap(),
            }
        }
    }
    #[test]
    fn managed_restart_retains_the_open_disc_and_one_session() {
        let mut fixture = Fixture::new("restart");
        let path = fixture._folder.path().join("selected.iso");
        std::fs::rename(&path, fixture._folder.path().join("held-original.iso")).unwrap();
        std::fs::write(&path, b"replacement must never be reopened").unwrap();
        let mut session = Session::start(fixture.launch()).unwrap();
        let began = Instant::now();
        let mut restarted = false;
        loop {
            match session.poll().unwrap() {
                Some(Event::Restarted { count: 1 }) => {
                    assert!(!restarted);
                    restarted = true;
                }
                Some(Event::Exited(0)) => break,
                Some(event) => panic!("unexpected event {event:?}: {}", fixture.log()),
                None => {
                    assert!(
                        began.elapsed() < Duration::from_secs(20),
                        "{}",
                        fixture.log()
                    );
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        assert!(restarted);
        assert_eq!(fixture.log().matches("CHILD READY").count(), 2);
        assert_eq!(session.poll().unwrap(), Some(Event::Exited(0)));
        assert_eq!(
            std::fs::read(path).unwrap(),
            b"replacement must never be reopened"
        );
    }
    #[test]
    fn failed_restart_is_terminal_and_does_not_retry_or_panic() {
        let fixture = Fixture::new("request");
        let mut session = Session::start(fixture.launch()).unwrap();
        // Inject the missing executable a real removal/cache failure presents.
        session.launch.executable = fixture._folder.path().join("missing.exe");
        let began = Instant::now();
        let error = loop {
            match session.poll() {
                Err(error) => break error,
                Ok(None) => {
                    assert!(began.elapsed() < Duration::from_secs(15));
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(event) => panic!("unexpected restart result {event:?}"),
            }
        };
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(session.process.is_none());
        assert_eq!(session.poll().unwrap_err().to_string(), error.to_string());
    }
    #[test]
    fn an_error_exit_after_a_restart_signal_is_reported_without_relaunch() {
        let fixture = Fixture::new("request_error");
        let mut session = Session::start(fixture.launch()).unwrap();
        let began = Instant::now();
        loop {
            match session.poll().unwrap() {
                Some(Event::Exited(23)) => break,
                Some(event) => panic!("unexpected restart after error: {event:?}"),
                None => {
                    assert!(began.elapsed() < Duration::from_secs(15));
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        assert_eq!(session.restarts, 0);
    }
    #[test]
    #[ignore = "requires the pinned original native restart fixture"]
    fn actual_upstream_restart_teardown() {
        let executable = PathBuf::from(
            std::env::var_os("GAMEPKG_RESTART_FIXTURE").expect("native fixture executable"),
        );
        let folder = tempfile::tempdir().unwrap();
        let marker = folder.path().join("marker.txt");
        let disc = tempfile::tempfile().unwrap();
        let mut output = tempfile::tempfile().unwrap();
        let mut environment = Vec::new();
        for key in ["SystemRoot", "TEMP", "TMP"] {
            if let Some(value) = std::env::var_os(key) {
                environment.push((key.into(), value));
            }
        }
        let mut session = Session::start(Launch {
            executable,
            args: vec!["once".into(), marker.clone().into_os_string()],
            cwd: folder.path().into(),
            environment,
            disc,
            output: output.try_clone().unwrap(),
        })
        .unwrap();
        let began = Instant::now();
        let mut restarts = 0;
        loop {
            match session.poll().unwrap() {
                Some(Event::Restarted { count: 1 }) => restarts += 1,
                Some(Event::Exited(0)) => break,
                Some(event) => panic!("unexpected original lifecycle event: {event:?}"),
                None => {
                    assert!(began.elapsed() < Duration::from_secs(20));
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        assert_eq!(restarts, 1);
        assert_eq!(
            std::fs::read_to_string(marker).unwrap(),
            "teardown complete\nfinal cleanup complete\nteardown complete\nfinal cleanup complete\n"
        );
        output.seek(SeekFrom::Start(0)).unwrap();
        let mut log = String::new();
        output.read_to_string(&mut log).unwrap();
        assert_eq!(
            log.matches("FIXTURE: normal save/audio/window teardown completed")
                .count(),
            2
        );
        assert_eq!(
            log.matches("FIXTURE: restarted after completed teardown")
                .count(),
            1
        );
        assert_eq!(
            log.matches("GamePKG: managed restart requested; normal exit pending.")
                .count(),
            1
        );
    }
    #[test]
    fn rejects_ambiguous_environment_and_embedded_nuls() {
        assert!(wide(OsStr::new("bad\0argument")).is_err());
        for values in [
            vec![("Path".into(), "a".into()), ("PATH".into(), "b".into())],
            vec![("GAMEPKG_TAKEDAWN_DISC_HANDLE".into(), "12".into())],
            vec![("bad=name".into(), "x".into())],
            vec![("GAMEPKG_TAKEDAWN_MANAGED_RESTART".into(), "1".into())],
            vec![("GAMEPKG_TAKEDAWN_RESTART_HANDLE".into(), "123".into())],
            vec![("".into(), "x".into())],
        ] {
            assert!(environment(&values, 12usize as HANDLE, None).is_err());
        }
        let mut quoted_empty = Vec::new();
        quoted(OsStr::new(""), &mut quoted_empty).unwrap();
        assert_eq!(quoted_empty, [34, 34]);
    }
}
