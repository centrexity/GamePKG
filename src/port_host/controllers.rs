//! Host controller provider. Full axes, no imposed game deadzones or filters.
use super::{Error, Result};
use gamepkg_native_input::{Gamepad, Service};
use gilrs::{Axis, Button, GamepadId, Gilrs, GilrsBuilder};
use std::collections::HashMap;
const BACKEND: u32 = 0x47504749;
pub struct Provider {
    gilrs: Gilrs,
    devices: HashMap<GamepadId, u64>,
    effects: HashMap<GamepadId, gilrs::ff::Effect>,
}
fn axis(value: f32) -> i16 {
    if !value.is_finite() {
        return 0;
    }
    let v = value.clamp(-1.0, 1.0);
    (v * if v < 0.0 { 32768.0 } else { 32767.0 }).round() as i16
}
fn trigger(value: f32) -> u16 {
    if value.is_finite() {
        (value.clamp(0.0, 1.0) * 65535.0).round() as u16
    } else {
        0
    }
}
impl Provider {
    pub fn new() -> Result<Self> {
        let gilrs = GilrsBuilder::new()
            .with_default_filters(false)
            .set_update_state(true)
            .build()
            .map_err(|_| Error::Unsupported)?;
        Ok(Self {
            gilrs,
            devices: HashMap::new(),
            effects: HashMap::new(),
        })
    }
    pub fn poll(&mut self, input: &mut Service) {
        // A busy hardware provider cannot monopolize the window owner.
        for _ in 0..2048 {
            if self.gilrs.next_event().is_none() {
                break;
            }
        }
        let connected = self.gilrs.gamepads().map(|(id, _)| id).collect::<Vec<_>>();
        self.devices.retain(|id, device| {
            if connected.contains(id) {
                true
            } else {
                input.disconnect(*device);
                self.effects.remove(id);
                false
            }
        });
        for id in connected {
            let device = if let Some(device) = self.devices.get(&id) {
                *device
            } else {
                let token = usize::from(id) as u64 + 1;
                let Some(device) = input.connect(3, BACKEND, token) else {
                    continue;
                };
                self.devices.insert(id, device);
                device
            };
            let pad = self.gilrs.gamepad(id);
            let buttons = [
                Button::DPadUp,
                Button::DPadDown,
                Button::DPadLeft,
                Button::DPadRight,
                Button::Start,
                Button::Select,
                Button::LeftThumb,
                Button::RightThumb,
                Button::LeftTrigger,
                Button::RightTrigger,
                Button::Mode,
                Button::Unknown,
                Button::South,
                Button::East,
                Button::West,
                Button::North,
            ];
            let mut mask = 0u32;
            for (bit, button) in buttons.into_iter().enumerate() {
                if button != Button::Unknown && pad.is_pressed(button) {
                    mask |= 1 << bit;
                }
            }
            // Some providers expose the hat only as axes when filters are off.
            let x = pad.value(Axis::DPadX);
            let y = pad.value(Axis::DPadY);
            if x < -0.5 {
                mask |= 1 << 2;
            }
            if x > 0.5 {
                mask |= 1 << 3;
            }
            if y > 0.5 {
                mask |= 1;
            }
            if y < -0.5 {
                mask |= 1 << 1;
            }
            input.update_gamepad(Gamepad {
                device,
                timestamp_ns: gamepkg_native_input::gamepkg_input_time_ns(),
                buttons: mask,
                left_x: axis(pad.value(Axis::LeftStickX)),
                left_y: axis(pad.value(Axis::LeftStickY)),
                right_x: axis(pad.value(Axis::RightStickX)),
                right_y: axis(pad.value(Axis::RightStickY)),
                left_trigger: trigger(
                    pad.button_data(Button::LeftTrigger2)
                        .map_or(0.0, |b| b.value()),
                ),
                right_trigger: trigger(
                    pad.button_data(Button::RightTrigger2)
                        .map_or(0.0, |b| b.value()),
                ),
            });
        }
    }
    pub fn rumble(&mut self, device: u64, low: u16, high: u16, millis: u32) -> Result<()> {
        use gilrs::ff::{BaseEffect, BaseEffectType, EffectBuilder, Repeat, Replay, Ticks};
        if millis > 10000 {
            return Err(Error::Invalid);
        }
        let id = self
            .devices
            .iter()
            .find_map(|(id, d)| (*d == device).then_some(*id))
            .ok_or(Error::Denied)?;
        if !self.gilrs.gamepad(id).is_ff_supported() {
            return Err(Error::Unsupported);
        }
        if let Some(effect) = self.effects.remove(&id) {
            let _ = effect.stop();
        }
        if millis == 0 || (low == 0 && high == 0) {
            return Ok(());
        }
        let duration = Ticks::from_ms(millis);
        let effect = EffectBuilder::new()
            .add_effect(BaseEffect {
                kind: BaseEffectType::Strong { magnitude: low },
                scheduling: Replay {
                    play_for: duration,
                    ..Default::default()
                },
                ..Default::default()
            })
            .add_effect(BaseEffect {
                kind: BaseEffectType::Weak { magnitude: high },
                scheduling: Replay {
                    play_for: duration,
                    ..Default::default()
                },
                ..Default::default()
            })
            .gamepads(&[id])
            .repeat(Repeat::For(duration))
            .finish(&mut self.gilrs)
            .map_err(|_| Error::Failed)?;
        effect.play().map_err(|_| Error::Failed)?;
        self.effects.insert(id, effect);
        Ok(())
    }
    pub fn disconnect(&mut self, input: &mut Service) {
        self.effects.clear();
        for (_, id) in self.devices.drain() {
            input.disconnect(id);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analog_quantization_preserves_full_ranges_without_deadzone() {
        assert_eq!(axis(-1.0), i16::MIN);
        assert_eq!(axis(1.0), i16::MAX);
        assert_eq!(axis(0.001), 33);
        assert_eq!(axis(f32::NAN), 0);
        assert_eq!(trigger(1.0), u16::MAX);
        assert_eq!(trigger(0.5), 32768);
        assert_eq!(trigger(f32::INFINITY), 0);
    }
}
