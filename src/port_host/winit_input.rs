//! Host-owned keyboard, mouse, Unicode/IME and touchscreen provider. Receives
//! events from the game's existing window loop; opens no second pump/window.
use super::{WindowCommand, WindowState};
use egui_winit::winit::{
    event::{
        DeviceEvent, ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
    },
    keyboard::{ModifiersState, PhysicalKey},
};
use gamepkg_native_input::{Event, Service};
use std::collections::{BTreeMap, HashMap};
const BACKEND: u32 = 0x47505749;
#[derive(Default)]
pub struct Provider {
    keyboard: u64,
    mouse: u64,
    surfaces: HashMap<egui_winit::winit::event::DeviceId, u64>,
    contacts: BTreeMap<(u64, u64), [f32; 2]>,
    modifiers: ModifiersState,
    ime: bool,
}

impl Provider {
    fn attach(&mut self, input: &mut Service) {
        if self.keyboard == 0 {
            self.keyboard = input.connect(1, BACKEND, 1).unwrap_or(0);
        }
        if self.mouse == 0 {
            self.mouse = input.connect(2, BACKEND, 1).unwrap_or(0);
        }
    }
    fn emit(
        input: &mut Service,
        device: u64,
        kind: u32,
        code: u32,
        x: f32,
        y: f32,
        contact: u64,
    ) -> bool {
        input.submit(Event {
            device,
            kind,
            code,
            x,
            y,
            contact,
            timestamp_ns: gamepkg_native_input::gamepkg_input_time_ns(),
        })
    }
    fn text(&self, input: &mut Service, text: &str) {
        for c in text.chars().filter(|c| !c.is_control()) {
            Self::emit(input, self.keyboard, 17, c as u32, 0.0, 0.0, 0);
        }
    }
    /// Only call for the owned game window. The host applies returned shortcut
    /// commands through its backend, then reports actual observed window state.
    pub fn window(
        &mut self,
        input: &mut Service,
        event: &WindowEvent,
        state: WindowState,
    ) -> Option<WindowCommand> {
        self.attach(input);
        if let WindowEvent::Focused(focused) = event {
            input.focus(*focused && !state.closed);
            if !focused {
                self.contacts.clear();
                self.modifiers = ModifiersState::empty();
            }
            return None;
        }
        if let WindowEvent::Ime(Ime::Enabled | Ime::Disabled) = event {
            self.ime = matches!(event, WindowEvent::Ime(Ime::Enabled));
            return None;
        }
        if state.closed || !state.focused {
            return None;
        }
        match event {
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if let Some(code) = super::physical_key::scancode(key) {
                        if code == 40
                            && event.state == ElementState::Pressed
                            && !event.repeat
                            && self.modifiers.alt_key()
                        {
                            Self::emit(input, self.keyboard, 2, code, 0.0, 0.0, 0);
                            return Some(WindowCommand::Fullscreen(!state.fullscreen));
                        }
                        if !(*is_synthetic && event.state == ElementState::Pressed) {
                            Self::emit(
                                input,
                                self.keyboard,
                                if event.state == ElementState::Pressed {
                                    1
                                } else {
                                    2
                                },
                                code,
                                0.0,
                                0.0,
                                0,
                            );
                        }
                    }
                }
                if !self.ime && event.state == ElementState::Pressed && !*is_synthetic {
                    if let Some(text) = &event.text {
                        self.text(input, text);
                    }
                }
            }
            WindowEvent::Ime(Ime::Enabled) => self.ime = true,
            WindowEvent::Ime(Ime::Disabled) => self.ime = false,
            WindowEvent::Ime(Ime::Commit(text)) => self.text(input, text),
            WindowEvent::CursorMoved { position, .. } => {
                Self::emit(
                    input,
                    self.mouse,
                    3,
                    0,
                    position.x as f32,
                    position.y as f32,
                    0,
                );
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let code = match button {
                    MouseButton::Left => 1,
                    MouseButton::Right => 2,
                    MouseButton::Middle => 3,
                    MouseButton::Back => 4,
                    MouseButton::Forward => 5,
                    _ => return None,
                };
                Self::emit(
                    input,
                    self.mouse,
                    if *state == ElementState::Pressed {
                        5
                    } else {
                        6
                    },
                    code,
                    0.0,
                    0.0,
                    0,
                );
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                };
                Self::emit(input, self.mouse, 7, 0, x, y, 0);
            }
            WindowEvent::Touch(touch) => {
                let id = if let Some(id) = self.surfaces.get(&touch.device_id) {
                    *id
                } else {
                    let token = self.surfaces.len() as u64 + 1;
                    let id = input.connect(4, BACKEND, token)?;
                    self.surfaces.insert(touch.device_id, id);
                    id
                };
                let key = (id, touch.id);
                let position = [
                    touch.location.x as f32 / state.width.max(1) as f32,
                    touch.location.y as f32 / state.height.max(1) as f32,
                ];
                let valid = state.width > 0
                    && state.height > 0
                    && position
                        .iter()
                        .all(|v| v.is_finite() && (0.0..=1.0).contains(v));
                let (kind, position) = match touch.phase {
                    TouchPhase::Started if valid && !self.contacts.contains_key(&key) => {
                        (8, position)
                    }
                    TouchPhase::Moved if valid && self.contacts.contains_key(&key) => (9, position),
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        let previous = *self.contacts.get(&key)?;
                        (10, if valid { position } else { previous })
                    }
                    _ => return None,
                };
                if Self::emit(input, id, kind, 0, position[0], position[1], touch.id) {
                    if kind == 10 {
                        self.contacts.remove(&key);
                    } else {
                        self.contacts.insert(key, position);
                    }
                }
            }
            _ => {}
        }
        None
    }
    pub fn device(&mut self, input: &mut Service, event: &DeviceEvent, focused: bool) {
        if !focused {
            return;
        }
        self.attach(input);
        if let DeviceEvent::MouseMotion { delta: (x, y) } = event {
            Self::emit(input, self.mouse, 4, 0, *x as f32, *y as f32, 0);
        }
    }
    pub fn disconnect(&mut self, input: &mut Service) {
        input.disconnect(self.keyboard);
        input.disconnect(self.mouse);
        for id in self.surfaces.values() {
            input.disconnect(*id);
        }
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_winit::winit::{
        dpi::PhysicalPosition,
        event::{DeviceId, Touch},
    };
    fn state() -> WindowState {
        WindowState {
            width: 200,
            height: 100,
            focused: true,
            ..Default::default()
        }
    }
    fn touch(phase: TouchPhase, id: u64, x: f64, y: f64) -> WindowEvent {
        WindowEvent::Touch(Touch {
            device_id: DeviceId::dummy(),
            phase,
            location: PhysicalPosition::new(x, y),
            force: None,
            id,
        })
    }
    #[test]
    fn native_touch_ids_cancellation_bounds_and_focus_release_are_preserved() {
        let mut p = Provider::default();
        let mut input = Service::new();
        input.focus(true);
        p.window(
            &mut input,
            &touch(TouchPhase::Started, u64::MAX, 50.0, 80.0),
            state(),
        );
        assert_eq!(input.contacts()[0].contact, u64::MAX);
        assert_eq!(input.contacts()[0].x, 0.25);
        p.window(
            &mut input,
            &touch(TouchPhase::Cancelled, u64::MAX, f64::NAN, 80.0),
            state(),
        );
        assert!(input.contacts().is_empty());
        for id in 0..1000 {
            p.window(
                &mut input,
                &touch(TouchPhase::Started, id, 50.0, 80.0),
                state(),
            );
        }
        assert_eq!(p.contacts.len(), 16);
        assert_eq!(input.contacts().len(), 16);
        p.window(&mut input, &WindowEvent::Focused(false), state());
        assert!(input.contacts().is_empty());
        assert!(p.contacts.is_empty());
        p.disconnect(&mut input);
        assert!(input.devices().is_empty());
    }
    #[test]
    fn native_mouse_relative_motion_wheel_text_and_focus_use_the_shared_queue() {
        let mut p = Provider::default();
        let mut input = Service::new();
        input.focus(true);
        p.window(
            &mut input,
            &WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(14.5, 9.25),
            },
            state(),
        );
        p.window(
            &mut input,
            &WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: MouseButton::Forward,
            },
            state(),
        );
        p.window(
            &mut input,
            &WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: MouseScrollDelta::LineDelta(0.25, -1.5),
                phase: TouchPhase::Moved,
            },
            state(),
        );
        p.device(
            &mut input,
            &DeviceEvent::MouseMotion {
                delta: (-3.5, 7.25),
            },
            true,
        );
        p.window(
            &mut input,
            &WindowEvent::Ime(Ime::Commit("é😀".into())),
            state(),
        );
        let events = input.drain(2048);
        assert!(events
            .iter()
            .any(|e| e.kind == 4 && e.x == -3.5 && e.y == 7.25));
        assert!(events
            .iter()
            .any(|e| e.kind == 7 && e.x == 0.25 && e.y == -1.5));
        assert_eq!(
            events
                .iter()
                .filter(|e| e.kind == 17)
                .map(|e| e.code)
                .collect::<Vec<_>>(),
            [0xe9, 0x1f600]
        );
        assert_eq!(input.pointer(p.mouse).unwrap().buttons, 1 << 4);
        p.window(&mut input, &WindowEvent::Focused(false), state());
        assert_eq!(input.pointer(p.mouse).unwrap().buttons, 0);
    }
}
