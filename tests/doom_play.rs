//! Requires a local player-owned IWAD; no game data is stored in the repository.
use gamepkg::{drawing::Command, runtime::Game};
fn load(root: &std::path::Path, wad: &std::path::Path) -> Game {
    let native = std::env::var("GAMEPKG_DOOM_TEST_TARGET")
        .ok()
        .map(|target| match target.as_str() {
            "native" => true,
            "wasm" => false,
            _ => panic!("test target must be native or wasm"),
        });
    Game::load_target(root, "com.gamepkg.doom", &[wad.to_path_buf()], native).unwrap()
}
fn copy(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == "saves" || name == "progress" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(name));
        } else {
            std::fs::copy(entry.path(), to.join(name)).unwrap();
        }
    }
}
#[test]
#[ignore = "requires GAMEPKG_DOOM_TEST_ROOT and GAMEPKG_DOOM_TEST_WAD"]
fn real_wad_gameplay_audio_save_and_reload() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_ROOT").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&root, fixture.path());
    let root = fixture.path().to_path_buf();
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let before = gamepkg::verify::digest(&std::fs::read(&wad).unwrap());
    assert!(Game::load(&root, "com.gamepkg.doom").is_err());
    let mut game = load(&root, &wad);
    fn run(game: &mut Game, frames: u32, input: u32) -> bool {
        let mut audible = false;
        for i in 0..frames {
            game.step(input)
                .unwrap_or_else(|e| panic!("frame {i}: {e:#}"));
            assert_eq!(game.audio_channels(), 2);
            assert_eq!(game.audio_frame().len(), 1600);
            audible |= game.audio_frame().iter().any(|v| *v != 0);
        }
        audible
    }
    fn press(game: &mut Game, input: u32) {
        run(game, 1, input);
        run(game, 12, 0);
    }
    run(&mut game, 120, 0);
    for _ in 0..4 {
        press(&mut game, 64);
    }
    run(&mut game, 120, 0);
    let initial = game.builtin_state().unwrap().unwrap();
    assert_eq!(
        initial.0 & 0xffff,
        0,
        "did not start actual single-player gameplay: {initial:?}"
    );
    run(&mut game, 60, 4 << 8);
    let moved = game.builtin_state().unwrap().unwrap();
    assert_ne!(
        (initial.1, initial.2),
        (moved.1, moved.2),
        "held Up did not move the player"
    );
    assert_ne!(game.builtin_controller().unwrap().unwrap() & 4, 0);
    let firing = run(&mut game, 60, 16 << 8);
    assert!(firing, "firing produced no mixed WAD sound effects");
    run(&mut game, 1, (1 << 22) | (1 << 23) | 64);
    run(&mut game, 60, 0);
    let saved_position = game.builtin_state().unwrap().unwrap();
    let saves: Vec<_> = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(saves.len(), 1);
    let bytes = std::fs::read(&saves[0]).unwrap();
    assert!(bytes.len() > 1000 && bytes.len() <= 512 * 1024);
    let frame = game.frame();
    let image = frame
        .commands
        .iter()
        .find_map(|c| {
            if let Command::Image { image, .. } = c {
                Some(image)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(image.pixels().len(), 320 * 200 * 4);
    let unique: std::collections::BTreeSet<_> = image.pixels().as_chunks::<4>().0.iter().collect();
    assert!(unique.len() > 64, "gameplay frame is blank");
    if let Some(path) = std::env::var_os("GAMEPKG_DOOM_PROOF") {
        image::save_buffer(path, image.pixels(), 320, 200, image::ColorType::Rgba8).unwrap();
    }
    run(&mut game, 60, 8 << 8);
    let away = game.builtin_state().unwrap().unwrap();
    assert_ne!((away.1, away.2), (saved_position.1, saved_position.2));
    drop(game);
    let mut resumed = load(&root, &wad);
    run(&mut resumed, 30, 0);
    let loaded = resumed.builtin_state().unwrap().unwrap();
    assert_eq!(loaded.0 & 0xffff, 0);
    // Saved momentum continues during load; compare within two map units,
    // far smaller than the deliberate movement away from the checkpoint.
    assert!(
        loaded.1.abs_diff(saved_position.1) <= 2 && loaded.2.abs_diff(saved_position.2) <= 2,
        "quicksave did not restore the checkpoint: {loaded:?} versus {saved_position:?}"
    );
    resumed.state = 0;
    run(&mut resumed, 10, 0);
    let restarted = resumed.builtin_state().unwrap().unwrap();
    assert_eq!(restarted.0 & 0xffff, 0);
    assert_ne!(
        (restarted.1, restarted.2),
        (loaded.1, loaded.2),
        "Restart did not reset the level"
    );
    run(&mut resumed, 1, 128);
    run(&mut resumed, 5, 0); // menu is usable after restore
    assert_eq!(
        gamepkg::verify::digest(&std::fs::read(&wad).unwrap()),
        before,
        "WAD was modified"
    );
}

#[test]
#[ignore = "requires GAMEPKG_DOOM_TEST_ROOT and GAMEPKG_DOOM_TEST_WAD"]
fn original_attract_demo_has_spatial_stereo() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&root, fixture.path());
    let mut game = load(fixture.path(), &wad);
    let mut demo = false;
    let mut separated = false;
    for _ in 0..3600 {
        game.step(0).unwrap();
        assert_eq!(game.audio_channels(), 2);
        assert_eq!(game.audio_frame().len(), 1600);
        demo |= game.builtin_state().unwrap().unwrap().0 & 256 != 0;
        separated |= game
            .audio_frame()
            .chunks_exact(2)
            .any(|pair| pair[0] != pair[1]);
        if demo && separated {
            break;
        }
    }
    assert!(demo, "original attract demo did not begin");
    assert!(separated, "original demo sounds lost spatial separation");
    game.finish().unwrap();
}
