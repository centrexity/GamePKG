//! GamePKG-only physical input hook. Disabled outside the active game viewport.
use egui::ViewportId;
use std::sync::Mutex;
use winit::{
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};
#[derive(Clone, Debug)]
pub enum Event {
    Key {
        code: KeyCode,
        pressed: bool,
        repeat: bool,
    },
    Button {
        button: MouseButton,
        pressed: bool,
    },
    Motion(f64, f64),
    Wheel(MouseScrollDelta),
    Text(String),
    Focus(bool),
    Overflow,
}
struct Capture {
    viewport: ViewportId,
    focused: bool,
    events: Vec<Event>,
}
static CAPTURE: Mutex<Option<Capture>> = Mutex::new(None);
/// Register only the game viewport, never library/settings/support windows.
pub fn enable(viewport: ViewportId, active: bool, focused: bool) {
    let mut capture = CAPTURE.lock().unwrap_or_else(|p| p.into_inner());
    if !active {
        if capture.as_ref().is_some_and(|c| c.viewport == viewport) {
            *capture = None;
        }
    } else if capture.as_ref().is_none_or(|c| c.viewport != viewport) {
        *capture = Some(Capture {
            viewport,
            focused,
            events: Vec::new(),
        });
    }
}
pub fn take(viewport: ViewportId) -> Vec<Event> {
    let mut capture = CAPTURE.lock().unwrap_or_else(|p| p.into_inner());
    capture
        .as_mut()
        .filter(|c| c.viewport == viewport)
        .map(|c| std::mem::take(&mut c.events))
        .unwrap_or_default()
}
fn push(capture: &mut Capture, event: Event) {
    if capture.events.len() >= 2048 {
        capture.events.clear();
        capture.events.push(Event::Overflow);
    } else if !matches!(capture.events.first(), Some(Event::Overflow)) {
        capture.events.push(event);
    }
}
/// Capture only text already accepted by the desktop backend's shortcut/IME
/// rules, in the same order as physical keys. Never retain preedit contents.
pub(crate) fn text(viewport: ViewportId, value: &str) {
    let mut guard = CAPTURE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(c) = guard
        .as_mut()
        .filter(|c| c.viewport == viewport && c.focused)
    {
        if value.len() > 4096 {
            c.events.clear();
            c.events.push(Event::Overflow);
        } else if !value.is_empty() {
            push(c, Event::Text(value.to_owned()));
        }
    }
}
pub(crate) fn window_event(viewport: ViewportId, event: &WindowEvent) {
    let mut guard = CAPTURE.lock().unwrap_or_else(|p| p.into_inner());
    let Some(c) = guard.as_mut().filter(|c| c.viewport == viewport) else {
        return;
    };
    if let WindowEvent::Focused(focused) = event {
        c.focused = *focused;
        push(c, Event::Focus(*focused));
        return;
    }
    if !c.focused {
        return;
    }
    match event {
        WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } if !(*is_synthetic && event.state == ElementState::Pressed) => {
            if let PhysicalKey::Code(code) = event.physical_key {
                push(
                    c,
                    Event::Key {
                        code,
                        pressed: event.state == ElementState::Pressed,
                        repeat: event.repeat,
                    },
                );
            }
        }
        WindowEvent::MouseInput { button, state, .. } => push(
            c,
            Event::Button {
                button: *button,
                pressed: *state == ElementState::Pressed,
            },
        ),
        WindowEvent::MouseWheel { delta, .. } => push(c, Event::Wheel(*delta)),
        _ => {}
    }
}
pub(crate) fn motion(viewport: ViewportId, (x, y): (f64, f64)) {
    let mut guard = CAPTURE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(c) = guard
        .as_mut()
        .filter(|c| c.viewport == viewport && c.focused)
    {
        push(c, Event::Motion(x, y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_is_scoped_focused_bounded_and_disabled_by_default() {
        let game = ViewportId::from_hash_of("gamepkg-input-test");
        let other = ViewportId::from_hash_of("gamepkg-library-test");
        motion(game, (1.0, 2.0));
        assert!(take(game).is_empty());
        enable(game, true, true);
        text(other, "library secret");
        assert!(take(game).is_empty());
        text(game, "console command");
        push(
            CAPTURE.lock().unwrap().as_mut().unwrap(),
            Event::Key {
                code: KeyCode::Enter,
                pressed: true,
                repeat: false,
            },
        );
        assert!(
            matches!(take(game).as_slice(), [Event::Text(value), Event::Key { code: KeyCode::Enter, .. }] if value == "console command")
        );
        motion(other, (1.0, 2.0));
        assert!(take(game).is_empty());
        motion(game, (3.0, -4.0));
        assert!(take(other).is_empty());
        assert!(matches!(take(game).as_slice(), [Event::Motion(3.0, -4.0)]));
        window_event(game, &WindowEvent::Focused(false));
        motion(game, (5.0, 6.0));
        text(game, "unfocused secret");
        assert!(matches!(take(game).as_slice(), [Event::Focus(false)]));
        window_event(game, &WindowEvent::Focused(true));
        take(game);
        for _ in 0..4096 {
            motion(game, (1.0, 1.0));
        }
        assert!(matches!(take(game).as_slice(), [Event::Overflow]));
        motion(game, (7.0, 8.0));
        enable(other, false, false);
        assert!(matches!(take(game).as_slice(), [Event::Motion(7.0, 8.0)]));
        motion(game, (9.0, 10.0));
        enable(game, false, false);
        motion(game, (11.0, 12.0));
        assert!(take(game).is_empty());
    }
}
