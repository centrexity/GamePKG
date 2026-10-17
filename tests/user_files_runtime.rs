use gamepkg::{
    input_events::Event,
    manifest::{Manifest, UserFile},
    runtime::Game,
};
fn manifest(grant: bool) -> Manifest {
    let mut m = Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
    if grant {
        m.permissions.files = vec![UserFile {
            id: "patterns".into(),
            title: "Pattern".into(),
            extensions: vec!["txt".into()],
            max_bytes: 1024,
            read: true,
            write: true,
        }];
    }
    m
}
fn guest() -> Vec<u8> {
    wat::parse_str(r#"(module
(import "gamepkg" "file_call" (func $file (param i32 i32 i32 i32 i32)(result i32)))
(memory (export "memory") 1)
(func (export "gamepkg_tick") (param $input i32)(param i64)(result i64)
 (if (i32.eq (local.get $input)(i32.const 1)) (then (drop(call $file(i32.const 0)(i32.const 0)(i32.const 4)(i32.const 65535)(i32.const 4)))))
 (if (i32.eq (local.get $input)(i32.const 2)) (then (drop(call $file(i32.const 0)(i32.const 0)(i32.const 4)(i32.const 64)(i32.const 4))) unreachable))
 (i64.extend_i32_s(call $file(i32.const 0)(i32.const 0)(i32.const 4)(i32.const 64)(i32.const 4)))))"#).unwrap()
}
fn gesture(game: &mut Game) {
    game.input_event(Event {
        kind: 9,
        x: 0.0,
        y: 0.0,
        code: 105,
    })
    .unwrap();
}
#[test]
fn raw_abi_checks_memory_before_consuming_a_player_gesture() {
    let mut game = Game::from_bytes(&guest(), &manifest(true)).unwrap();
    gesture(&mut game);
    assert_eq!(
        game.step(0).unwrap(),
        u64::MAX,
        "headless host enabled file dialogs"
    );
    game.enable_user_files();
    assert_eq!(
        game.step(0).unwrap(),
        u64::MAX,
        "old gesture survived a tick"
    );
    gesture(&mut game);
    assert_eq!(game.step(1).unwrap(), 4);
    let job = game.take_user_file_job().unwrap();
    assert_eq!(job.policy.id, "patterns");
    assert!(game.take_user_file_job().is_none());
}
#[test]
fn trapping_ticks_never_commit_file_dialogs_or_reuse_their_handle() {
    let mut game = Game::from_bytes(&guest(), &manifest(true)).unwrap();
    game.enable_user_files();
    gesture(&mut game);
    assert!(game.step(2).is_err());
    assert!(game.take_user_file_job().is_none());
    gesture(&mut game);
    assert_eq!(game.step(0).unwrap(), 4);
    assert_eq!(game.take_user_file_job().unwrap().handle, 2);
}
#[test]
fn unsigned_file_access_is_denied_even_after_host_enablement() {
    let mut game = Game::from_bytes(&guest(), &manifest(false)).unwrap();
    game.enable_user_files();
    gesture(&mut game);
    assert_eq!(game.step(0).unwrap(), u64::MAX);
    assert!(game.take_user_file_job().is_none());
}
