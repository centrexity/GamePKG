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
        for size in [
            Some([1280, 720]),
            Some([1, 1]),
            Some([65535, 65535]),
            None,
            Some([65536, 720]),
        ] {
            game.observe_window_size(size);
            let expected = if allowed {
                size.filter(|[w, h]| (1..=65535).contains(w) && (1..=65535).contains(h))
                    .map(|[w, h]| u64::from(w) | (u64::from(h) << 32))
                    .unwrap_or(0)
            } else {
                1
            };
            assert_eq!(
                game.step(64).unwrap(),
                expected,
                "size observation {suffix}: {size:?}"
            );
        }
        if allowed {
            game.observe_fullscreen(Some(false));
            game.observe_window_size(Some([960, 720]));
            for (key, expected) in [
                (62, Some([1000, 750])),
                (64, Some([960, 720])),
                (62, Some([1000, 750])),
                (63, Some([1000, 750])),
            ] {
                game.input_event(gamepkg::input_events::Event {
                    kind: 5,
                    x: 0.0,
                    y: 0.0,
                    code: key,
                })
                .unwrap();
                game.step(0).unwrap();
                assert_eq!(
                    game.window_state().resize,
                    expected,
                    "{suffix} resize key {key}"
                );
                if key == 63 {
                    assert!(game.window_state().resize_origin.is_none());
                }
            }
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
