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
