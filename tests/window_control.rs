use gamepkg::{
    manifest::{Manifest, UserFile},
    runtime::Game,
    window_control::State,
};
fn manifest(input: bool) -> Manifest {
    let mut m = Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
    m.permissions.input = input;
    m
}
fn module() -> Vec<u8> {
    wat::parse_str(r#"(module
(import "gamepkg" "window_control"(func $window(param i32 i32)(result i32)))
(func(export "gamepkg_tick")(param $input i32)(param i64)(result i64)
(drop(call $window(i32.const 0)(i32.const 0)))
(drop(call $window(i32.const 1)(i32.const 1)))
(if(i32.eq(local.get $input)(i32.const 99))(then(drop(call $window(i32.const 2)(i32.const 0)))unreachable))
(if(i32.eq(local.get $input)(i32.const 8))(then(return(i64.extend_i32_s(call $window(i32.const 3)(i32.const 0))))))
(i64.extend_i32_s(call $window(i32.const 2)(i32.const 0)))))"#).unwrap()
}
#[test]
fn window_state_is_virtual_scoped_and_requires_fresh_actions_for_fullscreen_and_close() {
    let mut game = Game::from_bytes(&module(), &manifest(true)).unwrap();
    assert_eq!(game.window_state(), State::default());
    assert_eq!(game.step(0).unwrap(), 0);
    assert!(!game.window_state().visible);
    assert!(game.window_state().confined);
    assert_eq!(game.step(4).unwrap(), 1);
    assert!(game.window_state().fullscreen_toggle);
    assert_eq!(game.step(4 << 8).unwrap(), 0);
    assert!(
        game.window_state().fullscreen_toggle,
        "held input authorized another toggle"
    );
    let previous = game.window_state();
    assert!(game.step(99).is_err());
    assert_eq!(
        game.window_state(),
        previous,
        "trapped tick changed window state"
    );
    assert_eq!(game.step(8).unwrap(), 1);
    assert!(game.exited());
    let mut denied = Game::from_bytes(&module(), &manifest(false)).unwrap();
    assert_eq!(denied.step(4).unwrap(), 0);
    assert_eq!(denied.window_state(), State::default());
    assert!(!denied.exited());
}
#[test]
fn closing_a_window_discards_file_requests_before_any_picker_can_open() {
    let bytes = wat::parse_str(
        r#"(module
(import "gamepkg" "window_control"(func $window(param i32 i32)(result i32)))
(import "gamepkg" "file_call"(func $file(param i32 i32 i32 i32 i32)(result i32)))
(memory(export "memory")1)
(func(export "gamepkg_tick")(param i32 i64)(result i64)
(drop(call $file(i32.const 0)(i32.const 0)(i32.const 4)(i32.const 64)(i32.const 4)))
(i64.extend_i32_s(call $window(i32.const 3)(i32.const 0)))))"#,
    )
    .unwrap();
    let mut m = manifest(true);
    m.permissions.files = vec![UserFile {
        id: "files".into(),
        title: "Text".into(),
        extensions: vec!["txt".into()],
        max_bytes: 1024,
        read: true,
        write: false,
    }];
    let mut game = Game::from_bytes(&bytes, &m).unwrap();
    game.enable_user_files();
    assert_eq!(game.step(4).unwrap(), 1);
    assert!(game.exited());
    assert!(game.take_user_file_job().is_none());
}
#[test]
fn only_edges_authorize_host_window_actions() {
    use gamepkg::input_events::{gesture, Event};
    for held in [0, 1 << 8, 1 << 9, 1 << 20, 1 << 23] {
        assert!(!gesture(&[], held));
    }
    for pressed in [1, 4, 128, 1 << 16, 1 << 19] {
        assert!(gesture(&[], pressed));
    }
    for (kind, code, allowed) in [
        (5, 68, true),
        (5, 68 | 65536, false),
        (9, 113, true),
        (9, 113 | 0x80000000, false),
        (6, 68, false),
        (2, 1, true),
        (1, 1, false),
    ] {
        assert_eq!(
            gesture(
                &[Event {
                    kind,
                    x: 0.0,
                    y: 0.0,
                    code
                }],
                0
            ),
            allowed
        );
    }
}
