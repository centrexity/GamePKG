//! Device access lives in the host; guests receive only a bounded button mask.
use crate::input_buttons as buttons;
use eframe::egui;

pub fn key_edge(input: &egui::InputState, key: egui::Key) -> bool {
    input.events.iter().any(|event| matches!(event,egui::Event::Key {key:actual,pressed:true,repeat:false,..} if *actual==key))
}
#[derive(Default)]
pub struct Keyboard {
    keys: std::collections::HashMap<egui::Key, u32>,
}
fn key_button(key: egui::Key) -> u32 {
    use egui::Key::*;
    match key {
        ArrowLeft | A => buttons::LEFT,
        ArrowRight | D => buttons::RIGHT,
        ArrowUp | W => buttons::UP,
        ArrowDown | S => buttons::DOWN,
        Z => buttons::B,
        X => buttons::Y,
        Enter => buttons::START,
        Tab => buttons::SELECT,
        C => buttons::A,
        V => buttons::X,
        Q => buttons::L,
        E => buttons::R,
        _ => 0,
    }
}
impl Keyboard {
    pub fn reset(&mut self) {
        self.keys.clear();
    }
    pub fn poll(&mut self, input: &egui::InputState) -> u32 {
        if !input.focused {
            self.reset();
            return 0;
        }
        let mut pressed = 0;
        for event in &input.events {
            if let egui::Event::Key {
                key,
                physical_key,
                pressed: down,
                repeat: false,
                modifiers,
                ..
            } = event
            {
                let id = physical_key.unwrap_or(*key);
                if *down {
                    if (modifiers.ctrl || modifiers.command || modifiers.alt)
                        && !matches!(
                            key,
                            egui::Key::ArrowLeft
                                | egui::Key::ArrowRight
                                | egui::Key::ArrowUp
                                | egui::Key::ArrowDown
                                | egui::Key::Enter
                                | egui::Key::Tab
                        )
                    {
                        continue;
                    }
                    let bit = key_button(*key);
                    if bit == 0 || self.keys.contains_key(&id) {
                        continue;
                    }
                    let previously_down = self.keys.values().any(|held| *held == bit);
                    self.keys.insert(id, bit);
                    if !previously_down {
                        pressed |= bit;
                    }
                } else {
                    self.keys.remove(&id);
                }
            }
        }
        let held = self.keys.values().fold(0, |mask, bit| mask | *bit);
        pressed | buttons::held_bits(held)
    }
}

#[derive(Default)]
pub struct Devices {
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    controller: Option<gilrs::Gilrs>,
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    controller_events: ControllerEvents,
    initialized: bool,
    previous: u32,
    look: (i32, i32),
    look_motion: LookMotion,
    touch_look: TouchLook,
    contacts: std::collections::BTreeMap<(egui::TouchDeviceId, egui::TouchId), egui::Pos2>,
    error: Option<String>,
}
impl Devices {
    pub fn reset(&mut self) {
        self.previous = 0;
        self.look = (0, 0);
        self.look_motion = Default::default();
        self.touch_look = Default::default();
        self.contacts.clear();
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        {
            self.controller_events = ControllerEvents::default();
        }
    }
    fn update_contacts(&mut self, events: &[egui::Event], focused: bool) {
        if !focused {
            self.contacts.clear();
            return;
        }
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
                    egui::TouchPhase::Start if pos.is_finite() => {
                        if self.contacts.len() < 32 || self.contacts.contains_key(&key) {
                            self.contacts.insert(key, *pos);
                        }
                    }
                    egui::TouchPhase::Move if pos.is_finite() => {
                        if let Some(previous) = self.contacts.get_mut(&key) {
                            *previous = *pos;
                        }
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                        self.contacts.remove(&key);
                    }
                    _ => {
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
    pub fn poll(&mut self, ui: &mut egui::Ui, camera: bool) -> u32 {
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
        let focused = ui.input(|i| i.focused);
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        if let Some(controller) = self.controller.as_mut() {
            let (edges, baseline) = self.controller_events.poll(focused, || {
                controller.next_event().map(|event| match event.event {
                    gilrs::EventType::ButtonPressed(button, _) => Some(button),
                    _ => None,
                })
            });
            pressed |= edges;
            if focused {
                let mut controller_held_mask = 0;
                for (_, gamepad) in controller.gamepads() {
                    self.look.0 = (self.look.0
                        + look_axis(gamepad.value(gilrs::Axis::RightStickX)))
                    .clamp(-64, 64);
                    self.look.1 = (self.look.1
                        - look_axis(gamepad.value(gilrs::Axis::RightStickY)))
                    .clamp(-64, 64);
                    controller_held_mask |= controller_held(|button| gamepad.is_pressed(button));
                    controller_held_mask |= stick(
                        gamepad.value(gilrs::Axis::LeftStickX),
                        gamepad.value(gilrs::Axis::LeftStickY),
                    );
                }
                held |= controller_held_mask;
                if baseline {
                    // Recovered held state is movement/state, not a fresh gesture.
                    self.previous |= controller_held_mask;
                }
            }
        }
        self.look = self
            .look_motion
            .sample(self.look, ui.input(|i| i.time), focused);
        ui.input(|i| self.update_contacts(&i.events, focused));
        // Touch targets never draw host controls over the game. The bottom third
        // retains simultaneous direction/action input in twelve equal regions.
        let bounds = ui.max_rect();
        let touch_scale = ui.ctx().pixels_per_point();
        let touch_delta = ui.input(|i| {
            self.touch_look
                .update(&i.events, focused && camera, bounds, touch_scale)
        });
        self.look.0 = self.look.0.saturating_add(touch_delta.0);
        self.look.1 = self.look.1.saturating_add(touch_delta.1);
        for (key, pos) in &self.contacts {
            if self.touch_look.active.as_ref().map(|(id, _)| id) != Some(key) {
                held |= touch_button(bounds, *pos);
            }
        }
        ui.input(|i| {
            for event in &i.events {
                if let egui::Event::Touch {
                    phase: egui::TouchPhase::Start,
                    pos,
                    ..
                } = event
                {
                    pressed |= touch_button(bounds, *pos);
                }
            }
        });
        if !focused {
            held = 0;
            pressed = 0;
        }
        let encoded = buttons::snapshot(held, self.previous) | pressed;
        self.previous = held;
        encoded
    }
}
#[derive(Default)]
struct TouchLook {
    active: Option<((egui::TouchDeviceId, egui::TouchId), egui::Pos2)>,
    remainder: [f64; 2],
}
impl TouchLook {
    fn update(
        &mut self,
        events: &[egui::Event],
        enabled: bool,
        bounds: egui::Rect,
        scale: f32,
    ) -> (i32, i32) {
        if !enabled
            || !bounds.is_finite()
            || !bounds.width().is_finite()
            || !bounds.height().is_finite()
            || bounds.width() <= 0.0
            || bounds.height() <= 0.0
            || !scale.is_finite()
            || scale <= 0.0
        {
            *self = Self::default();
            return (0, 0);
        }
        let mut travel = self.remainder;
        let mut output = [0i32; 2];
        for event in events {
            let egui::Event::Touch {
                device_id,
                id,
                phase,
                pos,
                ..
            } = event
            else {
                continue;
            };
            let key = (*device_id, *id);
            if *phase == egui::TouchPhase::Cancel
                && self.active.as_ref().map(|(id, _)| *id) == Some(key)
            {
                *self = Self::default();
                travel = [0.0; 2];
                continue;
            }
            if !pos.is_finite() {
                if self.active.as_ref().map(|(id, _)| *id) == Some(key) {
                    *self = Self::default();
                    travel = [0.0; 2];
                }
                continue;
            }
            if *phase == egui::TouchPhase::Start {
                if self.active.is_none()
                    && bounds.contains(*pos)
                    && pos.y < bounds.bottom() - bounds.height() / 3.0
                {
                    self.active = Some((key, *pos));
                    travel = [0.0; 2];
                }
                continue;
            }
            if let Some((active, previous)) = self.active.as_mut() {
                if *active != key {
                    continue;
                }
                for (axis, (now, old)) in [(pos.x, previous.x), (pos.y, previous.y)]
                    .into_iter()
                    .enumerate()
                {
                    travel[axis] = (travel[axis]
                        + (f64::from(now) - f64::from(old)) * f64::from(scale))
                    .clamp(-32768.0, 32767.0);
                    let whole = travel[axis].trunc() as i32;
                    output[axis] = output[axis].saturating_add(whole).clamp(-32768, 32767);
                    travel[axis] -= f64::from(whole);
                }
                *previous = *pos;
                if *phase == egui::TouchPhase::End {
                    self.active = None;
                }
            }
        }
        self.remainder = if self.active.is_some() {
            travel
        } else {
            [0.0; 2]
        };
        (output[0], output[1])
    }
}
/// Stick values are rates in the previous 60-Hz units, not mouse events.
/// Keep subpixel travel, but never catch up a long pause or lost focus.
#[derive(Default)]
struct LookMotion {
    time: Option<f64>,
    remainder: [f64; 2],
}
impl LookMotion {
    fn sample(&mut self, rate: (i32, i32), time: f64, focused: bool) -> (i32, i32) {
        if !focused || !time.is_finite() {
            *self = Self::default();
            return (0, 0);
        }
        let previous = self.time.replace(time);
        let Some(previous) = previous else {
            return (0, 0);
        };
        let elapsed = time - previous;
        if !(0.0..=0.1 + 1e-9).contains(&elapsed) {
            self.remainder = [0.0; 2];
            return (0, 0);
        }
        let mut output = [0; 2];
        for (axis, value) in [rate.0, rate.1].into_iter().enumerate() {
            let value = value.clamp(-64, 64);
            if value == 0 || self.remainder[axis].signum() != f64::from(value).signum() {
                self.remainder[axis] = 0.0;
            }
            let travel = f64::from(value) * 60.0 * elapsed.min(0.1) + self.remainder[axis];
            output[axis] = travel.trunc() as i32;
            self.remainder[axis] = travel - f64::from(output[axis]);
        }
        (output[0], output[1])
    }
}
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
#[derive(Default)]
struct ControllerEvents {
    ready: bool,
}
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
impl ControllerEvents {
    fn poll(
        &mut self,
        focused: bool,
        mut next: impl FnMut() -> Option<Option<gilrs::Button>>,
    ) -> (u32, bool) {
        if !focused {
            self.ready = false;
        }
        let was_ready = self.ready;
        let mut pressed = 0;
        // A bounded backlog may span frames. Rearm only after observing its end.
        for _ in 0..256 {
            match next() {
                Some(Some(button)) if was_ready => pressed |= controller_button(button),
                Some(_) => (),
                None => {
                    self.ready = focused;
                    break;
                }
            }
        }
        (pressed, !was_ready)
    }
}
fn touch_button(bounds: egui::Rect, pos: egui::Pos2) -> u32 {
    if !bounds.min.is_finite()
        || !bounds.max.is_finite()
        || !pos.is_finite()
        || bounds.width() <= 0.0
        || bounds.height() <= 0.0
        || !bounds.contains(pos)
        || pos.y < bounds.bottom() - bounds.height() / 3.0
    {
        return 0;
    }
    let buttons = [
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
    ];
    for (index, button) in buttons.into_iter().enumerate() {
        let right = bounds.left() + bounds.width() * (index + 1) as f32 / 12.0;
        // Shared edges belong to the region on their right. Keep the outer
        // right edge usable without ever pressing two neighboring actions.
        if pos.x < right || index == 11 {
            return button;
        }
    }
    0
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
        gilrs::Button::LeftTrigger | gilrs::Button::LeftTrigger2 => buttons::L,
        gilrs::Button::RightTrigger | gilrs::Button::RightTrigger2 => buttons::R,
        _ => 0,
    }
}
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn controller_held(mut pressed: impl FnMut(gilrs::Button) -> bool) -> u32 {
    use gilrs::Button::*;
    [
        DPadLeft,
        DPadRight,
        DPadUp,
        DPadDown,
        South,
        West,
        East,
        North,
        Start,
        Select,
        LeftTrigger,
        RightTrigger,
        LeftTrigger2,
        RightTrigger2,
    ]
    .into_iter()
    .filter(|button| pressed(*button))
    .fold(0, |mask, button| mask | controller_button(button))
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
    let x = if x.is_finite() { x } else { 0.0 };
    let y = if y.is_finite() { y } else { 0.0 };
    (u32::from(x < -0.35) * buttons::LEFT)
        | (u32::from(x > 0.35) * buttons::RIGHT)
        | (u32::from(y > 0.35) * buttons::UP)
        | (u32::from(y < -0.35) * buttons::DOWN)
}
#[cfg(test)]
mod tests {
    #[test]
    fn controller_turning_uses_time_instead_of_window_update_count() {
        for fps in [10, 20, 30, 60, 75, 120, 144, 240] {
            for rate in [1, 3, 12, 64] {
                let mut motion = super::LookMotion::default();
                assert_eq!(motion.sample((rate, -rate), 0.0, true), (0, 0));
                let mut total = (0, 0);
                for frame in 1..=fps * 10 {
                    let step =
                        motion.sample((rate, -rate), f64::from(frame) / f64::from(fps), true);
                    assert!(step.0.abs() <= 384 && step.1.abs() <= 384);
                    total.0 += step.0;
                    total.1 += step.1;
                }
                let expected = rate * 600;
                assert!(
                    (total.0 - expected).abs() <= 1,
                    "{fps} Hz, rate {rate}: {total:?}"
                );
                assert!((total.1 + expected).abs() <= 1);
            }
        }
        let mut motion = super::LookMotion::default();
        motion.sample((12, -12), 0.0, true);
        let mut total = (0, 0);
        let mut time = 0.0;
        for _ in 0..100 {
            for elapsed in [0.001, 0.007, 0.019, 0.003, 0.02] {
                time += elapsed;
                let step = motion.sample((12, -12), time, true);
                total.0 += step.0;
                total.1 += step.1;
            }
        }
        assert!((total.0 - 3600).abs() <= 1);
        assert!((total.1 + 3600).abs() <= 1);
    }
    #[test]
    fn controller_turning_drops_stalls_focus_gaps_and_stale_fractional_motion() {
        let mut motion = super::LookMotion::default();
        assert_eq!(motion.sample((12, -12), 0.0, true), (0, 0));
        assert_eq!(motion.sample((12, -12), 0.01, true), (7, -7));
        assert_eq!(motion.sample((0, 0), 0.02, true), (0, 0));
        assert_eq!(motion.sample((-1, 1), 0.021, true), (0, 0));
        assert_eq!(motion.sample((1, -1), 0.022, true), (0, 0));
        for bad_time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(motion.sample((64, 64), bad_time, true), (0, 0));
            assert_eq!(motion.sample((64, 64), 1.0, true), (0, 0));
        }
        assert_eq!(motion.sample((64, 64), 50.0, true), (0, 0));
        assert_eq!(motion.sample((12, -12), 50.01, false), (0, 0));
        assert_eq!(motion.sample((12, -12), 50.02, true), (0, 0));
        assert_eq!(motion.sample((12, -12), 50.03, true), (7, -7));
        assert_eq!(motion.sample((12, -12), 49.0, true), (0, 0));
        assert_eq!(motion.sample((i32::MAX, i32::MIN), 49.1, true), (384, -384));
        assert_eq!(motion.sample((64, 64), 49.1, true), (0, 0));
    }
    use super::*;
    fn camera_touch(device: u64, id: u64, phase: egui::TouchPhase, x: f32, y: f32) -> egui::Event {
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(device),
            id: egui::TouchId(id),
            phase,
            pos: egui::pos2(x, y),
            force: None,
        }
    }
    #[test]
    fn touch_camera_owns_one_finger_and_keeps_ordered_fractional_scaled_motion() {
        use egui::TouchPhase::*;
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 600.0));
        let mut camera = TouchLook::default();
        let mut update = |events: Vec<egui::Event>| camera.update(&events, true, bounds, 2.0);
        assert_eq!(
            update(vec![camera_touch(1, 1, Start, 100.0, 100.0)]),
            (0, 0)
        );
        assert_eq!(
            update(vec![
                camera_touch(1, 2, Start, 200.0, 100.0),
                camera_touch(1, 2, Move, 500.0, 200.0),
                camera_touch(2, 1, Move, 600.0, 200.0)
            ]),
            (0, 0)
        );
        assert_eq!(
            update(vec![camera_touch(1, 1, Move, 100.25, 99.75)]),
            (0, 0)
        );
        assert_eq!(update(vec![camera_touch(1, 1, Move, 100.5, 99.5)]), (1, -1));
        assert_eq!(
            update(vec![
                camera_touch(1, 1, End, 110.5, 120.5),
                camera_touch(1, 3, Start, 500.0, 100.0),
                camera_touch(1, 3, Move, 510.0, 110.0)
            ]),
            (40, 62)
        );
        assert_eq!(
            update(vec![
                camera_touch(1, 3, Cancel, f32::NAN, 0.0),
                camera_touch(1, 3, Move, 1100.0, 500.0)
            ]),
            (0, 0)
        );
        assert!(camera.active.is_none());
        assert_eq!(
            camera.update(
                &[camera_touch(1, 1, Start, 100.0, 400.0)],
                true,
                bounds,
                1.0
            ),
            (0, 0)
        );
        assert!(camera.active.is_none());
        camera.update(
            &[camera_touch(1, 1, Start, 100.0, 100.0)],
            true,
            bounds,
            1.0,
        );
        assert_eq!(
            camera.update(
                &[camera_touch(1, 1, Move, 110.0, 110.0)],
                false,
                bounds,
                1.0
            ),
            (0, 0)
        );
        assert_eq!(
            camera.update(&[camera_touch(1, 1, Move, 200.0, 200.0)], true, bounds, 1.0),
            (0, 0)
        );
    }
    #[test]
    fn touch_camera_invalid_input_and_reset_cannot_resume_an_old_drag() {
        use egui::TouchPhase::*;
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 600.0));
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut camera = TouchLook::default();
            camera.update(
                &[camera_touch(1, 1, Start, 100.0, 100.0)],
                true,
                bounds,
                1.0,
            );
            assert_eq!(
                camera.update(
                    &[camera_touch(1, 1, Move, 200.0, 200.0)],
                    true,
                    bounds,
                    scale
                ),
                (0, 0)
            );
            assert!(camera.active.is_none());
        }
        let mut devices = Devices::default();
        devices.touch_look.update(
            &[camera_touch(1, 1, Start, 100.0, 100.0)],
            true,
            bounds,
            1.0,
        );
        assert_eq!(
            devices.touch_look.update(
                &[camera_touch(1, 1, Move, f32::MAX, -f32::MAX)],
                true,
                bounds,
                1.0
            ),
            (32767, -32768)
        );
        devices.reset();
        assert_eq!(
            devices
                .touch_look
                .update(&[camera_touch(1, 1, Move, 200.0, 200.0)], true, bounds, 1.0),
            (0, 0)
        );
    }
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
    fn device_frame(
        ctx: &egui::Context,
        devices: &mut Devices,
        events: Vec<egui::Event>,
        focused: bool,
    ) -> u32 {
        device_frame_mode(ctx, devices, events, focused, false)
    }
    fn device_frame_mode(
        ctx: &egui::Context,
        devices: &mut Devices,
        events: Vec<egui::Event>,
        focused: bool,
        camera: bool,
    ) -> u32 {
        let mut result = 0;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 600.0),
                )),
                events,
                focused,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        result = devices.poll(ui, camera);
                    });
            },
        );
        result
    }
    #[test]
    fn touch_camera_and_action_fingers_stay_independent_through_window_polling() {
        use egui::TouchPhase::*;
        let ctx = egui::Context::default();
        let mut devices = Devices {
            initialized: true,
            ..Default::default()
        };
        let mask = device_frame_mode(
            &ctx,
            &mut devices,
            vec![
                camera_touch(1, 1, Start, 250.0, 550.0),
                camera_touch(1, 2, Start, 600.0, 100.0),
            ],
            true,
            true,
        );
        assert_ne!(mask & buttons::UP, 0);
        assert_eq!(devices.look_delta(), (0, 0));
        let mask = device_frame_mode(
            &ctx,
            &mut devices,
            vec![camera_touch(1, 2, Move, 650.0, 550.0)],
            true,
            true,
        );
        assert_eq!(devices.look_delta(), (50, 450));
        assert_eq!(mask, buttons::held_bits(buttons::UP));
        let mask = device_frame_mode(
            &ctx,
            &mut devices,
            vec![
                camera_touch(1, 1, End, 250.0, 550.0),
                camera_touch(1, 3, Start, 450.0, 550.0),
                camera_touch(1, 2, Move, 655.0, 545.0),
            ],
            true,
            true,
        );
        assert_eq!(devices.look_delta(), (5, -5));
        assert_eq!(mask, buttons::snapshot(buttons::B, buttons::UP));
        let mask = device_frame_mode(&ctx, &mut devices, vec![], false, true);
        assert_eq!(mask, 0);
        assert_eq!(devices.look_delta(), (0, 0));
        let mask = device_frame_mode(
            &ctx,
            &mut devices,
            vec![camera_touch(1, 2, Move, 700.0, 550.0)],
            true,
            true,
        );
        assert_eq!(devices.look_delta(), (0, 0));
        assert_eq!(mask, 0);
        device_frame_mode(
            &ctx,
            &mut devices,
            vec![camera_touch(1, 2, Cancel, 700.0, 550.0)],
            true,
            true,
        );
        device_frame_mode(
            &ctx,
            &mut devices,
            vec![
                camera_touch(1, 4, Start, 500.0, 100.0),
                camera_touch(1, 4, Move, 510.0, 110.0),
            ],
            true,
            false,
        );
        assert_eq!(devices.look_delta(), (0, 0));
    }
    fn contact(id: u64, phase: egui::TouchPhase, x: f32) -> egui::Event {
        let mut event = touch(id, phase, x);
        if let egui::Event::Touch { pos, .. } = &mut event {
            pos.y = 550.0;
        }
        event
    }
    #[test]
    fn touch_shared_edges_have_one_action_through_actual_polling() {
        let expected = [
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
        ];
        for (index, button) in expected.into_iter().enumerate() {
            let ctx = egui::Context::default();
            let mut devices = Devices {
                initialized: true,
                ..Default::default()
            };
            let actual = device_frame(
                &ctx,
                &mut devices,
                vec![contact(1, egui::TouchPhase::Start, index as f32 * 100.0)],
                true,
            );
            assert_eq!(
                actual,
                button | buttons::held_bits(button),
                "boundary {index}"
            );
        }
    }
    #[test]
    fn touch_outer_edges_and_invalid_regions_do_not_create_extra_actions() {
        let bounds = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(1200.0, 600.0));
        assert_eq!(
            touch_button(bounds, egui::pos2(1210.0, 620.0)),
            buttons::SELECT
        );
        assert_eq!(touch_button(bounds, egui::pos2(10.0, 420.0)), buttons::LEFT);
        for pos in [
            egui::pos2(9.0, 550.0),
            egui::pos2(1211.0, 550.0),
            egui::pos2(10.0, 419.0),
            egui::pos2(10.0, 621.0),
            egui::pos2(f32::NAN, 550.0),
            egui::pos2(10.0, f32::INFINITY),
        ] {
            assert_eq!(touch_button(bounds, pos), 0);
        }
        for invalid in [
            egui::Rect::NOTHING,
            egui::Rect::EVERYTHING,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(0.0, 10.0)),
        ] {
            assert_eq!(touch_button(invalid, egui::pos2(0.0, 5.0)), 0);
        }
    }
    #[test]
    fn simultaneous_touch_move_cancel_tap_and_focus_loss_preserve_button_edges() {
        let ctx = egui::Context::default();
        let mut devices = Devices {
            initialized: true,
            ..Default::default()
        };
        let both = buttons::RIGHT | buttons::B;
        assert_eq!(
            device_frame(
                &ctx,
                &mut devices,
                vec![
                    contact(1, egui::TouchPhase::Start, 100.0),
                    contact(2, egui::TouchPhase::Start, 450.0)
                ],
                true
            ),
            both | buttons::held_bits(both)
        );
        assert_eq!(
            device_frame(&ctx, &mut devices, vec![], true),
            buttons::held_bits(both)
        );
        let moved = buttons::UP | buttons::B;
        assert_eq!(
            device_frame(
                &ctx,
                &mut devices,
                vec![contact(1, egui::TouchPhase::Move, 200.0)],
                true
            ),
            buttons::UP | buttons::held_bits(moved)
        );
        assert_eq!(
            device_frame(
                &ctx,
                &mut devices,
                vec![contact(2, egui::TouchPhase::Cancel, 450.0)],
                true
            ),
            buttons::held_bits(buttons::UP)
        );
        assert_eq!(
            device_frame(
                &ctx,
                &mut devices,
                vec![
                    contact(1, egui::TouchPhase::End, 200.0),
                    contact(3, egui::TouchPhase::Start, 650.0),
                    contact(3, egui::TouchPhase::End, 650.0)
                ],
                true
            ),
            buttons::A
        );
        assert_eq!(
            device_frame(
                &ctx,
                &mut devices,
                vec![contact(4, egui::TouchPhase::Start, 450.0)],
                false
            ),
            0
        );
        assert_eq!(device_frame(&ctx, &mut devices, vec![], true), 0);
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
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(stick(invalid, invalid), 0);
            assert_eq!(stick(invalid, 1.0), buttons::UP);
            assert_eq!(stick(-1.0, invalid), buttons::LEFT);
        }
    }
    #[test]
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    fn controller_bumpers_and_triggers_share_actions_without_premature_release() {
        use gilrs::Button::*;
        let states: &[(&[gilrs::Button], u32)] = &[
            (&[RightTrigger2], buttons::R),
            (&[RightTrigger, RightTrigger2], buttons::R),
            (&[RightTrigger], buttons::R),
            (&[], 0),
            (&[LeftTrigger2], buttons::L),
            (&[LeftTrigger, LeftTrigger2], buttons::L),
            (
                &[LeftTrigger2, RightTrigger2, South, DPadUp],
                buttons::L | buttons::R | buttons::B | buttons::UP,
            ),
            (&[LeftThumb, RightThumb, Mode, Unknown], 0),
        ];
        for (physical, expected) in states {
            let actual = controller_held(|button| physical.contains(&button));
            assert_eq!(actual, *expected);
        }
        assert_eq!(controller_button(LeftTrigger2), buttons::L);
        assert_eq!(controller_button(RightTrigger2), buttons::R);
        assert_eq!(controller_button(Unknown), 0);
    }
    #[test]
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    fn controller_focus_recovery_discards_queued_presses_before_rearming() {
        use gilrs::Button::*;
        let mut events = ControllerEvents::default();
        let mut backlog =
            std::collections::VecDeque::from([Some(RightTrigger2), None, Some(South), None]);
        assert_eq!(events.poll(true, || backlog.pop_front()), (0, true));
        assert!(events.ready && backlog.is_empty());
        let mut tap = [Some(RightTrigger2), None].into_iter();
        assert_eq!(events.poll(true, || tap.next()), (buttons::R, false));
        let mut background = [Some(South), None].into_iter();
        assert_eq!(events.poll(false, || background.next()), (0, true));
        assert!(!events.ready);
        let mut returned = [Some(LeftTrigger2), None].into_iter();
        assert_eq!(events.poll(true, || returned.next()), (0, true));
        let mut fresh = [Some(LeftTrigger2), None].into_iter();
        assert_eq!(events.poll(true, || fresh.next()), (buttons::L, false));
    }
    #[test]
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    fn controller_focus_backlog_stays_bounded_and_cannot_leak_across_frames() {
        let mut events = ControllerEvents::default();
        let mut backlog = std::collections::VecDeque::from(vec![Some(gilrs::Button::South); 513]);
        for remaining in [257, 1] {
            assert_eq!(events.poll(true, || backlog.pop_front()), (0, true));
            assert!(!events.ready);
            assert_eq!(backlog.len(), remaining);
        }
        assert_eq!(events.poll(true, || backlog.pop_front()), (0, true));
        assert!(events.ready && backlog.is_empty());
        let mut press = [Some(gilrs::Button::South)].into_iter();
        assert_eq!(events.poll(true, || press.next()), (buttons::B, false));
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;
    fn event(key: egui::Key, pressed: bool, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat,
            modifiers: egui::Modifiers::default(),
        }
    }
    fn frame(
        ctx: &egui::Context,
        keyboard: &mut Keyboard,
        events: Vec<egui::Event>,
        focused: bool,
    ) -> u32 {
        let raw = egui::RawInput {
            events,
            focused,
            ..Default::default()
        };
        let mut result = 0;
        let _ = ctx.run(raw, |ctx| result = ctx.input(|input| keyboard.poll(input)));
        result
    }
    #[test]
    fn command_letters_do_not_trigger_gameplay_while_arrow_controls_remain_available() {
        let ctx = egui::Context::default();
        let mut keyboard = Keyboard::default();
        let mut paste = event(egui::Key::V, true, false);
        if let egui::Event::Key { modifiers, .. } = &mut paste {
            modifiers.ctrl = true;
        }
        assert_eq!(frame(&ctx, &mut keyboard, vec![paste], true), 0);
        let mut arrow = event(egui::Key::ArrowLeft, true, false);
        if let egui::Event::Key { modifiers, .. } = &mut arrow {
            modifiers.ctrl = true;
        }
        assert_eq!(frame(&ctx, &mut keyboard, vec![arrow], true), 1 | 256);
    }
    #[test]
    fn physical_edges_ignore_repeat_and_keep_shared_bindings_down_until_all_release() {
        let ctx = egui::Context::default();
        let mut keyboard = Keyboard::default();
        use egui::Key::*;
        assert_eq!(
            frame(
                &ctx,
                &mut keyboard,
                vec![event(ArrowLeft, true, false)],
                true
            ),
            1 | 256
        );
        assert_eq!(
            frame(
                &ctx,
                &mut keyboard,
                vec![event(ArrowLeft, true, true)],
                true
            ),
            256
        );
        assert_eq!(
            frame(&ctx, &mut keyboard, vec![event(A, true, false)], true),
            256
        );
        assert_eq!(
            frame(
                &ctx,
                &mut keyboard,
                vec![event(ArrowLeft, false, false)],
                true
            ),
            256
        );
        assert_eq!(
            frame(&ctx, &mut keyboard, vec![event(A, false, false)], true),
            0
        );
        assert_eq!(
            frame(
                &ctx,
                &mut keyboard,
                vec![event(X, true, false), event(X, false, false)],
                true
            ),
            32
        );
        assert_eq!(frame(&ctx, &mut keyboard, vec![], true), 0);
        assert_eq!(
            frame(&ctx, &mut keyboard, vec![event(C, true, false)], true),
            65536 | 1048576
        );
        assert_eq!(frame(&ctx, &mut keyboard, vec![], false), 0);
        assert_eq!(frame(&ctx, &mut keyboard, vec![], true), 0);
    }
}
