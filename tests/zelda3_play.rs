use gamepkg::{drawing::Command, runtime::Game};
fn copy(from: &std::path::Path, to: &std::path::Path, include_session: bool) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if !include_session && (name == "saves" || name == "progress" || name == "states") {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(name), include_session);
        } else {
            std::fs::copy(entry.path(), to.join(name)).unwrap();
        }
    }
}
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
fn actual_zelda3_host_snapshots_restore_without_restarting_and_replay_exactly() {
    fn run(game: &mut Game, frames: u32, input: u32) {
        for _ in 0..frames {
            game.step(input).unwrap();
        }
    }
    fn enter_gameplay(game: &mut Game) {
        for (frames, input) in [
            (1, 64),
            (60, 0),
            (1, 64),
            (60, 0),
            (1, 16),
            (10, 0),
            (1, 64),
            (60, 0),
            (1, 64),
            (600, 0),
            (180, 1 << 20),
        ] {
            run(game, frames, input);
        }
        for _ in 0..12 {
            run(game, 1, 16);
            run(game, 180, 0);
        }
        assert_eq!(
            game.builtin_state().unwrap().unwrap().0 & 255,
            7,
            "opening dialogue did not reach gameplay"
        );
    }
    let original = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
    let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&original, fixture.path(), false);
    let mut game = load(fixture.path(), &rom);
    for _ in 0..1000 {
        game.step(0).unwrap();
    }
    let snapshot = game.capture_zelda_snapshot().unwrap();
    assert_eq!(snapshot.state(), 1000);
    assert_eq!(game.state, 1000, "capture advanced a game tick");
    let diagnostic = game.builtin_state().unwrap();
    let controller = game.builtin_controller().unwrap();
    // Three owned states fit with transfer scratch; a fourth must fail before
    // touching the worker. Releasing one immediately restores that capacity.
    let second = game.capture_zelda_snapshot().unwrap();
    let third = game.capture_zelda_snapshot().unwrap();
    assert!(game.capture_zelda_snapshot().is_err());
    drop(second);
    drop(third);
    let fresh_fixture = tempfile::tempdir().unwrap();
    copy(&original, fresh_fixture.path(), false);
    let mut fresh = load(fresh_fixture.path(), &rom);
    assert_eq!(fresh.state, 0);
    fresh.restore_zelda_snapshot(&snapshot).unwrap();
    assert_eq!(fresh.state, 1000);
    assert!(fresh.frame().commands.is_empty());
    assert!(fresh.audio_frame().is_empty());
    assert_eq!(fresh.builtin_state().unwrap(), diagnostic);
    assert_eq!(fresh.builtin_controller().unwrap(), controller);
    // Mutate the existing engine before restoring, while the other engine was
    // freshly initialized. Both must resume the captured original state.
    enter_gameplay(&mut game);
    game.restore_zelda_snapshot(&snapshot).unwrap();
    assert_eq!(game.state, 1000);
    assert_eq!(game.builtin_state().unwrap(), diagnostic);
    fn compare(game: &mut Game, fresh: &mut Game, inputs: impl IntoIterator<Item = u32>) {
        for (frame, input) in inputs.into_iter().enumerate() {
            game.step(input).unwrap();
            fresh.step(input).unwrap();
            assert_eq!(game.state, fresh.state, "wrapper counter frame {frame}");
            assert_eq!(
                game.builtin_state().unwrap(),
                fresh.builtin_state().unwrap(),
                "engine frame {frame}"
            );
            assert_eq!(
                game.builtin_controller().unwrap(),
                fresh.builtin_controller().unwrap(),
                "controller frame {frame}"
            );
            assert!(
                game.frame().commands == fresh.frame().commands,
                "pixels frame {frame}"
            );
            assert_eq!(game.audio_channels(), 2);
            if game.audio_frame() != fresh.audio_frame() {
                let sample = game
                    .audio_frame()
                    .iter()
                    .zip(fresh.audio_frame())
                    .position(|(a, b)| a != b);
                let a = game.capture_zelda_snapshot().unwrap();
                let b = fresh.capture_zelda_snapshot().unwrap();
                let differences: Vec<_> = a
                    .bytes()
                    .iter()
                    .zip(b.bytes())
                    .enumerate()
                    .filter(|(_, (a, b))| a != b)
                    .take(24)
                    .map(|(offset, (a, b))| (offset, *a, *b))
                    .collect();
                panic!("PCM frame {frame}: first sample difference {sample:?}; snapshot differences {differences:?}");
            }
        }
    }
    compare(
        &mut game,
        &mut fresh,
        (0..180).map(|frame| if frame == 30 || frame == 90 { 64 } else { 0 }),
    );
    game.restore_zelda_snapshot(&snapshot).unwrap();
    enter_gameplay(&mut game);
    let gameplay = game.capture_zelda_snapshot().unwrap();
    let gameplay_diagnostic = game.builtin_state().unwrap();
    run(&mut game, 1, 64);
    run(&mut game, 45, 0);
    assert_ne!(
        game.builtin_state().unwrap().unwrap().0 & 255,
        7,
        "inventory did not open"
    );
    game.restore_zelda_snapshot(&gameplay).unwrap();
    fresh.restore_zelda_snapshot(&gameplay).unwrap();
    assert_eq!(game.builtin_state().unwrap(), gameplay_diagnostic);
    assert_eq!(fresh.builtin_state().unwrap(), gameplay_diagnostic);
    compare(
        &mut game,
        &mut fresh,
        (0..180).map(|frame| match frame {
            0..60 => 512,
            60..90 => 1 << 20,
            90..150 => 256,
            _ => 0,
        }),
    );
    // Persist complete gameplay, stop its originating session, then restore
    // into a newly launched game rather than retain an in-memory snapshot.
    let saved_counter = game.state;
    let saved_diagnostic = game.builtin_state().unwrap();
    game.save_zelda_slot(fixture.path(), 0).unwrap();
    game.save_zelda_slot(fixture.path(), 19).unwrap();
    let manifest =
        gamepkg::registry::installed_manifest(fixture.path(), "com.gamepkg.zelda3").unwrap();
    let usage = gamepkg::storage::usage(fixture.path(), &manifest).unwrap();
    assert_eq!(
        usage.save_bytes,
        8192 + 2 * (275497 + 96),
        "snapshot files are missing from per-game storage"
    );
    drop(game);
    let mut restarted = load(fixture.path(), &rom);
    restarted.load_zelda_slot(fixture.path(), 0).unwrap();
    assert_eq!(restarted.state, saved_counter);
    assert_eq!(restarted.builtin_state().unwrap(), saved_diagnostic);
    fresh.load_zelda_slot(fixture.path(), 19).unwrap();
    compare(
        &mut restarted,
        &mut fresh,
        (0..120).map(|frame| if frame < 60 { 512 } else { 256 }),
    );
    // Disk corruption must fail before changing a live engine or its counter.
    let slot = std::fs::read_dir(fixture.path().join("states"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("-01.gpstate")
        })
        .unwrap();
    let mut bytes = std::fs::read(&slot).unwrap();
    bytes[56] ^= 0x80;
    std::fs::write(&slot, &bytes).unwrap();
    let before = restarted.state;
    let diagnostic = restarted.builtin_state().unwrap();
    assert!(restarted.load_zelda_slot(fixture.path(), 0).is_err());
    assert_eq!(restarted.state, before);
    assert_eq!(restarted.builtin_state().unwrap(), diagnostic);
    restarted.step(0).unwrap();
    assert_eq!(restarted.state, before + 1);
    // Snapshot ownership survives its originating game and can be restored by
    // another session of the same signed game without copying the full state.
    fresh.restore_zelda_snapshot(&snapshot).unwrap();
    assert_eq!(fresh.state, 1000);
    fresh.step(0).unwrap();
    assert_eq!(fresh.state, 1001);
    fresh.finish().unwrap();
}
#[test]
#[ignore = "requires the signed built Zelda3 fixture and player-selected ROM"]
fn actual_zelda3_frames() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
    let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
    let fixture = tempfile::tempdir().unwrap();
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
    image::save_buffer(
        path,
        image.pixels(),
        image.size()[0] as u32,
        image.size()[1] as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
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

#[test]
#[ignore = "requires the signed built Zelda3 fixture and player-selected ROM"]
fn actual_zelda3_opposing_directions_follow_original_filter() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
    let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
    let mut game = load(&root, &rom);
    for _ in 0..1000 {
        game.step(0).unwrap();
    }
    for directions in 0u32..16 {
        let mut expected = directions;
        if expected & 3 == 3 {
            expected &= !3;
        }
        if expected & 12 == 12 {
            expected &= !12;
        }
        let expected = ((expected & 1) << 1)
            | ((expected & 2) >> 1)
            | ((expected & 4) << 1)
            | ((expected & 8) >> 1);
        game.step(directions).unwrap();
        game.step(directions).unwrap();
        assert_eq!(
            game.builtin_controller().unwrap().unwrap() & 15,
            expected,
            "portable direction combination {directions:04b}"
        );
    }
    game.finish().unwrap();
}

#[test]
#[ignore = "requires fresh signed Zelda video fixtures and player-selected ROM"]
fn actual_zelda3_video_dimensions_aspect_and_pixels() {
    use sha2::{Digest, Sha256};
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
    let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
    let width: usize = std::env::var("GAMEPKG_ZELDA3_VIDEO_WIDTH")
        .unwrap()
        .parse()
        .unwrap();
    let height: usize = std::env::var("GAMEPKG_ZELDA3_VIDEO_HEIGHT")
        .unwrap()
        .parse()
        .unwrap();
    let mut game = load(&root, &rom);
    let mut digest = Sha256::new();
    let mut visible = false;
    for _frame in 1..=1000 {
        game.step(0).unwrap();
        let current = game.frame();
        let (x, y, drawn_width, drawn_height, image) = current
            .commands
            .iter()
            .find_map(|command| {
                if let Command::Image {
                    x,
                    y,
                    width,
                    height,
                    image,
                } = command
                {
                    Some((*x, *y, *width, *height, image))
                } else {
                    None
                }
            })
            .expect("missing original renderer image");
        assert_eq!(image.size(), [width, height]);
        assert_eq!(image.pixels().len(), width * height * 4);
        assert!(x >= 0.0 && y >= 0.0 && x + drawn_width <= 640.001 && y + drawn_height <= 480.001);
        assert!((drawn_width / drawn_height - width as f32 / height as f32).abs() < 0.00001);
        assert!((x * 2.0 + drawn_width - 640.0).abs() < 0.001);
        assert!((y * 2.0 + drawn_height - 480.0).abs() < 0.001);
        assert!(image
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255));
        visible |= image
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[..3] != [0, 0, 0]);
        digest.update(image.pixels());
    }
    assert!(visible, "original renderer stayed black");
    game.finish().unwrap();
    if let Some(output) = std::env::var_os("GAMEPKG_ZELDA3_VIDEO_FINGERPRINT") {
        std::fs::write(output, hex::encode(digest.finalize())).unwrap();
    }
}

#[test]
#[ignore = "requires signed built Zelda fixtures, selected ROM and isolated compiler"]
fn actual_zelda3_stale_renderer_is_rejected_and_integrity_fix_rebuilds() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
    let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&root, fixture.path(), true);
    let root = fixture.path();
    let id = "com.gamepkg.zelda3";
    let package = std::fs::read(root.join(format!("{id}.gamepkg"))).unwrap();
    let native = gamepkg::native::cache_path(root, id).unwrap();
    if native.exists() {
        std::fs::remove_file(native).unwrap();
    }
    let engine = gamepkg::build::cache_path(root, id)
        .unwrap()
        .with_file_name("zelda3.wasm");
    // Preserve every actual engine function, but remove the new compatibility
    // section to reproduce a fully callable pre-source-identity cache layout.
    let current = std::fs::read(&engine).unwrap();
    let mut old = current[..8].to_vec();
    let mut offset = 8;
    let mut removed = 0;
    for payload in wasmparser::Parser::new(0).parse_all(&current) {
        let payload = payload.unwrap();
        if let Some((_, range)) = payload.as_section() {
            if matches!(&payload, wasmparser::Payload::CustomSection(section)
                if section.name() == "gamepkg.zelda3.sources.v1")
            {
                removed += 1;
            } else {
                old.extend_from_slice(&current[offset..range.end]);
            }
            offset = range.end;
        }
    }
    assert_eq!(removed, 1);
    assert_eq!(offset, current.len());
    wasmparser::Validator::new().validate_all(&old).unwrap();
    std::fs::write(&engine, &old).unwrap();
    gamepkg::integrity::record_build(&engine, &old).unwrap();
    assert!(!gamepkg::build::is_built(root, id).unwrap());
    assert!(format!("{:#}", gamepkg::integrity::check(root, id).unwrap_err()).contains("rebuild"));
    let error = match Game::load_target(root, id, std::slice::from_ref(&rom), Some(false)) {
        Ok(_) => panic!("stale renderer launched"),
        Err(error) => error,
    };
    assert!(format!("{error:#}").contains("rebuild"));
    gamepkg::integrity::fix_target(root, id, None, false).unwrap();
    assert!(gamepkg::build::is_built(root, id).unwrap());
    gamepkg::integrity::check(root, id).unwrap();
    let mut game = Game::load_target(root, id, &[rom], Some(false)).unwrap();
    game.step(0).unwrap();
    game.finish().unwrap();
    assert_eq!(
        std::fs::read(root.join(format!("{id}.gamepkg"))).unwrap(),
        package
    );
}

#[test]
#[ignore = "requires signed built Zelda fixtures and selected ROM"]
fn actual_zelda3_early_intro_skip_preserves_original_default() {
    let source = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
    let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
    for early in [false, true] {
        let fixture = tempfile::tempdir().unwrap();
        copy(&source, fixture.path(), false);
        let manifest =
            gamepkg::registry::installed_manifest(fixture.path(), "com.gamepkg.zelda3").unwrap();
        gamepkg::video_selections::remember_for_game(
            fixture.path(),
            &manifest.id,
            Some(gamepkg::manifest::ZeldaVideo {
                skip_intro_on_keypress: early,
                ..Default::default()
            }),
        )
        .unwrap();
        let mut game = load(fixture.path(), &rom);
        let mut reached = false;
        for _ in 0..2000 {
            game.step(0).unwrap();
            let state = game.builtin_state().unwrap().unwrap().0;
            if state & 255 == 0 && (state >> 8) & 255 == 4 {
                reached = true;
                break;
            }
        }
        assert!(reached, "original intro never reached stage 4");
        for _ in 0..4 {
            game.step(64).unwrap();
        }
        let module = game.builtin_state().unwrap().unwrap().0 & 255;
        assert_eq!(
            module,
            if early { 1 } else { 0 },
            "early intro skip choice {early} did not follow original stage threshold"
        );
        game.finish().unwrap();
    }
}

#[test]
#[ignore = "requires signed built Zelda fixtures and selected ROM"]
fn actual_zelda3_all_logical_buttons_follow_original_joypad_mapping() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
    let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&root, fixture.path(), false);
    let mut game = load(fixture.path(), &rom);
    for _ in 0..1000 {
        game.step(0).unwrap();
    }
    for (pressed, held, expected) in [
        (1, 1 << 8, 0x0002),
        (2, 1 << 9, 0x0001),
        (4, 1 << 10, 0x0008),
        (8, 1 << 11, 0x0004),
        (16, 1 << 12, 0x0080),
        (32, 1 << 13, 0x0040),
        (64, 1 << 14, 0x0010),
        (128, 1 << 15, 0x0020),
        (1 << 16, 1 << 20, 0x8000),
        (1 << 17, 1 << 21, 0x4000),
        (1 << 18, 1 << 22, 0x2000),
        (1 << 19, 1 << 23, 0x1000),
    ] {
        for input in [pressed, held] {
            for _ in 0..4 {
                game.step(input).unwrap();
            }
            let controller = game.builtin_controller().unwrap().unwrap();
            assert_eq!(controller & 0xffff, expected, "logical button {input:#x}");
            assert_eq!(
                controller >> 16,
                0,
                "held logical button retained a stale press edge"
            );
            for _ in 0..2 {
                game.step(0).unwrap();
            }
            assert_eq!(
                game.builtin_controller().unwrap().unwrap() & 0xffff,
                0,
                "logical button release {input:#x}"
            );
        }
    }
    game.finish().unwrap();
}
