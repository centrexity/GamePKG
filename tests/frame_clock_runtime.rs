use gamepkg::runtime::Game;
#[test]
#[ignore = "requires fixtures from tests/frame_clock_game.py"]
fn generated_sdk_clocks_are_scoped_stable_and_keep_deadlines() {
    let root = std::path::PathBuf::from(std::env::var("GAMEPKG_FRAME_CLOCK_ROOT").unwrap());
    for id in ["com.gamepkg.clock-core", "com.gamepkg.clock-component"] {
        let mut game = Game::load(&root, id).unwrap();
        assert_eq!(game.step(0).unwrap(), 1);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = game.step(0).unwrap() >> 1;
        assert!(elapsed >= 10000);
        let mut separate = Game::load(&root, id).unwrap();
        assert_eq!(separate.step(0).unwrap(), 1);
        assert!(game.step(0).unwrap() >> 1 >= elapsed);
        assert!(
            separate.step(128).is_err(),
            "clock calls escaped the execution deadline"
        );
    }
}
