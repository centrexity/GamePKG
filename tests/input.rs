use gamepkg::input_buttons as input;
#[test]
fn portable_buttons_emit_edges_and_held_bits_without_reserved_bits() {
    let buttons = [
        input::LEFT,
        input::RIGHT,
        input::UP,
        input::DOWN,
        input::A,
        input::B,
        input::X,
        input::Y,
        input::L,
        input::R,
        input::START,
        input::SELECT,
    ];
    for button in buttons {
        let first = input::snapshot(button, 0);
        assert!(input::pressed(first, button));
        assert!(input::held(first, button));
        let continuing = input::snapshot(button, button);
        assert!(!input::pressed(continuing, button));
        assert!(input::held(continuing, button));
        assert_eq!(input::snapshot(0, button), 0);
    }
    let simultaneous = input::snapshot(input::LEFT | input::B, 0);
    assert!(input::held(simultaneous, input::LEFT));
    assert!(input::pressed(simultaneous, input::B));
    assert_eq!(
        input::snapshot(u32::MAX, 0),
        input::PRESSED_MASK | input::HELD_MASK
    );
    assert_eq!(input::held_bits(!input::PRESSED_MASK), 0);
    assert!(!input::held(0, u32::MAX));
    assert!(!input::pressed(0, u32::MAX));
}
