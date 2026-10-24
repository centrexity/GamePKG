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
}
impl Default for Options {
    fn default() -> Self {
        Self {
            headless: false,
            mute_audio: false,
            audio_volume: 100,
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
            admitted: false,
            window: false,
            frames: 0,
            headless: options.headless,
            started: false,
            finished: false,
            exit: None,
        })
    }
    fn observe(&mut self) -> Result<()> {
        let mut bytes = [0u8; 8192];
        // Read through an independently opened file handle, so inspection never
        // seeks the game's stdout handle or affects its shared restart position.
        for _ in 0..128 {
            let n = self.log.as_file_mut().read(&mut bytes)?;
            if n == 0 {
                break;
            }
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
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.admitted
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
        self.observe()?;
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
    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut owner) = self.owner.take() {
            owner.stop()?;
        }
        self.history.finish()?;
        self.finished = true;
        self.data_lock.take();
        Ok(())
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
        // This std-only host fixture tests app ownership, not Vulkan rendering
        // or original game fidelity. Actual game checks use the prepared kit.
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("fixture.rs");
        fs::write(
            &source,
            r#"
use std::{fs, io::Read, os::windows::io::FromRawHandle};
#[link(name="kernel32")]
extern "system" { fn SetEvent(handle: *mut core::ffi::c_void) -> i32; }
fn main() {
    assert_eq!(std::env::args().nth(1).unwrap(), "/gamepkg-disc/default.xbe");
    assert_eq!(std::env::var("RECOMP_AUDIO_MUTE").unwrap(), "1");
    assert_eq!(std::env::var("RECOMP_AUDIO_VOLUME").unwrap(), "0.37");
    let raw: usize = std::env::var("GAMEPKG_TAKEDAWN_DISC_HANDLE").unwrap().parse().unwrap();
    let mut disc = unsafe { fs::File::from_raw_handle(raw as _) };
    // Positional I/O preserves the selected capability's offset across restart.
    use std::os::windows::fs::FileExt;
    let mut bytes = [0; 4];
    assert_eq!(disc.seek_read(&mut bytes, 0).unwrap(), 4);
    assert_eq!(&bytes, b"ISO!");
    let saves = std::path::PathBuf::from(std::env::var_os("RECOMP_SAVE_DIR").unwrap());
    let profile = saves.join("profile");
    println!("GamePKG: verified disc admitted in place");
    println!("[PRESENT] window up: app lifecycle fixture");
    std::thread::sleep(std::time::Duration::from_millis(150));
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
        drop(session);
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
