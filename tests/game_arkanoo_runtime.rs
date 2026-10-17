use gamepkg::{drawing::Command, runtime::Game};
#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn real_gameplay_audio_progress_and_restart() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ARKANOO_TEST_ROOT").unwrap());
    let mut game = Game::load(&root, "com.gamepkg.arkanoo").unwrap();
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
            matches!(command, Command::Rect {width,height,..} if *width==28.0 && *height==16.0)
        }).count();
        broken |= bricks < 200;
        assert!(game.frame().commands.len() < 600);
    }
    assert!(audio, "no synthesized collision/launch audio");
    assert!(broken, "no bricks broken by actual game physics");
    drop(game);
    let saves: Vec<_> = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(saves.len(), 1);
    let bytes = std::fs::read(&saves[0]).unwrap();
    assert_eq!(&bytes[..4], b"ARK1");
    let score = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert!(score >= 10, "checkpoint did not retain the high score");
    let records = std::fs::read_dir(root.join("progress")).unwrap();
    assert!(records.filter_map(Result::ok).any(|entry| {
        std::fs::read_to_string(entry.path()).is_ok_and(|text| text.contains("first_brick"))
    }));
    let mut reloaded = Game::load(&root, "com.gamepkg.arkanoo").unwrap();
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
    copy(&source, root.path());
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
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ARKANOO_TEST_ROOT").unwrap());
    let mut game = Game::load(&root, "com.gamepkg.arkanoo-modes").unwrap();
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
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ARKANOO_TEST_ROOT").unwrap());
    let mut game = Game::load(&root, "com.gamepkg.arkanoo-modes").unwrap();
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
