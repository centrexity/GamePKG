mod adapted {
    pub mod gamepkg {
        thread_local! {
            static WORDS: std::cell::RefCell<std::collections::VecDeque<u64>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
            static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
        }
        pub fn samples(values: &[u64]) {
            WORDS.with(|words| *words.borrow_mut() = values.iter().copied().collect());
        }
        pub fn random_u64() -> u64 {
            WORDS
                .with(|words| words.borrow_mut().pop_front())
                .unwrap_or_else(|| {
                    NEXT.with(|next| {
                        let value = next.get().wrapping_mul(6364136223846793005).wrapping_add(1);
                        next.set(value);
                        value
                    })
                })
        }
        pub fn draw_clear(_: u32) -> bool {
            true
        }
        pub fn draw_rect(_: f32, _: f32, _: f32, _: f32, _: u32) -> bool {
            true
        }
    }
    include!("../ports/2048/source/src/lib.rs");
    pub fn spawn_test(board: u64) -> u64 {
        spawn(board)
    }
    pub fn bounded_test(bound: u64) -> u64 {
        random_below(bound)
    }
}
#[test]
fn spawns_sample_empty_squares_and_match_original_cli_four_probability() {
    for rank in 0..16 {
        for value in 10..20 {
            adapted::gamepkg::samples(&[value, rank]);
            let expected = if value % 10 == 0 { 2 } else { 1 };
            assert_eq!(adapted::spawn_test(0), expected << (4 * rank));
        }
    }
    // With two empty cells, rank one must select the second empty cell,
    // without scanning from a biased random starting position.
    let board = u64::MAX & !(15u64 << 4) & !(15u64 << 52);
    adapted::gamepkg::samples(&[11, 1]);
    assert_eq!(adapted::spawn_test(board), board | (1u64 << 52));
    assert_eq!(adapted::spawn_test(u64::MAX), u64::MAX);
    adapted::gamepkg::samples(&[0, 5, 6]); // 2^64 % 10 = 6: reject 0 and 5.
    assert_eq!(adapted::bounded_test(10), 6);
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
