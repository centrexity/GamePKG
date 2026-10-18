use gamepkg::runtime::Game;
#[test]
#[ignore = "requires tests/window_observation_game.py fixtures"]
fn generated_sdks_and_real_port_follow_actual_window_state_and_permissions() {
    let root = std::path::PathBuf::from(std::env::var("GAMEPKG_WINDOW_OBSERVATION_ROOT").unwrap());
    for suffix in ["core", "component", "core-denied", "component-denied"] {
        let id = format!("com.gamepkg.window-{suffix}");
        let mut game = Game::load(&root, &id).unwrap();
        let allowed = !suffix.ends_with("denied");
        assert_eq!(game.step(0).unwrap(), 1);
        for (observation, expected) in [
            (Some(false), 2),
            (Some(true), 4),
            (Some(false), 2),
            (None, 1),
        ] {
            game.observe_fullscreen(observation);
            assert_eq!(
                game.step(0).unwrap(),
                if allowed { expected } else { 1 },
                "{suffix}, {observation:?}"
            );
        }
        game.observe_fullscreen(Some(true));
        let mut separate = Game::load(&root, &id).unwrap();
        assert_eq!(separate.step(0).unwrap(), 1);
        assert_eq!(game.step(0).unwrap(), if allowed { 4 } else { 1 });
        if allowed {
            assert!(
                separate.step(128).is_err(),
                "observation reads escaped tick deadline"
            );
        }
    }
}
