use gamepkg::{manifest::Manifest, runtime::Game};
fn fixture(component: bool, spin: bool) -> (Vec<u8>, Manifest) {
    let mut manifest =
        Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
    manifest.permissions.audio = true;
    let (namespace, mono, stereo, entry) = if component {
        manifest.runtime_profile = "component-v1".into();
        (
            "gamepkg:engine/host@0.1.0",
            "audio-submit",
            "audio-submit-stereo",
            "tick",
        )
    } else {
        (
            "gamepkg",
            "audio_submit",
            "audio_submit_stereo",
            "gamepkg_tick",
        )
    };
    // Canonical list parameters are pointer/count; boolean success is 1 versus core 0.
    let ok = if component { 1 } else { 0 };
    let denied = if component { 0 } else { -1 };
    let body = if spin {
        "(loop $forever i32.const 0 i32.const 0 call $stereo drop br $forever) i64.const 1"
            .to_owned()
    } else {
        format!(
            r#"
 local.get 0 i32.const 2 i32.eq if
  i32.const 0 i32.const 2 call $mono i32.const {ok} i32.ne if unreachable end
  i32.const 0 i32.const 2 call $stereo i32.const {denied} i32.ne if unreachable end
  i64.const 2 return end
 local.get 0 i32.const 3 i32.eq if
  i32.const -1 i32.const 2 call $stereo i32.const {denied} i32.ne if unreachable end
  i32.const 65535 i32.const 2 call $stereo drop
  i64.const 3 return end
 local.get 0 i32.const 5 i32.eq if
  i32.const 0 i32.const 3 call $stereo i32.const {denied} i32.ne if unreachable end
  i32.const 0 i32.const 0 call $mono i32.const {ok} i32.ne if unreachable end
  i32.const 0 i32.const 4 call $stereo i32.const {ok} i32.ne if unreachable end
  i32.const 0 i32.const 2 call $mono i32.const {denied} i32.ne if unreachable end
  i64.const 5 return end
 local.get 0 i32.const 4 i32.eq if
  i32.const 0 i32.const 1600 call $stereo i32.const {ok} i32.ne if unreachable end
  i32.const 0 i32.const 2 call $stereo i32.const {denied} i32.ne if unreachable end
  i64.const 4 return end
 i32.const 0 i32.const 4 call $stereo drop
 local.get 0 i32.const 1 i32.eq if unreachable end
 i64.const 1"#
        )
    };
    let module=wat::parse_str(format!(r#"(module(import "{namespace}" "{mono}"(func $mono(param i32 i32)(result i32)))(import "{namespace}" "{stereo}"(func $stereo(param i32 i32)(result i32)))(memory(export "memory") 1)(data(i32.const 0)"\ff\7f\00\80\00\40\00\c0")(func(export "{entry}")(param i32 i64)(result i64){body}))"#)).unwrap();
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
fn stereo_channels_permission_frame_reset_and_rollback() {
    for component in [false, true] {
        let (bytes, mut manifest) = fixture(component, false);
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        game.step(0).unwrap();
        assert_eq!(game.audio_frame(), [32767, -32768, 16384, -16384]);
        assert_eq!(game.audio_channels(), 2);
        game.step(2).unwrap();
        assert_eq!(game.audio_frame(), [32767, -32768]);
        assert_eq!(game.audio_channels(), 1);
        assert!(game.step(1).is_err());
        assert!(game.audio_frame().is_empty());
        assert_eq!(game.audio_channels(), 1);
        manifest.permissions.audio = false;
        let mut denied = Game::from_bytes(&bytes, &manifest).unwrap();
        denied.step(0).unwrap();
        assert!(denied.audio_frame().is_empty());
        assert_eq!(denied.audio_channels(), 1);
    }
}
#[test]
fn stereo_uses_existing_total_sample_budget() {
    for component in [false, true] {
        let (bytes, manifest) = fixture(component, false);
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        game.step(4).unwrap();
        assert_eq!(game.audio_frame().len(), 1600);
        assert_eq!(game.audio_channels(), 2);
    }
}
#[test]
fn infinite_stereo_calls_keep_deadlines() {
    for component in [false, true] {
        let (bytes, manifest) = fixture(component, true);
        assert!(Game::from_bytes(&bytes, &manifest)
            .unwrap()
            .step(0)
            .is_err());
    }
}

#[test]
fn invalid_stereo_is_atomic_and_empty_mono_does_not_select_format() {
    for component in [false, true] {
        let (bytes, manifest) = fixture(component, false);
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        game.step(5).unwrap();
        assert_eq!(game.audio_frame(), [32767, -32768, 16384, -16384]);
        assert_eq!(game.audio_channels(), 2);
        let isolated = Game::from_bytes(&bytes, &manifest).unwrap();
        assert!(isolated.audio_frame().is_empty());
        assert_eq!(isolated.audio_channels(), 1);
    }
}

#[test]
fn invalid_guest_memory_never_publishes_partial_audio() {
    for component in [false, true] {
        let (bytes, manifest) = fixture(component, false);
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        // Canonical ABI rejects invalid lists before entering the host function;
        // the core ABI reports false and lets the guest continue.
        assert_eq!(game.step(3).is_err(), component);
        assert!(game.audio_frame().is_empty());
        assert_eq!(game.audio_channels(), 1);
    }
}
