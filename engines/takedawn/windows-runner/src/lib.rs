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
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Memory::{GetProcessHeap, HeapAlloc, HeapFree},
        Threading::{
            CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess, GetExitCodeProcess,
            InitializeProcThreadAttributeList, ResumeThread, TerminateProcess,
            UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
            CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
            PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESTDHANDLES,
            STARTUPINFOEXW,
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
fn environment(values: &[(OsString, OsString)], disc: HANDLE) -> io::Result<Vec<u16>> {
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
        if name == "GAMEPKG_TAKEDAWN_DISC_HANDLE" || entries.iter().any(|(k, _)| k == &name) {
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
        let mut handles = [
            source.as_raw_handle(),
            log.as_raw_handle(),
            input.as_raw_handle(),
        ];
        let attributes = Attributes::new(&mut handles)?;
        let env = environment(values, source.as_raw_handle())?;
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
        let owner = Self { process, job };
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
    pub fn stop(&self) -> io::Result<()> {
        if unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { WaitForSingleObject(self.process.as_raw_handle(), 5000) } != WAIT_OBJECT_0 {
            return Err(io::Error::other("native game did not stop"));
        }
        Ok(())
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.stop();
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
            let mut disc = tempfile::tempfile().unwrap();
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
    #[test]
    fn rejects_ambiguous_environment_and_embedded_nuls() {
        assert!(wide(OsStr::new("bad\0argument")).is_err());
        for values in [
            vec![("Path".into(), "a".into()), ("PATH".into(), "b".into())],
            vec![("GAMEPKG_TAKEDAWN_DISC_HANDLE".into(), "12".into())],
            vec![("bad=name".into(), "x".into())],
            vec![("".into(), "x".into())],
        ] {
            assert!(environment(&values, 12usize as HANDLE).is_err());
        }
        let mut quoted_empty = Vec::new();
        quoted(OsStr::new(""), &mut quoted_empty).unwrap();
        assert_eq!(quoted_empty, [34, 34]);
    }
}
