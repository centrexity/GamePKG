//! Ordered physical input from the registered, focused source-game viewport.
use crate::input_events::Event;
use eframe::egui;
use egui_winit::{
    gamepkg_input::Event as Native,
    winit::event::{MouseButton, MouseScrollDelta},
};
#[derive(Default)]
pub(super) struct Capture {
    position: Option<egui::Pos2>,
    buttons: u32,
    focused: bool,
}
impl Capture {
    pub fn translate(
        &mut self,
        events: &[Native],
        rect: egui::Rect,
        pixels_per_point: f32,
        focused: bool,
    ) -> Vec<Event> {
        let mut output = Vec::new();
        if !focused
            || rect.width() <= 0.0
            || rect.height() <= 0.0
            || !rect.min.x.is_finite()
            || !rect.min.y.is_finite()
            || !rect.max.x.is_finite()
            || !rect.max.y.is_finite()
            || !pixels_per_point.is_finite()
            || pixels_per_point <= 0.0
        {
            if self.focused || self.buttons != 0 {
                output.push(Event::RESET);
            }
            *self = Self::default();
            return output;
        }
        self.focused = true;
        let mut active = true;
        for event in events {
            if !active && !matches!(event, Native::Focus(_) | Native::Overflow) {
                continue;
            }
            if output.len() >= crate::input_events::MAX_EVENTS {
                self.buttons = 0;
                self.position = None;
                return vec![Event::RESET];
            }
            let event = match event {
                Native::Focus(false) | Native::Overflow => {
                    active = false;
                    self.buttons = 0;
                    self.position = None;
                    Event::RESET
                }
                Native::Focus(true) => {
                    active = true;
                    self.position = None;
                    self.buttons = 0;
                    Event::RESET
                }
                Native::PointerGone => {
                    self.buttons = 0;
                    self.position = None;
                    Event {
                        kind: 8,
                        ..Event::RESET
                    }
                }
                Native::Position(x, y) => {
                    let position =
                        egui::pos2(*x as f32 / pixels_per_point, *y as f32 / pixels_per_point);
                    if !position.x.is_finite() || !position.y.is_finite() {
                        continue;
                    }
                    if !rect.contains(position) && self.buttons == 0 {
                        self.position = None;
                        continue;
                    }
                    let x = ((position.x - rect.min.x) / rect.width() * 640.0).clamp(0.0, 640.0);
                    let y = ((position.y - rect.min.y) / rect.height() * 480.0).clamp(0.0, 480.0);
                    self.position = Some(egui::pos2(x, y));
                    Event {
                        kind: 1,
                        x,
                        y,
                        code: self.buttons,
                    }
                }
                Native::Button { button, pressed } => {
                    let code = match button {
                        MouseButton::Left => 1,
                        MouseButton::Right => 2,
                        MouseButton::Middle => 4,
                        MouseButton::Back => 8,
                        MouseButton::Forward => 16,
                        _ => continue,
                    };
                    let Some(position) = self.position else {
                        continue;
                    };
                    if *pressed {
                        self.buttons |= code;
                    } else {
                        self.buttons &= !code;
                    }
                    Event {
                        kind: if *pressed { 2 } else { 3 },
                        x: position.x,
                        y: position.y,
                        code,
                    }
                }
                Native::Wheel(delta) => {
                    if self.position.is_none() {
                        continue;
                    }
                    let (x, y, code) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (*x, *y, 0),
                        MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32, 1),
                    };
                    if !x.is_finite() || !y.is_finite() {
                        continue;
                    }
                    Event {
                        kind: 4,
                        x: x.clamp(-4096.0, 4096.0),
                        y: y.clamp(-4096.0, 4096.0),
                        code,
                    }
                }
                Native::Key {
                    code,
                    pressed,
                    repeat,
                } => {
                    let Some(scan) = super::cube2_input::scancode(*code) else {
                        continue;
                    };
                    Event {
                        kind: if *pressed { 5 } else { 6 },
                        x: 0.0,
                        y: 0.0,
                        code: scan | if *pressed && *repeat { 1 << 16 } else { 0 },
                    }
                }
                Native::LogicalKey {
                    key,
                    physical,
                    pressed,
                    repeat,
                } => {
                    use egui_winit::winit::keyboard::{Key, NamedKey};
                    let code = match key {
                        Key::Character(text) => {
                            let mut chars = text.chars();
                            let Some(character) = chars.next() else {
                                continue;
                            };
                            if chars.next().is_some() || character == '\0' {
                                continue;
                            }
                            u32::from(character.to_ascii_lowercase())
                        }
                        Key::Named(named) => match named {
                            NamedKey::Space => 32,
                            NamedKey::Enter => 13,
                            NamedKey::Escape => 27,
                            NamedKey::Tab => 9,
                            NamedKey::Backspace => 8,
                            NamedKey::Delete => 127,
                            _ => {
                                let Some(scan) = physical.and_then(super::cube2_input::scancode)
                                else {
                                    continue;
                                };
                                scan | 0x4000_0000
                            }
                        },
                        _ => continue,
                    };
                    Event {
                        kind: if *pressed { 9 } else { 10 },
                        x: 0.0,
                        y: 0.0,
                        code: code | if *pressed && *repeat { 0x8000_0000 } else { 0 },
                    }
                }
                Native::Text(text) => {
                    for character in text.chars().filter(|c| *c != '\0') {
                        if output.len() >= crate::input_events::MAX_EVENTS {
                            self.buttons = 0;
                            self.position = None;
                            return vec![Event::RESET];
                        }
                        output.push(Event {
                            kind: 7,
                            x: 0.0,
                            y: 0.0,
                            code: u32::from(character),
                        });
                    }
                    continue;
                }
                _ => continue,
            };
            output.push(event);
        }
        self.focused = active;
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_winit::winit::keyboard::KeyCode;
    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(1280.0, 960.0))
    }
    #[test]
    fn physical_keys_committed_text_and_pointer_events_keep_order_and_scale() {
        let mut capture = Capture::default();
        let events = [
            Native::Position(1300.0, 1000.0),
            Native::Key {
                code: KeyCode::KeyN,
                pressed: true,
                repeat: false,
            },
            Native::Text("é😀".into()),
            Native::Button {
                button: MouseButton::Left,
                pressed: true,
            },
            Native::Position(1500.0, 1200.0),
            Native::Button {
                button: MouseButton::Left,
                pressed: false,
            },
        ];
        let values = capture.translate(&events, rect(), 2.0, true);
        assert_eq!(
            values.iter().map(|v| v.kind).collect::<Vec<_>>(),
            [1, 5, 7, 7, 2, 1, 3]
        );
        assert_eq!((values[0].x, values[0].y), (320.0, 240.0));
        assert_eq!(values[1].code, 17);
        assert_eq!(values[2].code, 233);
        assert_eq!(values[3].code, 0x1f600);
        assert_eq!(values[5].code, 1);
    }
    #[test]
    fn focus_loss_suppresses_later_text_and_invalid_surfaces_do_not_leak() {
        let mut capture = Capture::default();
        assert!(capture
            .translate(&[Native::Text("outside secret".into())], rect(), 1.0, false)
            .is_empty());
        let events = capture.translate(
            &[
                Native::Focus(false),
                Native::Text("unfocused secret".into()),
            ],
            rect(),
            1.0,
            true,
        );
        assert_eq!(events, [Event::RESET]);
        assert!(capture
            .translate(&[Native::Text("hidden".into())], rect(), 0.0, true)
            .is_empty());
        let overflow = capture.translate(&[Native::Text("x".repeat(4096))], rect(), 1.0, true);
        assert_eq!(overflow, [Event::RESET]);
    }
}

#[cfg(test)]
mod logical_tests {
    use super::*;
    use egui_winit::winit::keyboard::{Key, KeyCode, NamedKey};
    #[test]
    fn logical_shortcuts_follow_layout_and_repeat_bits_preserve_named_keys() {
        let mut capture = Capture::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0));
        let values = capture.translate(
            &[
                Native::Key {
                    code: KeyCode::KeyQ,
                    pressed: true,
                    repeat: false,
                },
                Native::LogicalKey {
                    key: Key::Character("a".into()),
                    physical: Some(KeyCode::KeyQ),
                    pressed: true,
                    repeat: false,
                },
                Native::Text("a".into()),
                Native::LogicalKey {
                    key: Key::Character("R".into()),
                    physical: Some(KeyCode::KeyR),
                    pressed: true,
                    repeat: true,
                },
                Native::LogicalKey {
                    key: Key::Named(NamedKey::ArrowLeft),
                    physical: Some(KeyCode::ArrowLeft),
                    pressed: false,
                    repeat: false,
                },
            ],
            rect,
            1.0,
            true,
        );
        assert_eq!(
            values.iter().map(|e| (e.kind, e.code)).collect::<Vec<_>>(),
            [(5, 20), (9, 97), (7, 97), (9, 0x80000072), (10, 0x40000050)]
        );
    }
}
