use gamepkg::{manifest::Manifest, runtime::Game};
fn manifest() -> Manifest {
    Manifest::parse(
        &std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/minimal/manifest.toml"
        ))
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn audio_permission_and_trap_rollback_are_enforced() {
    let wasm = wat::parse_str(
        r#"(module
        (import "gamepkg" "audio_submit" (func $audio (param i32 i32) (result i32)))
        (memory (export "memory") 1)
        (data (i32.const 0) "\00\40\00\c0")
        (func (export "gamepkg_tick") (param i32 i64) (result i64)
            i32.const 0 i32.const 2 call $audio drop
            local.get 0 if unreachable end
            i64.const 1))"#,
    )
    .unwrap();
    let mut m = manifest();
    let mut denied = Game::from_bytes(&wasm, &m).unwrap();
    denied.step(0).unwrap();
    assert!(denied.audio_frame().is_empty());
    m.permissions.audio = true;
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    game.step(0).unwrap();
    assert_eq!(game.audio_frame(), &[16384, -16384]);
    game.step(0).unwrap();
    assert_eq!(game.audio_frame().len(), 2);
    assert!(game.step(1).is_err());
    assert!(game.audio_frame().is_empty());
}
#[test]
fn invalid_audio_memory_and_total_sample_budget_are_rejected() {
    let wasm = wat::parse_str(
        r#"(module
        (import "gamepkg" "audio_submit" (func $audio (param i32 i32) (result i32)))
        (memory (export "memory") 1)
        (func (export "gamepkg_tick") (param i32 i64) (result i64)
            i32.const -1 i32.const 1 call $audio i32.const -1 i32.ne if unreachable end
            i32.const 65535 i32.const 2 call $audio i32.const -1 i32.ne if unreachable end
            i32.const 0 i32.const 1601 call $audio i32.const -1 i32.ne if unreachable end
            i32.const 0 i32.const 1600 call $audio i32.const 0 i32.ne if unreachable end
            i32.const 0 i32.const 1 call $audio i32.const -1 i32.ne if unreachable end
            i64.const 1))"#,
    )
    .unwrap();
    let mut m = manifest();
    m.permissions.audio = true;
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    game.step(0).unwrap();
    assert_eq!(game.audio_frame().len(), 1600);
}
#[test]
fn repeated_audio_calls_are_metered() {
    let wasm = wat::parse_str(
        r#"(module
        (import "gamepkg" "audio_submit" (func $audio (param i32 i32) (result i32)))
        (memory (export "memory") 1)
        (func (export "gamepkg_tick") (param i32 i64) (result i64)
            (loop i32.const 0 i32.const 0 call $audio drop br 0) i64.const 0))"#,
    )
    .unwrap();
    let mut m = manifest();
    m.permissions.audio = true;
    m.limits.fuel = 1000;
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    assert!(game.step(0).is_err());
    assert!(game.audio_frame().is_empty());
}
