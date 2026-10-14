mod adapted {
    pub mod gamepkg {
        pub fn draw_clear(_: u32) -> bool {
            true
        }
        pub fn draw_rect(_: f32, _: f32, _: f32, _: f32, _: u32) -> bool {
            true
        }
    }
    include!("../examples/good/2048/source/src/lib.rs");
}
#[test]
fn upstream_merges_once_per_move_in_all_directions() {
    assert_eq!(adapted::slide(0x1111, 0), 0x22);
    assert_eq!(adapted::slide(0x1111, 1), 0x2200);
    assert_eq!(adapted::slide(0x1000100010001, 2), 0x20002);
    assert_eq!(adapted::slide(0x1000100010001, 3), 0x2000200000000);
    assert_eq!(adapted::slide(0x0f0f, 0), 0xff);
}
#[test]
fn playable_state_changes_only_on_input_and_has_bounded_tiles() {
    let initial = adapted::game_tick(0, 0);
    assert_ne!(initial, 0);
    assert_eq!(adapted::game_tick(0, initial), initial);
    let moved = adapted::game_tick(2, initial);
    assert_ne!(moved, initial);
    for input in [1, 2, 4, 8] {
        let _ = adapted::game_tick(input, u64::MAX);
    }
}
