use gamepkg::{drawing::Command, runtime::Game};
fn load(root: &std::path::Path, rom: &std::path::Path) -> Game {
    let native = std::env::var("GAMEPKG_ZELDA3_TEST_TARGET")
        .ok()
        .map(|target| match target.as_str() {
            "native" => true,
            "wasm" => false,
            _ => panic!("test target must be native or wasm"),
        });
    Game::load_target(root, "com.gamepkg.zelda3", &[rom.to_path_buf()], native).unwrap()
}
#[test]
#[ignore = "requires the signed built Zelda3 fixture and player-selected ROM"]
fn actual_zelda3_frames() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
    let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    fn copy(from: &std::path::Path, to: &std::path::Path, include_session: bool) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if !include_session && (name == "saves" || name == "progress") {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &to.join(name), include_session);
            } else {
                std::fs::copy(entry.path(), to.join(name)).unwrap();
            }
        }
    }
    copy(&root, fixture.path(), false);
    let root = fixture.path().to_path_buf();
    let mut game = load(&root, &rom);
    fn run(game: &mut Game, frames: u32, input: u32) -> bool {
        let mut audible = false;
        for frame in 0..frames {
            if let Err(error) = game.step(input) {
                panic!("frame {frame}: {error:#}");
            }
            assert_eq!(
                game.audio_channels(),
                2,
                "Zelda engine audio must remain stereo"
            );
            assert_eq!(game.audio_frame().len(), 1600);
            audible |= game.audio_frame().iter().any(|sample| *sample != 0);
        }
        println!(
            "state {:?} controller {:?}",
            game.builtin_state().unwrap(),
            game.builtin_controller().unwrap()
        );
        audible
    }
    assert!(run(&mut game, 1000, 0), "Title playback produced no audio");
    run(&mut game, 1, 64);
    run(&mut game, 60, 0);
    run(&mut game, 1, 64);
    run(&mut game, 60, 0);
    run(&mut game, 1, 16);
    run(&mut game, 10, 0);
    run(&mut game, 1, 64);
    run(&mut game, 60, 0);
    run(&mut game, 1, 64);
    run(&mut game, 600, 0);
    run(&mut game, 180, 1 << 20);
    for _ in 0..12 {
        run(&mut game, 1, 16);
        run(&mut game, 180, 0);
    }
    let before = game.builtin_state().unwrap().unwrap();
    assert_eq!(
        before.0 & 255,
        7,
        "opening dialogue did not return to indoor gameplay"
    );
    run(&mut game, 30, 512);
    let after = game.builtin_state().unwrap().unwrap();
    assert!(after.1 > before.1, "held Right did not move Link");
    assert_eq!(game.builtin_controller().unwrap().unwrap() & 255, 1);
    let saves: Vec<_> = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(saves.len(), 1);
    let saved = std::fs::read(&saves[0]).unwrap();
    assert_eq!(saved.len(), 8192);
    assert_eq!(
        &saved[0x3e5..0x3e7],
        &[0xaa, 0x55],
        "created save slot is invalid"
    );
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
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/zelda3-proof/runtime-opening.png");
    image::save_buffer(path, image.pixels(), 256, 224, image::ColorType::Rgba8).unwrap();
    drop(game);
    let mut reopened = load(&root, &rom);
    run(&mut reopened, 1000, 0);
    run(&mut reopened, 1, 64);
    run(&mut reopened, 60, 0);
    run(&mut reopened, 1, 64);
    run(&mut reopened, 180, 0);
    assert_ne!(
        reopened.builtin_state().unwrap().unwrap().0 & 255,
        4,
        "reopened slot was lost and returned to name entry"
    );
    assert_eq!(std::fs::read(&saves[0]).unwrap(), saved);
    drop(reopened);
    // Optional developer fixture for the GUI check; it contains only the package,
    // compiled code, play history and SRAM. The selected ROM is never copied.
    if let Some(output) = std::env::var_os("GAMEPKG_ZELDA3_PLAY_EXPORT") {
        let output = std::path::PathBuf::from(output);
        assert!(
            !output.exists(),
            "test export must not replace an existing path"
        );
        copy(&root, &output, true);
    }
    println!("Actual GamePKG Zelda3 title, audio, file/name selection, opening dialogue, Link movement and SRAM reload passed.");
}

#[test]
#[ignore = "requires the signed built Zelda3 fixture and player-selected ROM"]
fn actual_zelda3_stereo_title_preserves_channel_differences() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
    let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
    let mut game = load(&root, &rom);
    let mut separated = false;
    for _ in 0..1000 {
        game.step(0).unwrap();
        assert_eq!(game.audio_channels(), 2);
        assert_eq!(game.audio_frame().len(), 1600);
        separated |= game
            .audio_frame()
            .as_chunks::<2>()
            .0
            .iter()
            .any(|pair| pair[0] != pair[1]);
    }
    assert!(separated, "Zelda DSP stereo channels were collapsed");
    game.finish().unwrap();
}
