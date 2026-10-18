use gamepkg::{manifest::Manifest, runtime::Game};
fn fixture(component: bool, spin: bool) -> (Vec<u8>, Manifest) {
    let mut manifest =
        Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
    let (namespace, name, entry) = if component {
        manifest.runtime_profile = "component-v1".into();
        ("gamepkg:engine/host@0.1.0", "window-state", "tick")
    } else {
        ("gamepkg", "window_state", "gamepkg_tick")
    };
    let body = if spin {
        "(loop $forever call $read drop br $forever) i64.const 0".to_owned()
    } else {
        format!(
            "call $read local.set $first {} local.get $first i64.extend_i32_u",
            "call $read local.get $first i32.ne if unreachable end ".repeat(64)
        )
    };
    let module=wat::parse_str(format!(r#"(module(import "{namespace}" "{name}"(func $read(result i32)))(memory(export "memory") 1)(func(export "{entry}")(param i32 i64)(result i64)(local $first i32){body}))"#)).unwrap();
    (
        if component {
            gamepkg::hostapi::componentize(module).unwrap()
        } else {
            module
        },
        manifest,
    )
}
#[test]
fn actual_window_observations_are_scoped_permission_gated_and_not_predicted() {
    for component in [false, true] {
        let (bytes, mut manifest) = fixture(component, false);
        manifest.permissions.input = true;
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        assert_eq!(game.step(0).unwrap(), 0);
        for (observed, expected) in [
            (Some(true), 3),
            (Some(false), 1),
            (None, 0),
            (Some(true), 3),
        ] {
            game.observe_fullscreen(observed);
            assert_eq!(game.step(0).unwrap(), expected);
        }
        let mut separate = Game::from_bytes(&bytes, &manifest).unwrap();
        assert_eq!(separate.step(0).unwrap(), 0);
        assert_eq!(game.step(0).unwrap(), 3);
        manifest.permissions.input = false;
        let mut denied = Game::from_bytes(&bytes, &manifest).unwrap();
        denied.observe_fullscreen(Some(true));
        assert_eq!(denied.step(0).unwrap(), 0);
    }
}
#[test]
fn readonly_observation_calls_keep_execution_deadlines() {
    for component in [false, true] {
        let (bytes, manifest) = fixture(component, true);
        assert!(Game::from_bytes(&bytes, &manifest)
            .unwrap()
            .step(0)
            .is_err());
    }
}
