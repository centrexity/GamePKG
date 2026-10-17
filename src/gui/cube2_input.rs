//! Ordered native keyboard/text events; controllers retain the action mask.
use eframe::egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton, Vec2};

/// Preserve the window's physical-pixel aspect ratio within the bridge budget.
pub fn resolution(size: Vec2, pixels_per_point: f32, limit: Option<(u32, u32)>) -> (u32, u32) {
    let size = size * pixels_per_point;
    if !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
        return (640, 480);
    }
    let (width, height) = limit.unwrap_or((10000, 10000));
    let scale = (width as f32 / size.x).min(height as f32 / size.y).min(1.0);
    (
        (size.x * scale).round().clamp(320.0, 10000.0) as u32,
        (size.y * scale).round().clamp(200.0, 10000.0) as u32,
    )
}

pub fn release_requested(input: &eframe::egui::InputState) -> bool {
    (input.modifiers.ctrl && input.modifiers.alt)
        || input.events.iter().any(|event| matches!(event,Event::Key { pressed:true,modifiers,.. } if modifiers.ctrl && modifiers.alt))
}

/// The real desktop window owns pointer capture; the isolated engine draws its
/// own menu cursor. Escape belongs to the game, never to capture management.
#[derive(Default)]
pub struct MouseCapture {
    focused: bool,
    released: bool,
    captured: bool,
}
impl MouseCapture {
    pub fn update(&mut self, focused: bool, release: bool, clicked: bool) -> (bool, bool) {
        let previous = self.captured;
        if focused && !self.focused {
            self.released = false;
        }
        if release {
            self.released = true;
        } else if focused && clicked {
            self.released = false;
        }
        self.focused = focused;
        self.captured = focused && !self.released;
        (self.captured, self.captured && !previous)
    }
    pub fn captured(&self) -> bool {
        self.captured
    }
    pub fn release(&mut self, ctx: &eframe::egui::Context, confined: bool) {
        *self = Self::default();
        Self::apply(ctx, false, confined);
    }
    pub fn apply(ctx: &eframe::egui::Context, captured: bool, confined: bool) {
        use eframe::egui::{CursorGrab, CursorIcon, ViewportCommand};
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(if captured && confined {
            CursorGrab::Confined
        } else if captured {
            CursorGrab::Locked
        } else {
            CursorGrab::None
        }));
        if captured && confined {
            // X11/Windows use confinement plus recentering, like SDL's fallback.
            // Camera input uses raw MouseMoved events, not warped positions.
            if let Some(rect) = ctx.input(|i| i.viewport().inner_rect) {
                ctx.send_viewport_cmd(ViewportCommand::CursorPosition(rect.size().to_pos2() * 0.5));
            }
        }
        ctx.send_viewport_cmd(ViewportCommand::CursorVisible(!captured));
        // egui's per-frame cursor output can otherwise override CursorVisible.
        ctx.set_cursor_icon(if captured {
            CursorIcon::None
        } else {
            CursorIcon::Default
        });
    }
}

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
        A => 4,
        D => 7,
        E => 8,
        R => 21,
        S => 22,
        W => 26,
        Num1 => 30,
        Num2 => 31,
        Num3 => 32,
        Num4 => 33,
        Num5 => 34,
        Enter => 40,
        Escape => 41,
        Tab => 43,
        Space => 44,
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
    fn resolution_uses_physical_pixels_and_preserves_widescreen_aspect() {
        assert_eq!(resolution(Vec2::new(960.0, 720.0), 1.0, None), (960, 720));
        assert_eq!(
            resolution(Vec2::new(1920.0, 1080.0), 1.0, None),
            (1920, 1080)
        );
        assert_eq!(resolution(Vec2::new(960.0, 540.0), 2.0, None), (1920, 1080));
        assert_eq!(
            resolution(Vec2::new(3840.0, 2160.0), 1.0, None),
            (3840, 2160)
        );
        assert_eq!(
            resolution(Vec2::new(1920.0, 1080.0), 1.0, Some((1280, 960))),
            (1280, 720)
        );
        assert_eq!(
            resolution(Vec2::new(640.0, 480.0), 1.0, Some((1920, 1080))),
            (640, 480)
        );
        assert_eq!(resolution(Vec2::new(f32::NAN, 1.0), 1.0, None), (640, 480));
    }
    #[test]
    fn release_shortcut_keeps_event_modifiers_after_keys_are_released() {
        let ctx = eframe::egui::Context::default();
        let raw = eframe::egui::RawInput {
            events: vec![Event::Key {
                key: Key::Escape,
                physical_key: Some(Key::Escape),
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        let mut release = false;
        let _ = ctx.run(raw, |ctx| release = ctx.input(release_requested));
        assert!(release);
    }
    #[test]
    fn capture_follows_focus_and_explicit_release_without_stealing_escape() {
        let mut capture = MouseCapture::default();
        assert_eq!(capture.update(true, false, false), (true, true));
        // Opening the original menu does not change focus or capture.
        assert_eq!(capture.update(true, false, false), (true, false));
        assert_eq!(capture.update(false, false, false), (false, false));
        assert_eq!(capture.update(true, false, false), (true, true));
        assert_eq!(capture.update(true, true, false), (false, false));
        assert_eq!(capture.update(true, false, false), (false, false));
        assert_eq!(capture.update(true, false, true), (true, true));
        assert_eq!(capture.update(true, true, true), (false, false));
    }
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
    fn desktop_keys_preserve_physical_sdl_codes_including_menu_and_movement_keys() {
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
            assert!(
                scancode(key).is_some(),
                "desktop key must carry ordered events: {key:?}"
            );
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
