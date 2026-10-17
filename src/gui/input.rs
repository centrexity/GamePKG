//! Device access lives in the host; guests receive only a bounded button mask.
use crate::input_buttons as buttons;
use eframe::egui;

#[derive(Default)]
pub struct Devices {
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    controller: Option<gilrs::Gilrs>,
    initialized: bool,
    previous: u32,
    look: (i32, i32),
    contacts: std::collections::BTreeMap<(egui::TouchDeviceId, egui::TouchId), egui::Pos2>,
    error: Option<String>,
}
impl Devices {
    pub fn reset(&mut self) {
        self.previous = 0;
        self.look = (0, 0);
        self.contacts.clear();
    }
    fn update_contacts(&mut self, events: &[egui::Event], focused: bool) {
        for event in events {
            if let egui::Event::Touch {
                device_id,
                id,
                phase,
                pos,
                ..
            } = event
            {
                let key = (*device_id, *id);
                match phase {
                    egui::TouchPhase::Start | egui::TouchPhase::Move => {
                        if self.contacts.len() < 32 || self.contacts.contains_key(&key) {
                            self.contacts.insert(key, *pos);
                        }
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                        self.contacts.remove(&key);
                    }
                }
            }
        }
        if !focused {
            self.contacts.clear();
        }
    }
    pub fn look_delta(&self) -> (i32, i32) {
        self.look
    }
    pub fn poll(&mut self, ui: &mut egui::Ui) -> u32 {
        self.look = (0, 0);
        if !self.initialized {
            self.initialized = true;
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            match gilrs::Gilrs::new() {
                Ok(controller) => self.controller = Some(controller),
                Err(error) => self.error = Some(format!("Controller input unavailable: {error}")),
            }
        }
        let mut held = 0;
        let mut pressed = 0;
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        if let Some(controller) = self.controller.as_mut() {
            // Bound per-frame work even when devices flood the queue.
            for _ in 0..256 {
                match controller.next_event() {
                    Some(event) => {
                        if let gilrs::EventType::ButtonPressed(button, _) = event.event {
                            pressed |= controller_button(button);
                        }
                    }
                    None => break,
                }
            }
            if ui.input(|i| i.focused) {
                for (_, gamepad) in controller.gamepads() {
                    self.look.0 = (self.look.0
                        + look_axis(gamepad.value(gilrs::Axis::RightStickX)))
                    .clamp(-64, 64);
                    self.look.1 = (self.look.1
                        - look_axis(gamepad.value(gilrs::Axis::RightStickY)))
                    .clamp(-64, 64);
                    for (button, mask) in [
                        (gilrs::Button::DPadLeft, buttons::LEFT),
                        (gilrs::Button::DPadRight, buttons::RIGHT),
                        (gilrs::Button::DPadUp, buttons::UP),
                        (gilrs::Button::DPadDown, buttons::DOWN),
                        (gilrs::Button::South, buttons::B),
                        (gilrs::Button::West, buttons::Y),
                        (gilrs::Button::East, buttons::A),
                        (gilrs::Button::North, buttons::X),
                        (gilrs::Button::Start, buttons::START),
                        (gilrs::Button::Select, buttons::SELECT),
                        (gilrs::Button::LeftTrigger, buttons::L),
                        (gilrs::Button::RightTrigger, buttons::R),
                    ] {
                        if gamepad.is_pressed(button) {
                            held |= mask;
                        }
                    }
                    held |= stick(
                        gamepad.value(gilrs::Axis::LeftStickX),
                        gamepad.value(gilrs::Axis::LeftStickY),
                    );
                }
            }
        }
        ui.input(|i| self.update_contacts(&i.events, i.focused));
        // Touch targets never draw host controls over the game. The bottom third
        // retains simultaneous direction/action input in twelve equal regions.
        let bounds = ui.max_rect();
        for (index, mask) in [
            buttons::LEFT,
            buttons::RIGHT,
            buttons::UP,
            buttons::DOWN,
            buttons::B,
            buttons::Y,
            buttons::A,
            buttons::X,
            buttons::L,
            buttons::R,
            buttons::START,
            buttons::SELECT,
        ]
        .into_iter()
        .enumerate()
        {
            let rect = egui::Rect::from_min_max(
                egui::pos2(
                    bounds.left() + bounds.width() * index as f32 / 12.0,
                    bounds.bottom() - bounds.height() / 3.0,
                ),
                egui::pos2(
                    bounds.left() + bounds.width() * (index + 1) as f32 / 12.0,
                    bounds.bottom(),
                ),
            );
            if self.contacts.values().any(|pos| rect.contains(*pos)) {
                held |= mask;
            }
            if ui.input(|i| i.events.iter().any(|e| matches!(e,
                egui::Event::Touch { phase: egui::TouchPhase::Start, pos, .. } if rect.contains(*pos)))) {
                pressed |= mask;
            }
        }
        if !ui.input(|i| i.focused) {
            held = 0;
            pressed = 0;
        }
        let encoded = buttons::snapshot(held, self.previous) | pressed;
        self.previous = held;
        encoded
    }
}
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn controller_button(button: gilrs::Button) -> u32 {
    match button {
        gilrs::Button::DPadLeft => buttons::LEFT,
        gilrs::Button::DPadRight => buttons::RIGHT,
        gilrs::Button::DPadUp => buttons::UP,
        gilrs::Button::DPadDown => buttons::DOWN,
        gilrs::Button::South => buttons::B,
        gilrs::Button::West => buttons::Y,
        gilrs::Button::East => buttons::A,
        gilrs::Button::North => buttons::X,
        gilrs::Button::Start => buttons::START,
        gilrs::Button::Select => buttons::SELECT,
        gilrs::Button::LeftTrigger => buttons::L,
        gilrs::Button::RightTrigger => buttons::R,
        _ => 0,
    }
}
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos", test))]
fn look_axis(value: f32) -> i32 {
    if !value.is_finite() || value.abs() <= 0.2 {
        0
    } else {
        (value.signum() * ((value.abs().min(1.0) - 0.2) / 0.8) * 12.0).round() as i32
    }
}
fn stick(x: f32, y: f32) -> u32 {
    (u32::from(x < -0.35) * buttons::LEFT)
        | (u32::from(x > 0.35) * buttons::RIGHT)
        | (u32::from(y > 0.35) * buttons::UP)
        | (u32::from(y < -0.35) * buttons::DOWN)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn touch(id: u64, phase: egui::TouchPhase, x: f32) -> egui::Event {
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(id),
            phase,
            pos: egui::pos2(x, 10.0),
            force: None,
        }
    }
    #[test]
    fn independent_contacts_persist_move_cancel_and_clear_on_focus_loss() {
        let mut devices = Devices::default();
        devices.update_contacts(
            &[
                touch(1, egui::TouchPhase::Start, 10.0),
                touch(2, egui::TouchPhase::Start, 60.0),
            ],
            true,
        );
        assert_eq!(devices.contacts.len(), 2);
        devices.update_contacts(&[], true);
        assert_eq!(devices.contacts.len(), 2);
        devices.update_contacts(
            &[
                touch(1, egui::TouchPhase::Move, 20.0),
                touch(2, egui::TouchPhase::Cancel, 60.0),
            ],
            true,
        );
        assert_eq!(devices.contacts.len(), 1);
        assert_eq!(devices.contacts.values().next().unwrap().x, 20.0);
        devices.update_contacts(&[touch(1, egui::TouchPhase::End, 20.0)], true);
        assert!(devices.contacts.is_empty());
        devices.update_contacts(&[touch(3, egui::TouchPhase::Start, 10.0)], false);
        assert!(devices.contacts.is_empty());
        let flood: Vec<_> = (0..100)
            .map(|id| touch(id, egui::TouchPhase::Start, 10.0))
            .collect();
        devices.update_contacts(&flood, true);
        assert_eq!(devices.contacts.len(), 32);
        devices.reset();
        assert!(devices.contacts.is_empty());
    }
    #[test]
    fn look_stick_is_bounded_and_invalid_samples_cannot_move_the_camera() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.2, 0.0, 0.2] {
            assert_eq!(look_axis(value), 0);
        }
        assert_eq!(look_axis(1.0), 12);
        assert_eq!(look_axis(-1.0), -12);
        assert_eq!(look_axis(1000.0), 12);
        assert_eq!(look_axis(-1000.0), -12);
    }
    #[test]
    fn analog_deadzone_and_invalid_values_do_not_press_buttons() {
        assert_eq!(stick(0.1, -0.2), 0);
        assert_eq!(stick(f32::NAN, f32::NAN), 0);
        assert_eq!(stick(-1.0, 1.0), buttons::LEFT | buttons::UP);
        assert_eq!(stick(1.0, -1.0), buttons::RIGHT | buttons::DOWN);
    }
}
