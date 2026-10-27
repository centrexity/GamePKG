//! Native Windows player ownership: selected-disc capability, per-game saves,
//! AppContainer, original managed restarts and host-owned play history.
use anyhow::{ensure, Context, Result};
use gamepkg_takedawn_windows_runner::{Event, Launch, Sandbox, Session as Owner};
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::Read,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        process::ExitStatusExt,
    },
    path::Path,
    process::ExitStatus,
};

#[derive(Clone, Copy)]
pub struct Options {
    pub headless: bool,
    pub mute_audio: bool,
    pub audio_volume: u8,
    pub watchdog_seconds: u16,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            headless: false,
            mute_audio: false,
            audio_volume: 100,
            watchdog_seconds: 0,
        }
    }
}
fn directory(path: &Path) -> Result<()> {
    fs::create_dir(path).or_else(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            Ok(())
        } else {
            Err(e)
        }
    })?;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(0x0220_0000)
        .open(path)?;
    ensure!(
        file.metadata()?.is_dir() && file.metadata()?.file_attributes() & 0x400 == 0,
        "game data must be a real directory"
    );
    Ok(())
}
fn check_tree(path: &Path) -> Result<()> {
    let mut pending = vec![path.to_owned()];
    let mut count = 0usize;
    while let Some(path) = pending.pop() {
        count += 1;
        ensure!(
            count <= 100_000,
            "game save inventory is too large to validate"
        );
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "game data cannot contain reparse points"
        );
        if metadata.is_dir() {
            for item in fs::read_dir(path)? {
                pending.push(item?.path());
            }
        } else {
            ensure!(metadata.is_file(), "game data must contain ordinary files");
        }
    }
    Ok(())
}
pub struct Session {
    owner: Option<Owner>,
    _binary: tempfile::TempDir,
    log: tempfile::NamedTempFile,
    history: crate::progress::Session,
    data_lock: Option<File>,
    startup: Vec<u8>,
    recent: VecDeque<u8>,
    overlap: Vec<u8>,
    path_audit: crate::takedawn::PathAudit,
    admitted: bool,
    window: bool,
    frames: u32,
    headless: bool,
    started: bool,
    finished: bool,
    exit: Option<ExitStatus>,
}
impl Session {
    pub fn start_cached(
        root: &Path,
        id: &str,
        creator: &str,
        schema: u32,
        executable: &Path,
        iso: &Path,
        options: Options,
    ) -> Result<Self> {
        ensure!(
            cfg!(target_arch = "x86_64"),
            "the native Takedawn player requires 64-bit Windows"
        );
        ensure!(options.audio_volume <= 100, "invalid game audio volume");
        ensure!(
            !id.is_empty()
                && id.len() <= 200
                && id != "."
                && id != ".."
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
                && !creator.is_empty()
                && creator.len() <= 256,
            "invalid native game identity"
        );
        let bytes = crate::files::read_regular(executable, 64 * 1048576, false)
            .context("build the reviewed native game before playing")?;
        crate::integrity::verify_build(executable, &bytes)?;
        crate::native::validate_executable(&bytes)?;
        let companion = executable.with_file_name("SDL2.dll");
        let dll = crate::files::read_regular(&companion, 32 * 1048576, false)?;
        crate::integrity::verify_build(&companion, &dll)?;
        let disc = crate::files::open_regular(iso, 16 * 1024 * 1024 * 1024, true)?;
        ensure!(
            disc.metadata()?.len() >= 33 * 2048,
            "select a supported Xbox ISO"
        );
        directory(root)?;
        directory(&root.join("takedawn-data"))?;
        let data = crate::takedawn::data_path(root, id, creator, schema);
        directory(&data)?;
        let locks = root.join("takedawn-session-locks");
        directory(&locks)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .custom_flags(0x0020_0000)
            .open(locks.join(data.file_name().context("invalid save identity")?))?;
        ensure!(
            lock.metadata()?.is_file() && lock.metadata()?.file_attributes() & 0x400 == 0,
            "invalid game session lock"
        );
        lock.try_lock().map_err(|e| {
            anyhow::anyhow!("This game is already running or its save folder is busy: {e}")
        })?;
        for name in ["saves", "shader-cache", "temp"] {
            directory(&data.join(name))?;
        }
        check_tree(&data)?;
        let data = data.canonicalize()?;
        let binary = tempfile::Builder::new()
            .prefix("gamepkg-native-")
            .tempdir()?;
        let engine = binary.path().join("game.exe");
        fs::write(&engine, &bytes)?;
        fs::write(binary.path().join("SDL2.dll"), &dll)?;
        let identity = crate::verify::digest(data.as_os_str().as_encoded_bytes());
        let sandbox = Sandbox::open(&format!("GamePKG.{}", &identity[..56]))?;
        sandbox.grant_runtime(binary.path())?;
        sandbox.grant_saves(&data)?;
        let log = tempfile::NamedTempFile::new()?;
        let mut environment = vec![
            (
                "LOCALAPPDATA".into(),
                std::env::var_os("LOCALAPPDATA")
                    .context("missing Windows local application data")?,
            ),
            (
                "SystemRoot".into(),
                std::env::var_os("SystemRoot").context("missing Windows system root")?,
            ),
            ("TEMP".into(), data.join("temp").into_os_string()),
            ("TMP".into(), data.join("temp").into_os_string()),
            (
                "RECOMP_SAVE_DIR".into(),
                data.join("saves").into_os_string(),
            ),
            (
                "RECOMP_RENDER_SHADER_CACHE".into(),
                data.join("shader-cache").into_os_string(),
            ),
            ("RECOMP_PB_BACKEND".into(), "vulkan".into()),
            ("RECOMP_UNIMPL_TRAP".into(), "1".into()),
            ("RECOMP_STUB_LOG".into(), "1".into()),
            (
                "RECOMP_HANG_WATCHDOG".into(),
                options.watchdog_seconds.to_string().into(),
            ),
            (
                "RECOMP_AUDIO_MUTE".into(),
                if options.mute_audio { "1" } else { "0" }.into(),
            ),
            (
                "RECOMP_AUDIO_VOLUME".into(),
                format!("{:.2}", f64::from(options.audio_volume) / 100.0).into(),
            ),
        ];
        let mut args = Vec::new();
        if options.headless {
            args.push("--headless".into());
            environment.extend([
                ("RECOMP_HEADLESS".into(), "1".into()),
                ("SDL_AUDIODRIVER".into(), "dummy".into()),
                ("RECOMP_FLIP_MARK".into(), "1".into()),
            ]);
        }
        args.push("/gamepkg-disc/default.xbe".into());
        let history = crate::progress::Session::open(root, id, creator)?;
        let owner = Owner::start_confined(
            Launch {
                executable: engine,
                args,
                cwd: data,
                environment,
                disc,
                output: log.reopen()?,
                memory_limit_bytes: Some(crate::takedawn::MEMORY),
            },
            sandbox,
        )
        .context("unable to launch the native Windows game container")?;
        Ok(Self {
            owner: Some(owner),
            _binary: binary,
            log,
            history,
            data_lock: Some(lock),
            startup: Vec::new(),
            recent: VecDeque::new(),
            overlap: Vec::new(),
            path_audit: crate::takedawn::PathAudit::default(),
            admitted: false,
            window: false,
            frames: 0,
            headless: options.headless,
            started: false,
            finished: false,
            exit: None,
        })
    }
    fn observe(&mut self) -> Result<bool> {
        let mut bytes = [0u8; 8192];
        // Read through an independently opened file handle, so inspection never
        // seeks the game's stdout handle or affects its shared restart position.
        for _ in 0..128 {
            let n = self.log.as_file_mut().read(&mut bytes)?;
            if n == 0 {
                return Ok(false);
            }
            self.path_audit.observe(&bytes[..n]);
            self.overlap.extend_from_slice(&bytes[..n]);
            let text = String::from_utf8_lossy(&self.overlap);
            self.admitted |= text.contains("verified disc admitted in place");
            self.window |= text.contains("[PRESENT] window up:");
            for (_, rest) in text
                .match_indices("[GPU] flip ")
                .map(|(n, _)| (n, &text[n + 11..]))
            {
                if let Ok(frame) = rest
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                {
                    self.frames = self.frames.max(frame);
                }
            }
            self.overlap.drain(..self.overlap.len().saturating_sub(64));
            let keep = n.min((16 * 1024usize).saturating_sub(self.startup.len()));
            self.startup.extend_from_slice(&bytes[..keep]);
            self.recent.extend(&bytes[keep..n]);
            self.recent
                .drain(..self.recent.len().saturating_sub(112 * 1024));
        }
        Ok(true)
    }
    pub fn ready(&self) -> bool {
        self.path_audit.failure().is_none()
            && self.admitted
            && if self.headless {
                self.frames > 0
            } else {
                self.window
            }
    }
    pub fn disc_admitted(&self) -> bool {
        self.admitted
    }
    pub fn frame_count(&self) -> u32 {
        self.frames
    }
    pub fn diagnostics(&self) -> String {
        let mut bytes = self.startup.clone();
        bytes.extend_from_slice(b"\n[GamePKG recent runtime output]\n");
        bytes.extend(&self.recent);
        String::from_utf8_lossy(&bytes).into_owned()
    }
    pub fn poll(&mut self) -> Result<Option<ExitStatus>> {
        self.observe()?;
        self.check_path()?;
        let event = self
            .owner
            .as_mut()
            .context("native session has stopped")?
            .poll()?;
        if matches!(event, Some(Event::Restarted { .. })) {
            self.admitted = false;
            self.window = false;
            self.frames = 0;
            self.overlap.clear();
        }
        let more = self.observe()?;
        if matches!(event, Some(Event::Exited(_))) && more {
            while self.observe()? {}
        }
        self.check_path()?;
        if !self.started && self.ready() {
            self.history.start();
            self.started = true;
        }
        if let Some(Event::Exited(code)) = event {
            if !self.finished {
                self.history.finish()?;
                self.finished = true;
                self.data_lock.take();
            }
            self.exit = Some(ExitStatus::from_raw(code));
        } else if self.started && !self.finished {
            self.history.tick(&[])?;
        }
        Ok(self.exit)
    }
    fn check_path(&mut self) -> Result<()> {
        if self.path_audit.failure().is_some() {
            self.stop()?;
        }
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        let cleanup: Result<()> = self
            .owner
            .take()
            .map_or(Ok(()), |mut owner| owner.stop().map_err(Into::into));
        // With the job stopped the log is finite. Audit its full remainder,
        // including a fault written immediately before a requested close.
        while self.observe()? {}
        let history = self.history.finish();
        self.finished = true;
        self.data_lock.take();
        if let Some(reason) = self.path_audit.failure() {
            anyhow::bail!(
                "This native build reached {reason}. The game was stopped. Update its game package.\n{}{}",
                self.diagnostics(),
                cleanup.err().or_else(|| history.err()).map(|e| format!("\nSession cleanup: {e:#}")).unwrap_or_default()
            );
        }
        cleanup.and(history)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn native_player_owns_saves_history_and_companions_across_restart() {
        // This std-only host fixture tests ownership and real Win32 window
        // access under the production confinement. It does not test Vulkan
        // rendering or original game fidelity; those use the prepared kit.
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("fixture.rs");
        fs::write(
            &source,
            r#"
use std::{fs, os::windows::io::FromRawHandle};
#[link(name="kernel32")]
extern "system" {
    fn SetEvent(handle: *mut core::ffi::c_void) -> i32;
    fn GetModuleHandleW(name: *const u16) -> *mut core::ffi::c_void;
}
#[repr(C)]
struct Message {
    window: *mut core::ffi::c_void,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    x: i32,
    y: i32,
    private: u32,
}
#[link(name="user32")]
extern "system" {
    fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32,
        x: i32, y: i32, width: i32, height: i32, parent: *mut core::ffi::c_void,
        menu: *mut core::ffi::c_void, instance: *mut core::ffi::c_void,
        parameter: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    fn IsWindowVisible(window: *mut core::ffi::c_void) -> i32;
    fn UpdateWindow(window: *mut core::ffi::c_void) -> i32;
    fn DestroyWindow(window: *mut core::ffi::c_void) -> i32;
    fn PeekMessageW(message: *mut Message, window: *mut core::ffi::c_void,
        first: u32, last: u32, remove: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
}
fn window() -> *mut core::ffi::c_void {
    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    let title: Vec<u16> = "GamePKG confined native window fixture\0".encode_utf16().collect();
    // WS_VISIBLE | WS_OVERLAPPEDWINDOW, using the built-in STATIC class.
    let window = unsafe { CreateWindowExW(0, class.as_ptr(), title.as_ptr(),
        0x10cf0000, 0, 0, 320, 240, std::ptr::null_mut(), std::ptr::null_mut(),
        GetModuleHandleW(std::ptr::null()), std::ptr::null_mut()) };
    assert!(!window.is_null(), "CreateWindowExW: {}", std::io::Error::last_os_error());
    assert_ne!(unsafe { IsWindowVisible(window) }, 0);
    unsafe { UpdateWindow(window); }
    window
}
fn pump_window() {
    let mut message: Message = unsafe { std::mem::zeroed() };
    while unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, 1) } != 0 {
        unsafe { TranslateMessage(&message); DispatchMessageW(&message); }
    }
}
fn main() {
    assert_eq!(std::env::args().nth(1).unwrap(), "/gamepkg-disc/default.xbe");
    assert_eq!(std::env::var("RECOMP_AUDIO_MUTE").unwrap(), "1");
    assert_eq!(std::env::var("RECOMP_AUDIO_VOLUME").unwrap(), "0.37");
    let watchdog = std::env::var("RECOMP_HANG_WATCHDOG").unwrap();
    assert!(watchdog == "0" || watchdog == "17");
    assert_eq!(std::env::var("RECOMP_UNIMPL_TRAP").unwrap(), "1");
    assert_eq!(std::env::var("RECOMP_STUB_LOG").unwrap(), "1");
    let saves = std::path::PathBuf::from(std::env::var_os("RECOMP_SAVE_DIR").unwrap());
    if let Ok(mode) = fs::read_to_string(saves.join("unsupported-mode")) {
        use std::io::Write;
        let mut out = std::io::stdout().lock();
        if mode == "restart" && saves.join("restart-requested").exists() {
            writeln!(out, "verified disc admitted in place\n[PRESENT] window up: restarted fixture").unwrap();
            return;
        }
        if mode == "running" {
            writeln!(out, "verified disc admitted in place\n[PRESENT] window up: fixture").unwrap();
            out.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if mode == "late" { out.write_all(&vec![b'x'; 3*1024*1024]).unwrap(); }
        // Force the marker across the first one-MiB observation budget. The
        // owner then restarts before the second half is observed.
        if mode == "restart" { out.write_all(&vec![b'x';1024*1024-10]).unwrap(); }
        out.write_all(b"[STUB] unresolved 0x12345678\n").unwrap(); out.flush().unwrap();
        if mode == "restart" {
            let raw: usize = std::env::var("GAMEPKG_TAKEDAWN_RESTART_HANDLE").unwrap().parse().unwrap();
            assert_ne!(unsafe { SetEvent(raw as _) }, 0);
            fs::write(saves.join("restart-requested"), b"1").unwrap();
        }
        fs::write(saves.join("output-written"), b"1").unwrap();
        if mode == "running" || mode == "stopping" {
            std::thread::sleep(std::time::Duration::from_secs(5));
            fs::write(saves.join("continued"), b"must not happen").unwrap();
        }
        return;
    }
    println!("watchdog seconds={watchdog}");
    let raw: usize = std::env::var("GAMEPKG_TAKEDAWN_DISC_HANDLE").unwrap().parse().unwrap();
    let disc = unsafe { fs::File::from_raw_handle(raw as _) };
    // Positional I/O preserves the selected capability's offset across restart.
    use std::os::windows::fs::FileExt;
    let mut bytes = [0; 4];
    assert_eq!(disc.seek_read(&mut bytes, 0).unwrap(), 4);
    assert_eq!(&bytes, b"ISO!");
    let saves = std::path::PathBuf::from(std::env::var_os("RECOMP_SAVE_DIR").unwrap());
    let profile = saves.join("profile");
    let window = window();
    println!("fixture Win32 window created and visible");
    println!("GamePKG: verified disc admitted in place");
    println!("[PRESENT] window up: app lifecycle fixture");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(150);
    while std::time::Instant::now() < deadline {
        pump_window();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_ne!(unsafe { DestroyWindow(window) }, 0);
    println!("fixture Win32 window closed");
    if !profile.exists() {
        fs::write(profile, b"original save fixture").unwrap();
        let raw: usize = std::env::var("GAMEPKG_TAKEDAWN_RESTART_HANDLE").unwrap().parse().unwrap();
        assert_ne!(unsafe { SetEvent(raw as _) }, 0);
    } else {
        assert_eq!(fs::read(profile).unwrap(), b"original save fixture");
    }
}
"#,
        )
        .unwrap();
        let executable = root.path().join("fixture.exe");
        let compiled = std::process::Command::new("rustc")
            .arg("--edition=2021")
            .arg("-Dwarnings")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let bytes = fs::read(&executable).unwrap();
        crate::integrity::record_build(&executable, &bytes).unwrap();
        let dll = root.path().join("SDL2.dll");
        fs::write(&dll, b"host companion fixture").unwrap();
        crate::integrity::record_build(&dll, b"host companion fixture").unwrap();
        let iso = root.path().join("selected.iso");
        fs::write(&iso, b"ISO!").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&iso)
            .unwrap()
            .set_len(68 * 1024)
            .unwrap();
        let options = Options {
            mute_audio: true,
            audio_volume: 37,
            watchdog_seconds: 17,
            headless: false,
        };
        let mut session = Session::start_cached(
            root.path(),
            "fixture",
            "creator",
            1,
            &executable,
            &iso,
            options,
        )
        .unwrap();
        assert!(Session::start_cached(
            root.path(),
            "fixture",
            "creator",
            1,
            &executable,
            &iso,
            options
        )
        .is_err());
        let began = Instant::now();
        loop {
            if let Some(status) = session.poll().unwrap() {
                assert!(status.success(), "{}", session.diagnostics());
                assert!(session.ready());
                break;
            }
            assert!(
                began.elapsed() < Duration::from_secs(30),
                "{}",
                session.diagnostics()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let text = session.diagnostics();
        assert_eq!(text.matches("[PRESENT] window up:").count(), 2);
        assert_eq!(text.matches("watchdog seconds=17").count(), 2);
        assert_eq!(
            text.matches("fixture Win32 window created and visible")
                .count(),
            2
        );
        assert_eq!(text.matches("fixture Win32 window closed").count(), 2);
        drop(session);
        // A later launch with the default must explicitly turn the runtime's
        // own default-on detector off, even after an enabled managed restart.
        let mut session = Session::start_cached(
            root.path(),
            "fixture",
            "creator",
            1,
            &executable,
            &iso,
            Options {
                watchdog_seconds: Options::default().watchdog_seconds,
                ..options
            },
        )
        .unwrap();
        let began = Instant::now();
        while session.poll().unwrap().is_none() {
            assert!(began.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(session.diagnostics().contains("watchdog seconds=0"));
        assert!(session
            .diagnostics()
            .contains("fixture Win32 window closed"));
        drop(session);
        println!("Verified real confined Win32 windows: two managed-restart launches and one default-watchdog launch, all visible and closed.");
        let saves =
            crate::takedawn::data_path(root.path(), "fixture", "creator", 1).join("saves/profile");
        assert_eq!(fs::read(saves).unwrap(), b"original save fixture");
        assert!(
            crate::progress::load(root.path(), "fixture", "creator")
                .unwrap()
                .played_ms
                >= 100
        );
        assert_eq!(
            crate::progress::load(root.path(), "fixture", "other")
                .unwrap()
                .played_ms,
            0
        );
        assert_eq!(&fs::read(&iso).unwrap()[..4], b"ISO!");
        for mode in ["immediate", "running", "late", "restart", "stopping"] {
            let id = format!("unsupported-{mode}");
            let saves = crate::takedawn::data_path(root.path(), &id, "creator", 1).join("saves");
            fs::create_dir_all(&saves).unwrap();
            fs::write(saves.join("unsupported-mode"), mode).unwrap();
            let mut session =
                Session::start_cached(root.path(), &id, "creator", 1, &executable, &iso, options)
                    .unwrap();
            let began = Instant::now();
            if mode != "running" {
                let ready = saves.join(if mode == "restart" {
                    "restart-requested"
                } else {
                    "output-written"
                });
                while !ready.exists() {
                    assert!(
                        began.elapsed() < Duration::from_secs(15),
                        "fixture did not publish its log"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if mode == "stopping" {
                let error = session.stop().unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains("an unresolved native game function"),
                    "{error:#}"
                );
            } else {
                loop {
                    match session.poll() {
                        Err(error) => {
                            assert!(
                                error
                                    .to_string()
                                    .contains("an unresolved native game function"),
                                "{error:#}"
                            );
                            break;
                        }
                        Ok(Some(status)) => {
                            panic!("unsupported game reported successful exit: {status}")
                        }
                        Ok(None) => {}
                    }
                    assert!(
                        began.elapsed() < Duration::from_secs(20),
                        "{}",
                        session.diagnostics()
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            assert!(session.finished && session.owner.is_none());
            assert!(session.data_lock.is_none() && !session.ready());
            assert!(!saves.join("continued").exists());
        }
        fs::write(&dll, b"changed companion").unwrap();
        assert!(Session::start_cached(
            root.path(),
            "fixture",
            "creator",
            1,
            &executable,
            &iso,
            options
        )
        .is_err());
    }
}
