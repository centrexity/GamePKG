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
    assert_eq!(&bytes[..4], b"ARK1");
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
