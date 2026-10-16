//! Portable button input shared by keyboards, controllers and touch controls.
//! The host supplies the tick's u32 value; this crate opens no devices.

pub const LEFT: u32 = 1;
pub const RIGHT: u32 = 1 << 1;
pub const UP: u32 = 1 << 2;
pub const DOWN: u32 = 1 << 3;
pub const B: u32 = 1 << 4;
pub const Y: u32 = 1 << 5;
pub const START: u32 = 1 << 6;
pub const SELECT: u32 = 1 << 7;
pub const A: u32 = 1 << 16;
pub const X: u32 = 1 << 17;
pub const L: u32 = 1 << 18;
pub const R: u32 = 1 << 19;
pub const PRESSED_MASK: u32 = 0x000f00ff;
pub const HELD_MASK: u32 = 0x00f0ff00;

/// True on the tick when any selected button is first pressed.
pub fn pressed(input: u32, buttons: u32) -> bool {
    input & buttons & PRESSED_MASK != 0
}

/// Convert press bits to corresponding held bits; ignore undefined bits.
pub fn held_bits(buttons: u32) -> u32 {
    ((buttons & 0xff) << 8) | ((buttons & 0x000f0000) << 4)
}

/// True while any selected button is held, regardless of input device.
pub fn held(input: u32, buttons: u32) -> bool {
    input & held_bits(buttons) != 0
}

/// Encode a host button snapshot, emitting edges only for newly held buttons.
pub fn snapshot(buttons: u32, previous: u32) -> u32 {
    let buttons = buttons & PRESSED_MASK;
    (buttons & !previous) | held_bits(buttons)
}
