//! Private native-worker gameplay qualification using public, pinned test data.
use super::Session;
use std::path::Path;

pub(super) fn verify(root: &Path, settings: &crate::settings::Settings, wad: &Path) {
    let bytes = crate::files::read_regular(wad, 64 * 1048576, false).unwrap();
    let hash = crate::verify::digest(&bytes);
    assert_eq!(
        hash, "7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d",
        "expected public Freedoom 0.13.0 phase 1"
    );
    let archive = root.join("com.gamepkg.doom.gamepkg");
    #[cfg(windows)]
    {
        let manifest = crate::registry::installed_manifest(root, "com.gamepkg.doom").unwrap();
        assert!(!super::build_supported(&manifest));
        assert!(!super::play_supported(&manifest));
        let error = crate::runtime::Game::load_target_configured(
            root,
            &manifest.id,
            &[wad.to_path_buf()],
            Some(true),
            settings,
        )
        .err()
        .expect("public Windows C-engine loader unexpectedly admitted the port");
        assert!(
            error
                .to_string()
                .contains("native game isolation is not available"),
            "{error:#}"
        );
    }
    let digest = crate::verify::digest(&std::fs::read(&archive).unwrap());
    let open = || {
        let mut package = crate::archive::Package::open(&archive).unwrap();
        let manifest = crate::verify::verify_embedded(&mut package).unwrap();
        package.resolve_shared_assets(root).unwrap();
        Session::new(
            root,
            &digest,
            package,
            manifest,
            vec![bytes.clone()],
            settings,
            0,
        )
        .unwrap()
    };
    let mut game = open();
    let mut state = 0;
    tick(&mut game, &mut state, 120, 0);
    for _ in 0..4 {
        tick(&mut game, &mut state, 1, 64);
        tick(&mut game, &mut state, 12, 0);
    }
    tick(&mut game, &mut state, 120, 0);
    let initial = position(&game);
    assert_eq!(
        initial.0 & 0xffff,
        0,
        "did not enter single-player gameplay"
    );
    tick(&mut game, &mut state, 60, 4 << 8);
    let moved = position(&game);
    assert_ne!(
        (initial.1, initial.2),
        (moved.1, moved.2),
        "Up did not move"
    );
    assert_ne!(
        game.diagnostic.unwrap().2 & 4,
        0,
        "controller not forwarded"
    );
    assert!(
        tick(&mut game, &mut state, 60, 16 << 8),
        "firing produced no PCM"
    );
    tick(&mut game, &mut state, 1, (1 << 22) | (1 << 23) | 64);
    for _ in 0..60 {
        tick(&mut game, &mut state, 1, 0);
        if root.join("saves").exists()
            && std::fs::read_dir(root.join("saves"))
                .unwrap()
                .next()
                .is_some()
        {
            break;
        }
    }
    let saved_position = position(&game);
    let saves: Vec<_> = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(saves.len(), 1);
    let save = std::fs::read(&saves[0]).unwrap();
    assert!(save.len() > 1000 && save.len() <= 512 * 1024);
    let image = game
        .frame
        .commands
        .iter()
        .find_map(|command| match command {
            crate::drawing::Command::Image { image, .. } => Some(image),
            _ => None,
        })
        .expect("gameplay image missing");
    assert_eq!(image.pixels().len(), 320 * 200 * 4);
    let unique: std::collections::BTreeSet<_> = image.pixels().as_chunks::<4>().0.iter().collect();
    assert!(unique.len() > 64, "blank gameplay frame");
    tick(&mut game, &mut state, 60, 8 << 8);
    let away = position(&game);
    assert_ne!((away.1, away.2), (saved_position.1, saved_position.2));
    drop(game);
    let mut resumed = open();
    let mut state = 0;
    for _ in 0..30 {
        tick(&mut resumed, &mut state, 1, 0);
        if position(&resumed).0 & 0xffff == 0 {
            break;
        }
    }
    let loaded = position(&resumed);
    assert_eq!(loaded.0 & 0xffff, 0);
    assert!(
        loaded.1.abs_diff(saved_position.1) <= 2 && loaded.2.abs_diff(saved_position.2) <= 2,
        "checkpoint not restored: {loaded:?} vs {saved_position:?}"
    );
    state = 0;
    tick(&mut resumed, &mut state, 10, 0);
    let restarted = position(&resumed);
    assert_eq!(restarted.0 & 0xffff, 0);
    assert_ne!(
        (restarted.1, restarted.2),
        (loaded.1, loaded.2),
        "restart did not reset level"
    );
    tick(&mut resumed, &mut state, 1, 128);
    tick(&mut resumed, &mut state, 5, 0);
    drop(resumed);
    assert_eq!(
        crate::verify::digest(&std::fs::read(wad).unwrap()),
        hash,
        "test data changed"
    );
    verify_missing_palette_error(root, settings, &bytes);
    println!("Verified private native DOOM worker: public Freedoom; gameplay, controller, stereo PCM, nonblank frame, scoped save, close/reload and restart; no physical audio.");
}

fn verify_missing_palette_error(root: &Path, settings: &crate::settings::Settings, wad: &[u8]) {
    // Derive an invalid input in RAM. Keep the original public file unchanged
    // and copy only the already-verified executable into a fresh save scope.
    let fresh = tempfile::tempdir().unwrap();
    let archive = root.join("com.gamepkg.doom.gamepkg");
    let manifest = crate::registry::install(fresh.path(), &archive, true).unwrap();
    let digest = crate::verify::digest(&std::fs::read(&archive).unwrap());
    let source = super::path_for_digest(root, &digest);
    let executable = std::fs::read(source).unwrap();
    let target = super::path_for_digest(fresh.path(), &digest);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, &executable).unwrap();
    crate::integrity::record_build(&target, &executable).unwrap();
    let mut invalid = wad.to_vec();
    let count = u32::from_le_bytes(invalid[4..8].try_into().unwrap()) as usize;
    let offset = u32::from_le_bytes(invalid[8..12].try_into().unwrap()) as usize;
    let mut changed = 0;
    for entry in invalid[offset..offset + count * 16].chunks_exact_mut(16) {
        if &entry[8..16] == b"PLAYPAL\0" {
            entry[8..16].copy_from_slice(b"BADPAL\0\0");
            changed += 1;
        }
    }
    assert_eq!(changed, 1, "expected the pinned public palette resource");
    let mut package = crate::archive::Package::open(&archive).unwrap();
    crate::verify::verify_embedded(&mut package).unwrap();
    package.resolve_shared_assets(fresh.path()).unwrap();
    let error = Session::new(
        fresh.path(),
        &digest,
        package,
        manifest,
        vec![invalid],
        settings,
        0,
    )
    .err()
    .expect("missing palette unexpectedly initialized the game");
    let message = format!("{error:#}");
    assert!(
        message.contains("W_GetNumForName") && message.contains("PLAYPAL"),
        "missing original native engine error: {message}"
    );
    assert!(
        !message.contains("deadline") && !message.contains("time limit"),
        "native engine error was replaced by a timeout: {message}"
    );
    println!("Verified native DOOM fatal diagnostics: missing PLAYPAL reported; no timeout or error dialog; original file unchanged.");
}

fn position(game: &Session) -> (u32, u16, u16) {
    let diagnostic = game.diagnostic.expect("missing engine diagnostics");
    (
        diagnostic.0,
        diagnostic.1 as u16,
        (diagnostic.1 >> 16) as u16,
    )
}

fn tick(game: &mut Session, state: &mut u64, frames: u32, input: u32) -> bool {
    let mut audible = false;
    for frame in 0..frames {
        let before = *state;
        *state = game
            .step(input, *state, &[])
            .unwrap_or_else(|error| panic!("tick state={before}, batch frame={frame}, input={input:#x}, last engine diagnostic={:?}: {error:#}",game.diagnostic));
        assert_eq!(game.audio_channels, 2);
        assert_eq!(game.audio.len(), 1600);
        audible |= game.audio.iter().any(|sample| *sample != 0);
    }
    audible
}

#[test]
#[ignore = "requires GAMEPKG_PUBLIC_DOOM_WAD and actual native build tools"]
fn public_freedoom_worker_gameplay_audio_and_save() {
    let root = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/doom");
    let archive = root.path().join("fixture.gamepkg");
    crate::pack::pack(&source, &archive, &[81; 32]).unwrap();
    let manifest = crate::registry::install(root.path(), &archive, true).unwrap();
    let settings = crate::settings::Settings::default();
    super::build::compile_with_settings(root.path(), &manifest.id, &settings).unwrap();
    verify(
        root.path(),
        &settings,
        &std::path::PathBuf::from(std::env::var_os("GAMEPKG_PUBLIC_DOOM_WAD").unwrap()),
    );
}
