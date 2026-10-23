use gamepkg::{manifest::Manifest, runtime::Game};
fn fixture(component: bool, spin: bool, size: bool) -> (Vec<u8>, Manifest) {
    let mut manifest =
        Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
    let (namespace, name, entry) = if component {
        manifest.runtime_profile = "component-v1".into();
        ("gamepkg:engine/host@0.1.0", "window-state", "tick")
    } else {
        ("gamepkg", "window_state", "gamepkg_tick")
    };
    let name = if size {
        if component {
            "window-size"
        } else {
            "window_size"
        }
    } else {
        name
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
        let (bytes, mut manifest) = fixture(component, false, false);
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
        let (bytes, manifest) = fixture(component, true, false);
        assert!(Game::from_bytes(&bytes, &manifest)
            .unwrap()
            .step(0)
            .is_err());
    }
}

#[test]
fn physical_window_size_is_actual_scoped_bounded_and_permission_gated() {
    for component in [false, true] {
        let (bytes, mut manifest) = fixture(component, false, true);
        manifest.permissions.input = true;
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        assert_eq!(game.step(0).unwrap(), 0);
        for size in [
            Some([1280, 720]),
            Some([65535, 65535]),
            Some([1, 1]),
            None,
            Some([0, 720]),
            Some([1280, 0]),
            Some([65536, 720]),
            Some([1280, u32::MAX]),
        ] {
            game.observe_window_size(size);
            let expected = size
                .filter(|[w, h]| (1..=65535).contains(w) && (1..=65535).contains(h))
                .map(|[w, h]| u64::from(w | (h << 16)))
                .unwrap_or(0);
            assert_eq!(game.step(0).unwrap(), expected);
        }
        game.observe_window_size(Some([1920, 1080]));
        let mut other = Game::from_bytes(&bytes, &manifest).unwrap();
        assert_eq!(other.step(0).unwrap(), 0);
        assert_eq!(game.step(0).unwrap(), 1920 | (1080 << 16));
        manifest.permissions.input = false;
        let mut denied = Game::from_bytes(&bytes, &manifest).unwrap();
        denied.observe_window_size(Some([1920, 1080]));
        assert_eq!(denied.step(0).unwrap(), 0);
        let (bytes, manifest) = fixture(component, true, true);
        assert!(Game::from_bytes(&bytes, &manifest)
            .unwrap()
            .step(0)
            .is_err());
    }
}
