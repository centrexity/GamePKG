use gamepkg::{manifest::Manifest, runtime::Game};

fn fixture(component: bool, allowed: bool) -> (Vec<u8>, Manifest) {
    let mut manifest =
        Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
    manifest.permissions.input = allowed;
    let (namespace, name, entry) = if component {
        manifest.runtime_profile = "component-v1".into();
        ("gamepkg:engine/host@0.1.0", "window-control", "tick")
    } else {
        ("gamepkg", "window_control", "gamepkg_tick")
    };
    let bytes = wat::parse_str(format!(
        r#"(module
        (import "{namespace}" "{name}" (func $window(param i32 i32)(result i32)))
        (memory(export "memory")1)
        (func $read(result i32)
          (i32.or(call $window(i32.const 9)(i32.const 0))
            (i32.shl(call $window(i32.const 9)(i32.const 1))(i32.const 1))))
        (func(export "{entry}")(param $input i32)(param i64)(result i64)
          (local $before i32)(local $accepted i32)
          (if(i32.eq(local.get $input)(i32.const 64))
            (then(loop $forever call $read drop br $forever)))
          (local.set $before(call $read))
          (if(i32.or(i32.eq(local.get $input)(i32.const 4))
            (i32.or(i32.eq(local.get $input)(i32.const 1024))(i32.eq(local.get $input)(i32.const 128))))
            (then(local.set $accepted(call $window(i32.const 10)(i32.const 0)))))
          (if(i32.eq(local.get $input)(i32.const 8))
            (then(local.set $accepted(call $window(i32.const 10)(i32.const 1)))))
          (if(i32.ne(local.get $before)(call $read))(then unreachable))
          (if(i32.eq(local.get $input)(i32.const 128))(then unreachable))
          (i64.extend_i32_u(i32.or(local.get $before)(i32.shl(local.get $accepted)(i32.const 2))))))"#
    ))
    .unwrap();
    (
        if component {
            gamepkg::hostapi::componentize(bytes).unwrap()
        } else {
            bytes
        },
        manifest,
    )
}

#[test]
fn vsync_preferences_commit_only_on_successful_gestures_in_both_wasm_profiles() {
    for component in [false, true] {
        let (bytes, manifest) = fixture(component, true);
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        assert_eq!(game.step(4).unwrap(), 0); // No persistent desktop host.
        assert_eq!(game.window_state().vsync, None);
        game.observe_window_vsync(Some(true));
        assert_eq!(game.step(0).unwrap(), 3);
        assert_eq!(game.step(1024).unwrap(), 3); // Held input is not a gesture.
        assert_eq!(game.step(4).unwrap(), 7); // Reads the old preference within tick.
        assert_eq!(game.window_state().vsync, Some(false));
        assert_eq!(game.step(0).unwrap(), 1);
        assert_eq!(game.step(8).unwrap(), 5);
        assert_eq!(game.step(0).unwrap(), 3);
        let before = game.window_state();
        assert!(game.step(128).is_err());
        assert_eq!(game.window_state(), before);
        let mut other = Game::from_bytes(&bytes, &manifest).unwrap();
        other.observe_window_vsync(Some(false));
        assert_eq!(other.step(0).unwrap(), 1);
        other.observe_window_vsync(None);
        assert_eq!(other.step(0).unwrap(), 0);
        let (bytes, denied) = fixture(component, false);
        let mut game = Game::from_bytes(&bytes, &denied).unwrap();
        game.observe_window_vsync(Some(true));
        assert_eq!(game.step(4).unwrap(), 0);
        assert_eq!(game.window_state().vsync, None);
    }
}

#[test]
fn repeated_vsync_preference_reads_still_enforce_execution_deadlines() {
    for component in [false, true] {
        let (bytes, manifest) = fixture(component, true);
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        game.observe_window_vsync(Some(true));
        assert!(game.step(64).is_err());
        assert_eq!(game.window_state().vsync, None);
    }
}
