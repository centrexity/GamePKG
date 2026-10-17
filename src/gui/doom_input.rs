//! Physical DOOM controls from the focused desktop window.
use crate::{doom_input::Event, runtime::Game};
use anyhow::{bail, Result};
use egui_winit::{
    gamepkg_input::Event as Native,
    winit::{
        event::{MouseButton, MouseScrollDelta},
        keyboard::KeyCode,
    },
};
use std::collections::HashSet;
#[derive(Default)]
pub struct Desktop {
    keys: HashSet<KeyCode>,
    buttons: u8,
    motion: (f64, f64),
    wheel: f64,
    active: bool,
}
fn code(key: KeyCode) -> Option<u8> {
    use KeyCode::*;
    Some(match key {
        KeyA => b'a',
        KeyB => b'b',
        KeyC => b'c',
        KeyD => b'd',
        KeyE => b'e',
        KeyF => b'f',
        KeyG => b'g',
        KeyH => b'h',
        KeyI => b'i',
        KeyJ => b'j',
        KeyK => b'k',
        KeyL => b'l',
        KeyM => b'm',
        KeyN => b'n',
        KeyO => b'o',
        KeyP => b'p',
        KeyQ => b'q',
        KeyR => b'r',
        KeyS => b's',
        KeyT => b't',
        KeyU => b'u',
        KeyV => b'v',
        KeyW => b'w',
        KeyX => b'x',
        KeyY => b'y',
        KeyZ => b'z',
        Digit0 => b'0',
        Digit1 => b'1',
        Digit2 => b'2',
        Digit3 => b'3',
        Digit4 => b'4',
        Digit5 => b'5',
        Digit6 => b'6',
        Digit7 => b'7',
        Digit8 => b'8',
        Digit9 => b'9',
        ArrowLeft => 0xac,
        ArrowRight => 0xae,
        ArrowUp => 0xad,
        ArrowDown => 0xaf,
        Escape => 27,
        Enter | NumpadEnter => 13,
        Tab => 9,
        Space => 32,
        Backspace => 127,
        Minus | NumpadSubtract => b'-',
        Equal | NumpadEqual => b'=',
        Comma => b',',
        Period => b'.',
        Semicolon => b';',
        Quote => b'\'',
        Slash | NumpadDivide => b'/',
        Backslash | IntlBackslash => b'\\',
        Backquote => b'`',
        BracketLeft => b'[',
        BracketRight => b']',
        NumpadAdd => b'+',
        NumpadMultiply => b'*',
        ShiftLeft | ShiftRight => 0xb6,
        ControlLeft | ControlRight => 0x9d,
        AltLeft | AltRight => 0xb8,
        CapsLock => 0xba,
        NumLock => 0xc5,
        ScrollLock => 0xc6,
        PrintScreen => 0xd9,
        Pause => 255,
        Home | Numpad7 => 0xc7,
        End | Numpad1 => 0xcf,
        PageUp | Numpad9 => 0xc9,
        PageDown | Numpad3 => 0xd1,
        Insert | Numpad0 => 0xd2,
        Delete | NumpadDecimal => 0xd3,
        Numpad2 => 0xaf,
        Numpad4 => 0xac,
        Numpad6 => 0xae,
        Numpad8 => 0xad,
        Numpad5 => b'5',
        F1 => 0xbb,
        F2 => 0xbc,
        F3 => 0xbd,
        F4 => 0xbe,
        F5 => 0xbf,
        F6 => 0xc0,
        F7 => 0xc1,
        F8 => 0xc2,
        F9 => 0xc3,
        F10 => 0xc4,
        F11 => 0xd7,
        F12 => 0xd8,
        _ => return None,
    })
}
impl Desktop {
    pub fn controller_look(&self, game: &mut Game, x: i32, y: i32) -> Result<()> {
        let (mut x, mut y) = (x.clamp(-1000000, 1000000), y.clamp(-1000000, 1000000));
        while x != 0 || y != 0 {
            let (dx, dy) = (x.clamp(-32768, 32767) as i16, y.clamp(-32768, 32767) as i16);
            game.doom_input(Event::Mouse {
                buttons: self.buttons,
                dx,
                dy,
            })?;
            x -= i32::from(dx);
            y -= i32::from(dy);
        }
        Ok(())
    }
    pub fn forward(
        &mut self,
        game: &mut Game,
        events: &[Native],
        captured: bool,
        recaptured: bool,
    ) -> Result<()> {
        for event in self.translate(events, captured, recaptured)? {
            if event == Event::Release {
                game.doom_release_input()?;
            } else {
                game.doom_input(event)?;
            }
        }
        Ok(())
    }
    fn translate(
        &mut self,
        events: &[Native],
        captured: bool,
        recaptured: bool,
    ) -> Result<Vec<Event>> {
        let mut output = Vec::new();
        if !captured {
            if self.active {
                output.push(Event::Release);
            }
            *self = Self::default();
            return Ok(output);
        }
        self.active = true;
        for event in events {
            match *event {
                Native::Overflow => bail!("desktop input exceeded its bounded queue"),
                Native::Focus(false) => {
                    output.push(Event::Release);
                    *self = Self::default();
                }
                Native::Key {
                    code: key,
                    pressed,
                    repeat,
                } => {
                    let Some(code) = code(key) else { continue };
                    let was = self
                        .keys
                        .iter()
                        .any(|key| crate::gui::doom_input::code(*key) == Some(code));
                    let changed = if pressed {
                        self.keys.insert(key)
                    } else {
                        self.keys.remove(&key)
                    };
                    let now = self
                        .keys
                        .iter()
                        .any(|key| crate::gui::doom_input::code(*key) == Some(code));
                    if was != now
                        || (pressed && repeat && !matches!(code, 0xb6 | 0x9d | 0xb8))
                        || (pressed && changed && was && !matches!(code, 0xb6 | 0x9d | 0xb8))
                    {
                        output.push(Event::Key { code, down: now });
                    }
                }
                Native::Button { button, pressed } if !recaptured => {
                    let bit = match button {
                        MouseButton::Left => 0,
                        MouseButton::Right => 1,
                        MouseButton::Middle => 2,
                        MouseButton::Back => 5,
                        MouseButton::Forward => 6,
                        MouseButton::Other(7) => 7,
                        _ => continue,
                    };
                    if pressed {
                        self.buttons |= 1 << bit;
                    } else {
                        self.buttons &= !(1 << bit);
                    }
                    output.push(Event::Mouse {
                        buttons: self.buttons,
                        dx: 0,
                        dy: 0,
                    });
                }
                Native::Motion(x, y) if !recaptured => {
                    if !x.is_finite() || !y.is_finite() {
                        continue;
                    }
                    let (x, y) = (self.motion.0 + x, self.motion.1 + y);
                    if x.abs() > 1000000.0 || y.abs() > 1000000.0 {
                        bail!("mouse motion exceeded its bound");
                    }
                    self.motion = (x.fract(), y.fract());
                    let (mut x, mut y) = (x.trunc() as i32, y.trunc() as i32);
                    while x != 0 || y != 0 {
                        let (dx, dy) =
                            (x.clamp(-32768, 32767) as i16, y.clamp(-32768, 32767) as i16);
                        output.push(Event::Mouse {
                            buttons: self.buttons,
                            dx,
                            dy,
                        });
                        x -= i32::from(dx);
                        y -= i32::from(dy);
                    }
                }
                Native::Wheel(delta) if !recaptured => {
                    let y = match delta {
                        MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                        MouseScrollDelta::PixelDelta(pos) => pos.y / 40.0,
                    };
                    if !y.is_finite() {
                        continue;
                    }
                    self.wheel += y;
                    if self.wheel.abs() > 32.0 {
                        bail!("mouse wheel exceeded its bound");
                    }
                    while self.wheel.abs() >= 1.0 {
                        let bit = if self.wheel > 0.0 { 3 } else { 4 };
                        output.push(Event::Mouse {
                            buttons: self.buttons | (1 << bit),
                            dx: 0,
                            dy: 0,
                        });
                        output.push(Event::Mouse {
                            buttons: self.buttons,
                            dx: 0,
                            dy: 0,
                        });
                        self.wheel -= self.wheel.signum();
                    }
                }
                _ => {}
            }
        }
        Ok(output)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode, pressed: bool) -> Native {
        Native::Key {
            code,
            pressed,
            repeat: false,
        }
    }
    #[test]
    fn physical_aliases_repeats_and_focus_release_preserve_key_state() {
        use KeyCode::*;
        let mut desktop = Desktop::default();
        assert!(desktop
            .translate(&[key(KeyQ, true)], false, false)
            .unwrap()
            .is_empty());
        let events = [
            key(ShiftLeft, true),
            key(ShiftRight, true),
            key(KeyA, true),
            Native::Key {
                code: KeyA,
                pressed: true,
                repeat: true,
            },
            key(ShiftLeft, false),
            key(ShiftRight, false),
            key(KeyA, false),
        ];
        assert_eq!(
            desktop.translate(&events, true, false).unwrap(),
            vec![
                Event::Key {
                    code: 0xb6,
                    down: true
                },
                Event::Key {
                    code: b'a',
                    down: true
                },
                Event::Key {
                    code: b'a',
                    down: true
                },
                Event::Key {
                    code: 0xb6,
                    down: false
                },
                Event::Key {
                    code: b'a',
                    down: false
                }
            ]
        );
        assert_eq!(
            desktop
                .translate(
                    &[key(Pause, true), key(Numpad2, true), key(F12, true)],
                    true,
                    false
                )
                .unwrap(),
            vec![
                Event::Key {
                    code: 255,
                    down: true
                },
                Event::Key {
                    code: 0xaf,
                    down: true
                },
                Event::Key {
                    code: 0xd8,
                    down: true
                }
            ]
        );
        assert_eq!(
            desktop.translate(&[], false, false).unwrap(),
            vec![Event::Release]
        );
        assert!(desktop.keys.is_empty());
        assert!(desktop.translate(&[], false, false).unwrap().is_empty());
    }
    #[test]
    fn pointer_buttons_fractional_motion_wheel_and_recapture_are_ordered() {
        let mut desktop = Desktop::default();
        let events = [
            Native::Button {
                button: MouseButton::Right,
                pressed: true,
            },
            Native::Motion(0.6, 0.0),
            Native::Motion(0.6, 0.0),
            Native::Wheel(MouseScrollDelta::LineDelta(0.0, 1.0)),
            Native::Button {
                button: MouseButton::Right,
                pressed: false,
            },
        ];
        assert_eq!(
            desktop.translate(&events, true, false).unwrap(),
            vec![
                Event::Mouse {
                    buttons: 2,
                    dx: 0,
                    dy: 0
                },
                Event::Mouse {
                    buttons: 2,
                    dx: 1,
                    dy: 0
                },
                Event::Mouse {
                    buttons: 10,
                    dx: 0,
                    dy: 0
                },
                Event::Mouse {
                    buttons: 2,
                    dx: 0,
                    dy: 0
                },
                Event::Mouse {
                    buttons: 0,
                    dx: 0,
                    dy: 0
                }
            ]
        );
        assert!(Desktop::default()
            .translate(&events, true, true)
            .unwrap()
            .is_empty());
        assert!(desktop.translate(&[Native::Overflow], true, false).is_err());
    }
}
