use gamepkg::{manifest::Manifest, runtime::Game};
fn fixture(component: bool, spin: bool) -> (Vec<u8>, Manifest) {
    let mut manifest =
        Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
    let (namespace, clock, entry) = if component {
        manifest.runtime_profile = "component-v1".into();
        ("gamepkg:engine/host@0.1.0", "frame-time-us", "tick")
    } else {
        ("gamepkg", "frame_time_us", "gamepkg_tick")
    };
    let body = if spin {
        "(loop $forever call $clock drop br $forever) i64.const 0".to_owned()
    } else {
        format!("call $clock local.set $first {} local.get $first i64.const 1 i64.shl i64.const 1 i64.or", "call $clock local.get $first i64.ne if unreachable end ".repeat(64))
    };
    let module=wat::parse_str(format!(r#"(module (import "{namespace}" "{clock}" (func $clock (result i64))) (memory (export "memory") 1) (func (export "{entry}") (param i32 i64) (result i64) (local $first i64) {body}))"#)).unwrap();
    let bytes = if component {
        gamepkg::hostapi::componentize(module).unwrap()
    } else {
        module
    };
    (bytes, manifest)
}
#[test]
fn core_and_component_clocks_are_per_game_relative_stable_and_advance_between_ticks() {
    for component in [false, true] {
        let (bytes, mut manifest) = fixture(component, false);
        manifest.permissions.input = false;
        let mut first = Game::from_bytes(&bytes, &manifest).unwrap();
        assert_eq!(first.step(0).unwrap(), 1);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = first.step(0).unwrap() >> 1;
        assert!(
            elapsed >= 10000,
            "clock did not count elapsed time: {elapsed}"
        );
        let mut independent = Game::from_bytes(&bytes, &manifest).unwrap();
        assert_eq!(independent.step(0).unwrap(), 1);
        assert!(first.step(0).unwrap() >> 1 >= elapsed);
    }
}
#[test]
fn reading_sampled_time_cannot_disable_execution_deadlines() {
    for component in [false, true] {
        let (bytes, manifest) = fixture(component, true);
        assert!(Game::from_bytes(&bytes, &manifest)
            .unwrap()
            .step(0)
            .is_err());
    }
}
