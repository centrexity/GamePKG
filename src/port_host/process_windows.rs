//! Launch the receipt-verified compatibility executable in its own AppContainer.
use super::{
    history::{drain_local_log, read_game_output, record_history, HistoryEvent},
    reaper, Artifact, Bootstrap, NativeLaunch, Permit, Process, Target,
};
use anyhow::{ensure, Context, Result};
use gamepkg_native_windows_runner::{Sandbox, WorkerProcess};
use std::{
    collections::VecDeque,
    ffi::OsString,
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
};

impl Process {
    pub fn start_windows(
        artifact: &Artifact,
        library: &Path,
        roms: Vec<fs::File>,
        options: NativeLaunch,
    ) -> Result<Self> {
        let target = if cfg!(target_arch = "x86") {
            Target::Windows32
        } else {
            Target::Windows64
        };
        ensure!(
            artifact.receipt().bootstrap == Bootstrap::InProcess
                && artifact.receipt().plan.target() == target,
            "native Windows compatibility artifact for this architecture required"
        );
        let manifest = artifact.manifest();
        super::require_windows_compatibility(manifest)?;
        ensure!(
            manifest.required_rom_count() == roms.len(),
            "wrong number of selected ROMs"
        );
        let args = options
            .arguments(manifest)?
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let permit = Permit::acquire()?;
        reaper()?;
        let source = artifact.executable()?;
        let bytes = crate::files::read_regular(&source, 256 * 1024 * 1024, false)?;
        ensure!(
            crate::verify::digest(&bytes) == artifact.receipt().binary,
            "service executable changed before launch"
        );
        let directory = tempfile::tempdir()?;
        let executable = directory.path().join("game.exe");
        fs::write(&executable, bytes)?;
        let sandbox = Sandbox::open(&format!(
            "GamePKG.Compatibility.{}",
            &super::super::owner::identity(manifest)[..32]
        ))?;
        let mut values = Vec::new();
        for key in ["SystemRoot", "LOCALAPPDATA"] {
            let value = std::env::var_os(key)
                .with_context(|| format!("Windows {key} directory unavailable"))?;
            values.push((key.into(), value));
        }
        if manifest.permissions.save {
            let root = library.canonicalize()?;
            let _scope = super::super::owner::save_scope(&root, manifest)?;
            let scope = root
                .join("port-state")
                .join(super::super::owner::identity(manifest));
            sandbox.grant_saves(&scope)?;
            values.push(("GAMEPKG_NATIVE_SAVE_SCOPE".into(), scope.into_os_string()));
            match crate::saves::previous_blob(library, manifest) {
                Ok(Some(blob)) => {
                    let snapshot = directory.path().join("previous-save");
                    fs::create_dir(&snapshot)?;
                    fs::write(snapshot.join("legacy-save.blob"), blob)?;
                }
                Ok(None) => {}
                Err(error) => eprintln!("GamePKG could not read the previous save: {error:#}"),
            }
        }
        sandbox.grant_runtime(directory.path())?;
        let session = crate::progress::Session::open(library, &manifest.id, &manifest.creator_key)?;
        let worker = WorkerProcess::spawn_compatibility(
            &executable,
            &args,
            directory.path(),
            &values,
            &sandbox,
            manifest.limits.hard_memory_bytes,
            &roms,
        )?;
        let (child, output, errors) = worker.into_parts();
        let (ready, readiness) = mpsc::sync_channel(1);
        let achievements = manifest
            .achievements
            .iter()
            .map(|achievement| achievement.id.clone())
            .collect();
        let history = std::thread::Builder::new()
            .name("gamepkg-play-history".into())
            .spawn(move || record_history(session, readiness, achievements))?;
        let frame = Arc::new(AtomicBool::new(false));
        let presented = Arc::clone(&frame);
        let log = Arc::new(Mutex::new(VecDeque::new()));
        let output_log = Arc::clone(&log);
        std::thread::Builder::new()
            .name("gamepkg-native-output".into())
            .spawn(move || {
                read_game_output(
                    output.into_service_io(),
                    &output_log,
                    b"GamePKG presented windows",
                    |event| {
                        if event == HistoryEvent::Presented {
                            presented.store(true, Ordering::Release);
                        }
                        let _ = ready.send(event);
                    },
                );
            })?;
        drain_local_log(errors, Arc::clone(&log));
        Ok(Self {
            child: Some(child),
            _sandbox: sandbox,
            directory: Some(directory),
            log,
            transport: None,
            clock: None,
            history: Some(history),
            game_frame: Some(frame),
            permit: Some(permit),
        })
    }
}
