use gamepkg::{drawing::Command, runtime::Game};
fn copy_library(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == "saves" || name == "progress" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy_library(&entry.path(), &to.join(name));
        } else {
            std::fs::copy(entry.path(), to.join(name)).unwrap();
        }
    }
}
fn isolated_library() -> tempfile::TempDir {
    let source = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ARKANOO_TEST_ROOT").unwrap());
    let root = tempfile::tempdir().unwrap();
    copy_library(&source, root.path());
    root
}
#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn real_gameplay_audio_progress_and_restart() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    assert!(game.frame().commands.len() < 600);
    game.step(4).unwrap();
    game.step(8).unwrap();
    let paused = game.frame().commands.clone();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands, paused);
    game.step(8).unwrap();
    let mut audio = false;
    let mut broken = false;
    for _ in 0..2200 {
        game.step(0).unwrap();
        audio |= game.audio_frame().iter().any(|sample| *sample != 0);
        let bricks = game.frame().commands.iter().filter(|command| {
            matches!(command, Command::Rect {width,height,..} if *width==30.0 && *height==10.0)
        }).count();
        broken |= bricks < 200;
        assert!(game.frame().commands.len() < 600);
    }
    assert!(audio, "no synthesized collision/launch audio");
    assert!(broken, "no bricks broken by actual game physics");
    drop(game);
    let saves: Vec<_> = std::fs::read_dir(root.path().join("saves"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(saves.len(), 1);
    let bytes = std::fs::read(&saves[0]).unwrap();
    assert_eq!(&bytes[..4], b"ARK2");
    let score = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert!(score >= 10, "checkpoint did not retain the high score");
    let records = std::fs::read_dir(root.path().join("progress")).unwrap();
    assert!(records.filter_map(Result::ok).any(|entry| {
        std::fs::read_to_string(entry.path()).is_ok_and(|text| text.contains("first_brick"))
    }));
    let mut reloaded = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    reloaded.step(0).unwrap();
    reloaded.step(4).unwrap();
    assert!(reloaded
        .frame()
        .commands
        .iter()
        .any(|command| matches!(command, Command::Circle { .. })));
}

#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn gravity_option_is_visible_and_persists_through_the_real_save_api() {
    let source = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ARKANOO_TEST_ROOT").unwrap());
    let root = tempfile::tempdir().unwrap();

    copy_library(&source, root.path());
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(131072).unwrap();
    let path = std::fs::read_dir(root.path().join("saves"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(&std::fs::read(&path).unwrap()[12..16], &[1, 0, 0, 0]);
    game.finish().unwrap();
    drop(game);
    let mut resumed = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    resumed.step(0).unwrap();
    assert!(
        resumed
            .frame()
            .commands
            .iter()
            .any(|command| match command {
                Command::Image { image, .. } => image
                    .pixels()
                    .chunks_exact(4)
                    .any(|pixel| pixel == [255, 205, 96, 255]),
                _ => false,
            }),
        "saved gravity indicator was absent after relaunch"
    );
    resumed.step(131072).unwrap();
    assert_eq!(&std::fs::read(path).unwrap()[12..16], &[0, 0, 0, 0]);
    resumed.finish().unwrap();
}

#[test]
#[ignore = "requires the production-physics portal fixture prepared by tests/arkanoo.py"]
fn portal_threshold_orbit_gravity_and_delayed_transition_reach_the_real_renderer() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let state = game.step(0).unwrap();
    assert_eq!(state >> 32, 5000);
    assert_eq!(state & 1, 1);
    assert_eq!((state >> 8) & 255, 3);
    let circles = game
        .frame()
        .commands
        .iter()
        .filter(|command| matches!(command, Command::Circle { .. }))
        .count();
    assert!(circles >= 49, "portal ring/orbit was not rendered");
    assert_eq!(game.step(131072).unwrap() & 2, 2);
    game.step(8).unwrap();
    let paused = game.frame().commands.clone();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands, paused);
    game.step(8).unwrap();
    game.step(64).unwrap();
    for _ in 0..268 {
        assert_eq!(game.step(0).unwrap() & 4, 0);
    }
    assert_eq!(game.step(0).unwrap() & 4, 4);
    let next = game.step(4).unwrap();
    assert_eq!(next & 1, 0);
    assert_eq!((next >> 16) & 255, 2);
    game.finish().unwrap();
}

#[test]
#[ignore = "requires the production-physics fixture prepared by tests/arkanoo.py"]
fn multiball_ejection_and_shockwave_reach_the_real_renderer() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let state = game.step(4096).unwrap();
    assert_eq!(
        state & 8,
        8,
        "original upward/separating response did not run"
    );
    assert_eq!(state & 1, 0, "contact incorrectly activated the portal");
    let shockwave = game
        .frame()
        .commands
        .iter()
        .filter(|command| matches!(command, Command::Rect { rgba, .. } if *rgba == 0xc8ffffff))
        .count();
    assert_eq!(shockwave, 36, "missing original 36-particle shockwave");
}

#[test]
#[ignore = "requires the production-physics fixture prepared by tests/arkanoo.py"]
fn original_brick_geometry_and_pair_before_paddle_order_reach_real_runtime() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let state = game.step(1024).unwrap();
    assert_eq!(
        state & 16,
        16,
        "integer brick collision did not match original geometry"
    );
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Circle{x,y,radius,..} if *x==34.0 && *y==113.0 && *radius==3.0)));
    drop(game);
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    assert_eq!(
        game.step(2048).unwrap() & 32,
        32,
        "pair contacts did not precede original paddle response"
    );
}

#[test]
#[ignore = "requires the production-physics fixture prepared by tests/arkanoo.py"]
fn per_ball_launch_delay_and_life_victory_flow_work_in_real_runtime() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    assert_ne!(game.step(8192).unwrap() & 64, 0);
    for _ in 1..30 {
        assert_ne!(game.step(0).unwrap() & 64, 0);
    }
    let state = game.step(0).unwrap();
    assert_eq!(state & 64, 0);
    assert_ne!(
        state & 128,
        0,
        "auto-launch moved before the original next frame"
    );
    game.step(0).unwrap();
    assert!(game.frame().commands.iter().any(
        |command| matches!(command,Command::Circle{y,radius,..} if *y==390.0 && *radius==3.0)
    ));
    let state = game.step(16384).unwrap();
    assert_eq!(state >> 32, 80);
    assert_eq!((state >> 8) & 3, 2);
    assert_ne!(state & 2048, 0, "life loss erased active bonuses");
    let state = game.step(64).unwrap();
    assert_eq!((state >> 8) & 3, 3);
    assert_ne!(state & 1024, 0, "level nine did not enter victory");
    let state = game.step(4).unwrap();
    assert_eq!((state >> 16) & 65535, 10);
    assert_eq!(state & 2048, 0);
    assert_ne!(state & 64, 0);
    let state = game.step(32768).unwrap();
    assert_eq!(state >> 32, 10000004);
    let state = game.step(4).unwrap();
    assert_eq!((state >> 16) & 65535, 1000);
}

#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn legacy_save_keeps_large_best_score_and_levels_above_the_old_cap() {
    let source = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ARKANOO_TEST_ROOT").unwrap());
    let root = tempfile::tempdir().unwrap();
    copy_library(&source, root.path());
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(131072).unwrap();
    game.finish().unwrap();
    drop(game);
    let save = std::fs::read_dir(root.path().join("saves"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut bytes = b"ARK1".to_vec();
    bytes.extend_from_slice(&10000004u32.to_le_bytes());
    bytes.extend_from_slice(&1000u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    std::fs::write(&save, bytes).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{x,y,width,..} if *x==240.0 && *y==23.0 && *width==48.0)),"best score was clamped or truncated");
    game.step(4).unwrap();
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{x,y,width,..} if *x==352.0 && *y==23.0 && *width==48.0)),"saved level 1000 was clamped");
    game.step(131072).unwrap();
    game.finish().unwrap();
    drop(game);
    let bytes = std::fs::read(&save).unwrap();
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        10000004
    );
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1000);
}

#[test]
#[ignore = "requires the production-physics fixture prepared by tests/arkanoo.py"]
fn special_bricks_ice_damage_and_world_radius_blasts_reach_real_renderers() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let state = game.step(65536).unwrap();
    assert_ne!(state & 4096, 0);
    assert_eq!(state >> 32, 0);
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{x,y,width,height,image} if *x==20.0 && *y==100.0 && *width==30.0 && *height==10.0 && image.pixels().chunks_exact(4).any(|pixel|pixel==[255,255,255,220]))),"damaged ice crack missing");
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{x,y,image,..} if *x==50.0 && *y==100.0 && image.pixels().chunks_exact(4).any(|pixel|pixel==[30,35,40,255]) && image.pixels().chunks_exact(4).any(|pixel|pixel==[150,150,160,255]))),"metal brick/rivets missing");
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{x,y,image,..} if *x==80.0 && *y==100.0 && image.pixels().chunks_exact(4).any(|pixel|pixel[0]>=220 && pixel[1]>=80 && pixel[2]==0 && pixel[3]==255))),"explosive brick missing");
    let state = game.step(262144).unwrap();
    assert_eq!(state >> 32, 10);
    assert_eq!(state & 4096, 0);
    let state = game.step(524288).unwrap();
    assert_eq!(state >> 32, 40);
    assert_ne!(
        state & 8192,
        0,
        "world-distance blast or no-chain rule incorrect"
    );
    assert!(
        game.frame()
            .commands
            .iter()
            .any(|command| matches!(command,Command::Image{x,y,..} if *x==20.0 && *y==160.5)),
        "outside-radius ice brick was removed"
    );
    game.step(1048576).unwrap();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands.iter().filter(|command|matches!(command,Command::Image{width,height,..} if *width==30.0 && *height==10.0)).count(),200);
    assert!(
        game.frame().commands.len() < 400,
        "special bricks exceeded drawing limits"
    );
}

#[test]
#[ignore = "requires the production-layout fixture prepared by tests/arkanoo.py"]
fn original_layouts_match_in_native_and_32_bit_wasm_through_the_real_renderer() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-levels.json")).unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-levels").unwrap();
    for fixture in reference["layouts"].as_array().unwrap() {
        let level = fixture["level"].as_u64().unwrap();
        assert_eq!(game.step(level as u32).unwrap(), level);
        let mut actual = [0u8; 200];
        for command in &game.frame().commands {
            if let Command::Rect {
                x,
                y,
                width,
                height,
                ..
            } = command
            {
                if *width == 30.0 && *height == 10.0 {
                    let col = ((*x - 20.0) / 30.0) as usize;
                    let row = ((*y - 100.0) / 10.0) as usize;
                    assert!(col < 20 && row < 10);
                    assert_eq!(
                        (*x, *y),
                        (20.0 + col as f32 * 30.0, 100.0 + row as f32 * 10.0)
                    );
                    assert_eq!(actual[row * 20 + col], 0);
                    actual[row * 20 + col] = 1;
                }
            }
        }
        for (actual, expected) in actual
            .iter()
            .zip(fixture["cells"].as_str().unwrap().bytes())
        {
            assert_eq!(*actual, expected - b'0', "level {level}");
        }
    }
}

#[test]
#[ignore = "requires signed controlled-input fixtures prepared by tests/arkanoo.py"]
fn sdk_input_events_preserve_pointer_keys_unicode_and_permission_on_every_profile() {
    use gamepkg::input_events::Event;
    for suffix in ["events", "events-component"] {
        let root = isolated_library();
        let id = format!("com.gamepkg.arkanoo-{suffix}");
        let mut game = Game::load(root.path(), &id).unwrap();
        game.input_event(Event {
            kind: 1,
            x: 123.5,
            y: 44.25,
            code: 3,
        })
        .unwrap();
        assert_eq!(game.step(0).unwrap(), (1u64 << 32) | 3);
        assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Rect{x,y,width,height,..} if *x==123.5 && *y==44.25 && *width==1.0 && *height==1.0)));
        for (kind, code) in [
            (5, 65536 + 511),
            (7, 0x1f600),
            (7, 0x10ffff),
            (9, 0x80000000 + 114),
            (10, 0x40000000 + 80),
        ] {
            game.input_event(Event {
                kind,
                x: 0.0,
                y: 0.0,
                code,
            })
            .unwrap();
            assert_eq!(
                game.step(0).unwrap(),
                (u64::from(kind) << 32) | u64::from(code)
            );
        }
        assert_eq!(game.step(0).unwrap(), 0);
        drop(game);
        let mut denied = Game::load(root.path(), &format!("{id}-denied")).unwrap();
        assert!(denied.input_event(Event::RESET).is_err());
        assert_eq!(denied.step(0).unwrap(), u64::MAX);
    }
}

#[test]
#[ignore = "requires the signed Arkanoo fixture prepared by tests/arkanoo.py"]
fn actual_source_game_uses_mouse_position_and_original_space_escape_controls() {
    use gamepkg::input_events::Event;
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(4).unwrap();
    game.input_event(Event {
        kind: 1,
        x: 500.4,
        y: 200.0,
        code: 0,
    })
    .unwrap();
    game.input_event(Event {
        kind: 5,
        x: 0.0,
        y: 0.0,
        code: 44,
    })
    .unwrap();
    game.step(0).unwrap();
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Rect{x,y,width,height,..} if *x==465.0 && *y==395.0 && *width==70.0 && *height==11.0)));
    assert!(
        game.frame().commands.iter().any(
            |command| matches!(command,Command::Circle{y,radius,..} if *y==390.0 && *radius==3.0)
        ),
        "Space did not launch immediately"
    );
    game.input_event(Event {
        kind: 5,
        x: 0.0,
        y: 0.0,
        code: 41,
    })
    .unwrap();
    game.step(0).unwrap();
    let paused = game.frame().commands.clone();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands, paused);
    game.input_event(Event {
        kind: 1,
        x: 100.0,
        y: 200.0,
        code: 0,
    })
    .unwrap();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands, paused);
    game.input_event(Event {
        kind: 5,
        x: 0.0,
        y: 0.0,
        code: 41,
    })
    .unwrap();
    game.input_event(Event {
        kind: 1,
        x: 100.0,
        y: 200.0,
        code: 0,
    })
    .unwrap();
    game.step(0).unwrap();
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Rect{x,y,width,..} if *x==65.0 && *y==395.0 && *width==70.0)));
}

fn editor_key(game: &mut Game, key: u32) {
    game.input_event(gamepkg::input_events::Event {
        kind: 9,
        x: 0.0,
        y: 0.0,
        code: key,
    })
    .unwrap();
    game.step(0).unwrap();
}
fn editor_click(game: &mut Game, x: f32, y: f32, code: u32) {
    game.input_event(gamepkg::input_events::Event {
        kind: 2,
        x,
        y,
        code,
    })
    .unwrap();
    game.input_event(gamepkg::input_events::Event {
        kind: 3,
        x,
        y,
        code,
    })
    .unwrap();
    game.step(0).unwrap();
}
fn editor_archive(root: &std::path::Path) -> std::path::PathBuf {
    std::fs::read_dir(root.join("saves"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}
#[test]
#[ignore = "requires signed editor fixtures prepared by tests/arkanoo.py"]
fn original_editor_workflow_persists_patterns_and_preserves_corrupt_saves_in_real_runtimes() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Rect{x,y,width,height,..} if *x==20.0 && *y==100.0 && *width==600.0 && *height==0.5)));
    for (index, key) in [54, 55, 56, 53].iter().enumerate() {
        editor_key(&mut game, *key);
        editor_click(&mut game, 20.0 + index as f32 * 30.0, 100.0, 1);
    }
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Rect{x,y,rgba,..} if *x==110.0 && *y==100.0 && *rgba==0x8a2be2ff)),"original violet brick color was lost");
    editor_key(&mut game, 115);
    let path = editor_archive(root.path());
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"ARK2");
    assert_eq!(u32::from_le_bytes(bytes[16..20].try_into().unwrap()), 1);
    let name_len = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
    assert_eq!(&bytes[24..24 + name_len], b"my_pattern");
    assert_eq!(&bytes[24 + name_len..28 + name_len], &[6, 7, 8, 5]);
    editor_key(&mut game, 116); // Test the actual edited board, then clear through the original Q action.
    game.step(8).unwrap(); // Pause and return to editing without completing the test.
    editor_key(&mut game, 101);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Rect{x,y,width,height,..} if *x==20.0 && *y==100.0 && *width==600.0 && *height==0.5)));
    editor_key(&mut game, 116);
    assert_eq!(
        game.frame()
            .commands
            .iter()
            .filter(
                |c| matches!(c,Command::Image{width,height,..} if *width==30.0 && *height==10.0)
            )
            .count(),
        3
    );
    editor_key(&mut game, 113);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Rect{x,y,width,height,..} if *x==20.0 && *y==100.0 && *width==600.0 && *height==0.5)));
    drop(game);
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_key(&mut game, 108);
    editor_key(&mut game, 13);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Image{x,y,width,height,image} if *x==20.0 && *y==100.0 && *width==30.0 && *height==10.0 && !image.pixels().chunks_exact(4).any(|pixel|pixel==[255,255,255,220]))),"loaded ice incorrectly appears damaged before being hit");
    assert_eq!(
        game.frame()
            .commands
            .iter()
            .filter(
                |c| matches!(c,Command::Image{width,height,..} if *width==30.0 && *height==10.0)
            )
            .count(),
        3
    );
    editor_key(&mut game, 115);
    let stored_again = std::fs::read(&path).unwrap();
    assert_eq!(
        &stored_again[16..],
        &bytes[16..],
        "saved pattern changed during test play"
    );
    assert_eq!(
        u32::from_le_bytes(stored_again[8..12].try_into().unwrap()),
        2
    );
    drop(game);
    let mut damaged = bytes.clone();
    *damaged.last_mut().unwrap() = 9;
    std::fs::write(&path, &damaged).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_click(&mut game, 20.0, 100.0, 1);
    editor_key(&mut game, 115);
    drop(game);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        damaged,
        "invalid save was overwritten"
    );
}
#[test]
#[ignore = "requires signed editor fixtures prepared by tests/arkanoo.py"]
fn denied_editor_saving_does_not_create_a_save_file() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-nosave").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_click(&mut game, 20.0, 100.0, 1);
    editor_key(&mut game, 115);
    game.step(0).unwrap();
    editor_key(&mut game, 108);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Image{x,y,width,height,..} if *x==180.0 && *y==148.0 && *width==102.0 && *height==7.0)),"denied save appeared as a saved pattern in the browser");
    drop(game);
    assert!(
        !root.path().join("saves").exists()
            || std::fs::read_dir(root.path().join("saves"))
                .unwrap()
                .next()
                .is_none()
    );
}
#[test]
#[ignore = "requires signed custom-level fixtures prepared by tests/arkanoo.py"]
fn original_custom_and_procedural_pool_matches_in_native_and_32_bit_wasm() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-custom-levels.json")).unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-custom").unwrap();
    for fixture in reference["layouts"].as_array().unwrap() {
        let level = fixture["level"].as_u64().unwrap() as u32;
        let expected = fixture["cells"]
            .as_array()
            .unwrap()
            .iter()
            .fold(14695981039346656037u64, |hash, cell| {
                (hash ^ cell.as_u64().unwrap()).wrapping_mul(1099511628211)
            });
        assert_eq!(game.step(level).unwrap(), expected, "level {level}");
    }
}

#[test]
#[ignore = "requires signed user-file fixtures prepared by tests/arkanoo.py"]
fn ascii_file_import_and_export_use_only_player_selected_bytes_across_all_profiles() {
    use gamepkg::user_files::{self, Kind, Outcome};
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut game = Game::load(root.path(), id).unwrap();
        game.enable_user_files();
        game.step(0).unwrap();
        editor_key(&mut game, 101);
        editor_key(&mut game, 105);
        let cancelled = game.take_user_file_job().expect("import request");
        assert!(matches!(cancelled.kind, Kind::Import));
        assert!(game.complete_user_file(&cancelled, Outcome::Cancelled));
        game.step(0).unwrap();
        let selected = tempfile::tempdir().unwrap();
        let input = selected.path().join("Imported.TXT");
        let original = format!("# {}\n012345678IEU\n", "x".repeat(70000));
        std::fs::write(&input, &original).unwrap();
        editor_key(&mut game, 105);
        let import = game.take_user_file_job().unwrap();
        assert_ne!(import.handle, cancelled.handle);
        assert!(game.complete_user_file(&import, user_files::execute(&import, Some(&input))));
        game.step(0).unwrap();
        assert_eq!(
            std::fs::read_to_string(&input).unwrap(),
            original,
            "import changed the selected source"
        );
        editor_key(&mut game, 115);
        let saved = std::fs::read(editor_archive(root.path())).unwrap();
        let length = u32::from_le_bytes(saved[20..24].try_into().unwrap()) as usize;
        assert_eq!(&saved[24..24 + length], b"Imported");
        assert_eq!(
            &saved[24 + length..36 + length],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 6, 7, 8]
        );
        editor_key(&mut game, 120);
        let export = game.take_user_file_job().unwrap();
        let Kind::Export { name, bytes } = &export.kind else {
            panic!("not an export");
        };
        assert_eq!(name, "Imported.txt");
        let mut expected=String::from("# Pattern: Imported\n# * = empty, 0-5 = normal, 6 = Ice, 7 = Explosive, 8 = Undestroyable\n\n012345678678********\n");
        for _ in 0..9 {
            expected.push_str("********************\n");
        }
        assert_eq!(bytes, expected.as_bytes());
        let output = selected.path().join("chosen export.txt");
        assert!(game.complete_user_file(&export, user_files::execute(&export, Some(&output))));
        game.step(0).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), expected.as_bytes());
        assert!(!selected.path().join("Imported.txt").exists());
        editor_key(&mut game, 105);
        let bad = game.take_user_file_job().unwrap();
        assert!(game.complete_user_file(
            &bad,
            Outcome::Imported {
                name: "bad.txt".into(),
                bytes: vec![255, 254]
            }
        ));
        game.step(0).unwrap();
        editor_key(&mut game, 115);
        assert_eq!(
            std::fs::read(editor_archive(root.path())).unwrap(),
            saved,
            "invalid imported text replaced the working pattern"
        );
    }
}
#[test]
#[ignore = "requires signed user-file fixtures prepared by tests/arkanoo.py"]
fn editor_file_controls_do_not_grant_undeclared_access_or_open_headless_dialogs() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-nofiles").unwrap();
    game.enable_user_files();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_key(&mut game, 105);
    assert!(game.take_user_file_job().is_none());
    editor_key(&mut game, 120);
    assert!(game.take_user_file_job().is_none());
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_key(&mut game, 105);
    assert!(game.take_user_file_job().is_none());
    game.enable_user_files();
    game.step(0).unwrap();
    assert!(game.take_user_file_job().is_none());
    editor_key(&mut game, 105);
    assert!(game.take_user_file_job().is_some());
}
#[test]
#[ignore = "requires signed user-file fixtures prepared by tests/arkanoo.py"]
fn pattern_archives_larger_than_one_read_chunk_reload_without_being_marked_corrupt() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_click(&mut game, 20.0, 100.0, 1);
    editor_key(&mut game, 115);
    let path = editor_archive(root.path());
    let initial = std::fs::read(&path).unwrap();
    drop(game);
    let length = u32::from_le_bytes(initial[20..24].try_into().unwrap()) as usize;
    let mut large = initial[..20].to_vec();
    large.extend(70000u32.to_le_bytes());
    large.extend(vec![b'x'; 70000]);
    large.extend(&initial[24 + length..]);
    std::fs::write(&path, &large).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_key(&mut game, 108);
    editor_key(&mut game, 13);
    editor_key(&mut game, 115);
    assert_eq!(std::fs::read(&path).unwrap(), large);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Rect{x,y,width,height,rgba} if *x==20.0 && *y==100.0 && *width==30.0 && *height==10.0 && *rgba==0xff0000ff)),"large archive's pattern was not loaded");
}

#[test]
#[ignore = "requires signed user-file fixtures prepared by tests/arkanoo.py"]
fn play_history_failure_does_not_open_a_new_file_dialog() {
    let root = isolated_library();
    let mut setup = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    setup.step(0).unwrap();
    editor_key(&mut setup, 101);
    editor_click(&mut setup, 20.0, 100.0, 1);
    editor_key(&mut setup, 115);
    let path = editor_archive(root.path());
    drop(setup);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    let progress = root.path().join("progress");
    if progress.exists() {
        std::fs::remove_dir_all(&progress).unwrap();
    }
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.enable_user_files();
    std::fs::write(&progress, b"block progress commits").unwrap();
    for code in [101, 105] {
        game.input_event(gamepkg::input_events::Event {
            kind: 9,
            x: 0.0,
            y: 0.0,
            code,
        })
        .unwrap();
    }
    assert!(game.step(0).is_err(), "progress failure was not exercised");
    assert!(
        game.take_user_file_job().is_none(),
        "a failed host tick exposed a file dialog"
    );
}

#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn original_window_bindings_work_across_runtime_profiles() {
    use gamepkg::input_events::Event;
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut game = Game::load(root.path(), id).unwrap();
        game.step(0).unwrap();
        assert!(game.window_state().visible);
        game.step(4).unwrap();
        assert!(!game.window_state().visible);
        assert!(game.window_state().confined);
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 68,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.window_state().fullscreen_toggle);
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 68 | 65536,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(
            game.window_state().fullscreen_toggle,
            "key repeat toggled fullscreen"
        );
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 41,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.window_state().visible);
        assert!(!game.window_state().confined);
        game.input_event(Event {
            kind: 9,
            x: 0.0,
            y: 0.0,
            code: 113,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.exited(), "original Q-to-quit was ignored in pause");
        assert!(game.take_user_file_job().is_none());
    }
}
