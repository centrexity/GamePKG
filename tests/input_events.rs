use gamepkg::{input_events::Event, manifest::Manifest, runtime::Game};
fn manifest() -> Manifest {
    Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap()
}
fn reader(body: &str) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
(import "gamepkg" "input_events" (func $events(param i32 i32)(result i32)))
(memory(export "memory") 1)
(func(export "gamepkg_tick")(param i32 i64)(result i64) {body}))"#
    ))
    .unwrap()
}
#[test]
fn only_the_signed_input_permission_can_deliver_events() {
    let wasm = reader("i32.const 0 i32.const 16 call $events i64.extend_i32_s");
    let mut m = manifest();
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    game.input_event(Event {
        kind: 1,
        x: 123.0,
        y: 24.0,
        code: 0,
    })
    .unwrap();
    assert_eq!(game.step(0).unwrap(), 16);
    assert_eq!(game.step(0).unwrap(), 0);
    m.permissions.input = false;
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    assert!(game.input_event(Event::RESET).is_err());
    assert_eq!(game.step(0).unwrap(), u64::MAX);
}
#[test]
fn bad_buffers_never_consume_the_valid_following_record_or_escape_guest_memory() {
    for (pointer, capacity) in [
        (-1, 16),
        (65530, 16),
        (0, 0),
        (0, 15),
        (0, 17),
        (0, 32769),
        (i32::MAX, 16),
    ] {
        let wasm=reader(&format!("i32.const {pointer} i32.const {capacity} call $events drop i32.const 0 i32.const 16 call $events drop i32.const 0 i32.load i64.extend_i32_u"));
        let mut game = Game::from_bytes(&wasm, &manifest()).unwrap();
        game.input_event(Event {
            kind: 7,
            x: 0.0,
            y: 0.0,
            code: 0x1f600,
        })
        .unwrap();
        assert_eq!(game.step(0).unwrap(), 7);
    }
}
#[test]
fn unread_records_expire_and_keyboard_values_preserve_all_bits() {
    let wasm=reader("local.get 0 if (result i64) i32.const 0 i32.const 16 call $events drop i32.const 12 i32.load i64.extend_i32_u else i64.const 0 end");
    let mut game = Game::from_bytes(&wasm, &manifest()).unwrap();
    game.input_event(Event {
        kind: 5,
        x: 0.0,
        y: 0.0,
        code: 65536 + 511,
    })
    .unwrap();
    assert_eq!(game.step(1).unwrap(), 66047);
    game.input_event(Event {
        kind: 7,
        x: 0.0,
        y: 0.0,
        code: 65,
    })
    .unwrap();
    game.step(0).unwrap();
    // Fresh empty input yields zero bytes, leaving a sentinel record unchanged.
    let wasm = reader("i32.const 0 i32.const 16 call $events i64.extend_i32_u");
    let mut game = Game::from_bytes(&wasm, &manifest()).unwrap();
    game.input_event(Event::RESET).unwrap();
    assert_eq!(game.step(0).unwrap(), 16);
    assert_eq!(game.step(0).unwrap(), 0);
}
