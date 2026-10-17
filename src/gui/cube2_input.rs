//! Additional native keyboard input. Legacy movement/action keys use the mask.
use eframe::egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton, Vec2};

#[derive(Default)]
pub struct Pointer {
    remainder: Vec2,
}
impl Pointer {
    fn wheel(&mut self, unit: MouseWheelUnit, delta: Vec2) -> (i32, i32) {
        let scale = match unit {
            MouseWheelUnit::Point => 1.0 / 40.0,
            MouseWheelUnit::Line => 1.0,
            MouseWheelUnit::Page => 3.0,
        };
        let finite = |n: f32| if n.is_finite() { n } else { 0.0 };
        let raw = Vec2::new(
            // egui/winit expresses horizontal content movement; SDL's wheel
            // direction has the opposite sign (verified with native X events).
            (self.remainder.x - finite(delta.x) * scale).clamp(-16.0, 16.0),
            (self.remainder.y + finite(delta.y) * scale).clamp(-16.0, 16.0),
        );
        let y = raw.y.trunc() as i32;
        let available = 16 - y.abs();
        let x = (raw.x.trunc() as i32).clamp(-available, available);
        self.remainder = Vec2::new(raw.x.fract(), raw.y.fract());
        (x, y)
    }
}

fn scancode(key: Key) -> Option<u32> {
    use Key::*;
    Some(match key {
        B => 5,
        C => 6,
        F => 9,
        G => 10,
        H => 11,
        I => 12,
        J => 13,
        K => 14,
        L => 15,
        M => 16,
        N => 17,
        O => 18,
        P => 19,
        Q => 20,
        T => 23,
        U => 24,
        V => 25,
        X => 27,
        Y => 28,
        Z => 29,
        Num6 => 35,
        Num7 => 36,
        Num8 => 37,
        Num9 => 38,
        Num0 => 39,
        Backspace => 42,
        Minus => 45,
        Equals | Plus => 46,
        OpenBracket | OpenCurlyBracket => 47,
        CloseBracket | CloseCurlyBracket => 48,
        Backslash | Pipe => 49,
        Semicolon | Colon => 51,
        Quote => 52,
        Backtick => 53,
        Comma => 54,
        Period => 55,
        Slash | Questionmark => 56,
        F1 => 58,
        F2 => 59,
        F3 => 60,
        F4 => 61,
        F5 => 62,
        F6 => 63,
        F7 => 64,
        F8 => 65,
        F9 => 66,
        F10 => 67,
        F11 => 68,
        F12 => 69,
        Insert => 73,
        Home => 74,
        PageUp => 75,
        Delete => 76,
        End => 77,
        PageDown => 78,
        ArrowRight => 79,
        ArrowLeft => 80,
        ArrowDown => 81,
        ArrowUp => 82,
        F13 => 104,
        F14 => 105,
        F15 => 106,
        F16 => 107,
        F17 => 108,
        F18 => 109,
        F19 => 110,
        F20 => 111,
        F21 => 112,
        F22 => 113,
        F23 => 114,
        F24 => 115,
        _ => return None,
    })
}
fn modifiers(mods: Modifiers) -> u32 {
    u32::from(mods.shift)
        | (u32::from(mods.ctrl) << 6)
        | (u32::from(mods.alt) << 8)
        | (u32::from(mods.mac_cmd) << 10)
}
pub fn forward(
    game: &mut crate::runtime::Game,
    events: &[Event],
    mods: Modifiers,
    pointer: &mut Pointer,
) {
    game.cube2_modifiers(modifiers(mods));
    for event in events {
        match event {
            Event::Key {
                key,
                physical_key,
                pressed,
                repeat,
                modifiers: mods,
            } => {
                if let Some(code) = scancode(physical_key.unwrap_or(*key)) {
                    game.cube2_key(code, *pressed, *repeat, modifiers(*mods));
                }
            }
            Event::Text(text) | Event::Paste(text) => game.cube2_text(text),
            Event::PointerButton {
                button,
                pressed,
                modifiers: mods,
                ..
            } => {
                let button = match button {
                    PointerButton::Middle => Some(2),
                    PointerButton::Extra1 => Some(4),
                    PointerButton::Extra2 => Some(5),
                    _ => None,
                };
                if let Some(button) = button {
                    game.cube2_mouse_button(button, *pressed, modifiers(*mods));
                }
            }
            Event::MouseWheel {
                unit,
                delta,
                modifiers: mods,
            } => {
                let (x, y) = pointer.wheel(*unit, *delta);
                if x != 0 || y != 0 {
                    game.cube2_wheel(x, y, modifiers(*mods));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wheel_preserves_fractions_rejects_nonfinite_values_and_bounds_bursts() {
        let mut pointer = Pointer::default();
        assert_eq!(
            pointer.wheel(MouseWheelUnit::Point, Vec2::new(0.0, 20.0)),
            (0, 0)
        );
        assert_eq!(
            pointer.wheel(MouseWheelUnit::Point, Vec2::new(0.0, 20.0)),
            (0, 1)
        );
        assert_eq!(
            pointer.wheel(MouseWheelUnit::Line, Vec2::new(-2.0, 3.0)),
            (2, 3)
        );
        assert_eq!(
            pointer.wheel(MouseWheelUnit::Page, Vec2::new(0.0, -1.0)),
            (0, -3)
        );
        assert_eq!(
            pointer.wheel(MouseWheelUnit::Line, Vec2::new(f32::NAN, f32::INFINITY)),
            (0, 0)
        );
        assert_eq!(
            pointer.wheel(MouseWheelUnit::Line, Vec2::new(f32::MAX, -f32::MAX)),
            (0, -16)
        );
        assert_eq!(pointer.wheel(MouseWheelUnit::Line, Vec2::ZERO), (0, 0));
    }
    #[test]
    fn extra_keys_preserve_legacy_controls_and_use_sdl_codes() {
        assert_eq!(scancode(Key::T), Some(23));
        assert_eq!(scancode(Key::F12), Some(69));
        assert_eq!(scancode(Key::Backspace), Some(42));
        for key in [
            Key::W,
            Key::A,
            Key::S,
            Key::D,
            Key::E,
            Key::R,
            Key::Enter,
            Key::Space,
            Key::Tab,
            Key::Escape,
            Key::Num1,
        ] {
            assert_eq!(scancode(key), None, "duplicate legacy key {key:?}");
        }
        assert_eq!(
            modifiers(Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            }),
            65
        );
    }
}
