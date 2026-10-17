//! Ordered native keyboard/text events; controllers retain the action mask.
use eframe::egui::{Event, Modifiers, MouseWheelUnit, PointerButton, Vec2};

#[derive(Default)]
pub struct Desktop {
    keys: std::collections::HashSet<egui_winit::winit::keyboard::KeyCode>,
}
impl Desktop {
    fn modifiers(&self) -> u32 {
        use egui_winit::winit::keyboard::KeyCode::*;
        [
            (ShiftLeft, 1),
            (ShiftRight, 2),
            (ControlLeft, 64),
            (ControlRight, 128),
            (AltLeft, 256),
            (AltRight, 512),
            (SuperLeft, 1024),
            (SuperRight, 2048),
        ]
        .iter()
        .filter(|(key, _)| self.keys.contains(key))
        .fold(0, |mods, (_, bit)| mods | bit)
    }
    pub fn forward(
        &mut self,
        game: &mut crate::runtime::Game,
        events: &[egui_winit::gamepkg_input::Event],
        captured: bool,
    ) -> anyhow::Result<()> {
        use egui_winit::gamepkg_input::Event as Native;
        if !captured {
            self.keys.clear();
            game.cube2_clear_input();
            return Ok(());
        }
        for event in events {
            match event {
                Native::Key {
                    code,
                    pressed,
                    repeat,
                } => {
                    if *pressed {
                        self.keys.insert(*code);
                    } else {
                        self.keys.remove(code);
                    }
                    if let Some(scan) = scancode(*code) {
                        game.cube2_key(scan, *pressed, *repeat, self.modifiers());
                    }
                }
                Native::Text(text) => game.cube2_text(text),
                Native::Focus(false) => {
                    self.keys.clear();
                    game.cube2_clear_input();
                }
                Native::Overflow => {
                    self.keys.clear();
                    game.cube2_clear_input();
                    anyhow::bail!("desktop input exceeded its bounded queue");
                }
                _ => {}
            }
        }
        game.cube2_modifiers(self.modifiers());
        Ok(())
    }
}

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

fn scancode(key: egui_winit::winit::keyboard::KeyCode) -> Option<u32> {
    use egui_winit::winit::keyboard::KeyCode::*;
    Some(match key {
        KeyA => 4,
        KeyB => 5,
        KeyC => 6,
        KeyD => 7,
        KeyE => 8,
        KeyF => 9,
        KeyG => 10,
        KeyH => 11,
        KeyI => 12,
        KeyJ => 13,
        KeyK => 14,
        KeyL => 15,
        KeyM => 16,
        KeyN => 17,
        KeyO => 18,
        KeyP => 19,
        KeyQ => 20,
        KeyR => 21,
        KeyS => 22,
        KeyT => 23,
        KeyU => 24,
        KeyV => 25,
        KeyW => 26,
        KeyX => 27,
        KeyY => 28,
        KeyZ => 29,
        Digit1 => 30,
        Digit2 => 31,
        Digit3 => 32,
        Digit4 => 33,
        Digit5 => 34,
        Digit6 => 35,
        Digit7 => 36,
        Digit8 => 37,
        Digit9 => 38,
        Digit0 => 39,
        Enter => 40,
        Escape => 41,
        Backspace => 42,
        Tab => 43,
        Space => 44,
        Minus => 45,
        Equal => 46,
        BracketLeft => 47,
        BracketRight => 48,
        Backslash => 49,
        Semicolon => 51,
        Quote => 52,
        Backquote => 53,
        Comma => 54,
        Period => 55,
        Slash => 56,
        CapsLock => 57,
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
        PrintScreen => 70,
        ScrollLock => 71,
        Pause => 72,
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
        NumLock => 83,
        NumpadDivide => 84,
        NumpadMultiply => 85,
        NumpadSubtract => 86,
        NumpadAdd => 87,
        NumpadEnter => 88,
        Numpad1 => 89,
        Numpad2 => 90,
        Numpad3 => 91,
        Numpad4 => 92,
        Numpad5 => 93,
        Numpad6 => 94,
        Numpad7 => 95,
        Numpad8 => 96,
        Numpad9 => 97,
        Numpad0 => 98,
        NumpadDecimal => 99,
        IntlBackslash => 100,
        ContextMenu => 101,
        Power => 102,
        NumpadEqual => 103,
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
        Open => 116,
        Help => 117,
        Select => 119,
        Again => 121,
        Undo => 122,
        Cut => 123,
        Copy => 124,
        Paste => 125,
        Find => 126,
        AudioVolumeMute => 127,
        AudioVolumeUp => 128,
        AudioVolumeDown => 129,
        NumpadComma => 133,
        IntlRo => 135,
        KanaMode => 136,
        IntlYen => 137,
        Convert => 138,
        NonConvert => 139,
        Lang1 => 144,
        Lang2 => 145,
        Lang3 => 146,
        Lang4 => 147,
        Lang5 => 148,
        Abort => 155,
        NumpadParenLeft => 182,
        NumpadParenRight => 183,
        NumpadBackspace => 187,
        NumpadMemoryStore => 208,
        NumpadMemoryRecall => 209,
        NumpadMemoryClear => 210,
        NumpadMemoryAdd => 211,
        NumpadMemorySubtract => 212,
        NumpadClear => 216,
        NumpadClearEntry => 217,
        ControlLeft => 224,
        ShiftLeft => 225,
        AltLeft => 226,
        SuperLeft => 227,
        ControlRight => 228,
        ShiftRight => 229,
        AltRight => 230,
        SuperRight => 231,
        NumpadHash => 204,
        Props => 118,
        Hiragana => 147,
        Katakana => 146,
        MediaTrackNext => 258,
        MediaTrackPrevious => 259,
        MediaStop => 260,
        MediaPlayPause => 261,
        MediaSelect => 263,
        LaunchMail => 265,
        LaunchApp2 => 266,
        LaunchApp1 => 267,
        BrowserSearch => 268,
        BrowserHome => 269,
        BrowserBack => 270,
        BrowserForward => 271,
        BrowserStop => 272,
        BrowserRefresh => 273,
        BrowserFavorites => 274,
        Eject => 281,
        Sleep => 282,

        _ => return None,
    })
}

fn modifiers(mods: Modifiers) -> u32 {
    u32::from(mods.shift)
        | (u32::from(mods.ctrl) << 6)
        | (u32::from(mods.alt) << 8)
        | (u32::from(mods.mac_cmd) << 10)
}
pub fn forward(game: &mut crate::runtime::Game, events: &[Event], pointer: &mut Pointer) {
    for event in events {
        match event {
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
                key: eframe::egui::Key::Escape,
                physical_key: Some(eframe::egui::Key::Escape),
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
        assert_eq!(
            scancode(egui_winit::winit::keyboard::KeyCode::KeyT),
            Some(23)
        );
        assert_eq!(
            scancode(egui_winit::winit::keyboard::KeyCode::F12),
            Some(69)
        );
        assert_eq!(
            scancode(egui_winit::winit::keyboard::KeyCode::Backspace),
            Some(42)
        );
        for key in [
            egui_winit::winit::keyboard::KeyCode::KeyW,
            egui_winit::winit::keyboard::KeyCode::KeyA,
            egui_winit::winit::keyboard::KeyCode::KeyS,
            egui_winit::winit::keyboard::KeyCode::KeyD,
            egui_winit::winit::keyboard::KeyCode::KeyE,
            egui_winit::winit::keyboard::KeyCode::KeyR,
            egui_winit::winit::keyboard::KeyCode::Enter,
            egui_winit::winit::keyboard::KeyCode::Space,
            egui_winit::winit::keyboard::KeyCode::Tab,
            egui_winit::winit::keyboard::KeyCode::Escape,
            egui_winit::winit::keyboard::KeyCode::Digit1,
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
    #[test]
    fn physical_keypad_pause_locks_and_modifier_sides_are_distinct() {
        use egui_winit::winit::keyboard::KeyCode::*;
        for (key, expected) in [
            (Pause, 72),
            (PrintScreen, 70),
            (CapsLock, 57),
            (NumLock, 83),
            (Numpad1, 89),
            (Digit1, 30),
            (NumpadEnter, 88),
            (Enter, 40),
            (NumpadDecimal, 99),
            (Delete, 76),
            (IntlBackslash, 100),
            (IntlRo, 135),
            (ControlLeft, 224),
            (ControlRight, 228),
            (ShiftLeft, 225),
            (ShiftRight, 229),
        ] {
            assert_eq!(scancode(key), Some(expected), "{key:?}");
        }
        let mut desktop = Desktop::default();
        desktop
            .keys
            .extend([ControlLeft, ControlRight, ShiftRight, AltRight]);
        assert_eq!(desktop.modifiers(), 64 | 128 | 2 | 512);
        desktop.keys.remove(&ControlLeft);
        assert_eq!(desktop.modifiers(), 128 | 2 | 512);
        desktop.keys.clear();
        assert_eq!(desktop.modifiers(), 0);
        assert_eq!(scancode(Fn), None);
        for (key, expected) in [
            (MediaTrackNext, 258),
            (MediaPlayPause, 261),
            (BrowserBack, 270),
            (BrowserFavorites, 274),
            (LaunchMail, 265),
            (LaunchApp1, 267),
            (LaunchApp2, 266),
            (NumpadHash, 204),
            (Hiragana, 147),
            (Katakana, 146),
        ] {
            assert_eq!(scancode(key), Some(expected), "{key:?}");
        }
    }
}
