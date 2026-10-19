use crate::window_control::State;
use eframe::egui::{self, CursorGrab, CursorIcon, ViewportCommand};
#[derive(Default)]
pub(super) struct Controller {
    released: bool,
    focused: bool,
    confined: bool,
    fullscreen_toggle: bool,
}
impl Controller {
    pub fn confined(&self) -> bool {
        self.confined
    }
    pub fn update(
        &mut self,
        requested: State,
        focused: bool,
        blocked: bool,
        release: bool,
        clicked: bool,
    ) -> State {
        if focused && !self.focused {
            self.released = false;
        }
        if release {
            self.released = true;
        } else if focused && clicked {
            self.released = false;
        }
        self.focused = focused;
        let active = focused && !blocked && !self.released && !requested.closed;
        self.confined = active && requested.confined;
        State {
            visible: !active || requested.visible,
            confined: self.confined,
            ..requested
        }
    }
    pub fn apply(&mut self, ctx: &egui::Context, requested: State, blocked: bool) {
        let (focused, release, clicked) = ctx.input(|input| {
            (
                input.focused,
                super::cube2_input::release_requested(input),
                input.pointer.any_pressed(),
            )
        });
        let effective = self.update(requested, focused, blocked, release, clicked);
        self.apply_display(ctx, requested, blocked);
        // Absolute confinement must never recenter or substitute relative lock.
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(if effective.confined {
            CursorGrab::Confined
        } else {
            CursorGrab::None
        }));
        ctx.send_viewport_cmd(ViewportCommand::CursorVisible(effective.visible));
        ctx.set_cursor_icon(if effective.visible {
            CursorIcon::Default
        } else {
            CursorIcon::None
        });
    }
    pub fn apply_display(&mut self, ctx: &egui::Context, requested: State, blocked: bool) {
        let (focused, fullscreen) = ctx.input(|input| (input.focused, input.viewport().fullscreen));
        if focused
            && !blocked
            && !requested.closed
            && requested.fullscreen_toggle != self.fullscreen_toggle
        {
            if let Some(fullscreen) = fullscreen {
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
                self.fullscreen_toggle = requested.fullscreen_toggle;
            }
        }
    }
    pub fn release(&mut self, ctx: &egui::Context) {
        *self = Self::default();
        release_pointer(ctx);
    }
}
pub(super) fn release_pointer(ctx: &egui::Context) {
    ctx.send_viewport_cmd(ViewportCommand::CursorGrab(CursorGrab::None));
    ctx.send_viewport_cmd(ViewportCommand::CursorVisible(true));
    ctx.set_cursor_icon(CursorIcon::Default);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_is_scoped_to_focus_and_release_latches_until_click() {
        let requested = State {
            visible: false,
            confined: true,
            ..Default::default()
        };
        let mut control = Controller::default();
        assert!(!control.update(requested, true, false, false, false).visible);
        assert!(control.confined());
        assert!(
            control
                .update(requested, false, false, false, false)
                .visible
        );
        assert!(!control.confined());
        control.update(requested, true, false, true, false);
        assert!(!control.confined());
        control.update(requested, true, false, false, false);
        assert!(!control.confined());
        control.update(requested, true, false, false, true);
        assert!(control.confined());
        assert!(control.update(requested, true, true, false, false).visible);
        assert!(!control.confined());
        assert!(
            control
                .update(
                    State {
                        closed: true,
                        ..requested
                    },
                    true,
                    false,
                    false,
                    false
                )
                .visible
        );
    }
    #[test]
    fn confining_does_not_warp_and_fullscreen_uses_current_os_state() {
        let ctx = egui::Context::default();
        let mut controller = Controller::default();
        let mut input = egui::RawInput {
            focused: true,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .fullscreen = Some(false);
        let output = ctx.run(input, |ctx| {
            controller.apply(
                ctx,
                State {
                    visible: false,
                    confined: true,
                    fullscreen_toggle: true,
                    closed: false,
                },
                false,
            )
        });
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands
            .iter()
            .any(|c| matches!(c, ViewportCommand::CursorGrab(CursorGrab::Confined))));
        assert!(!commands
            .iter()
            .any(|c| matches!(c, ViewportCommand::CursorPosition(_))));
        assert!(commands
            .iter()
            .any(|c| matches!(c, ViewportCommand::Fullscreen(true))));
    }
    #[test]
    fn fullscreen_waits_for_known_state_and_retains_pending_toggle() {
        let ctx = egui::Context::default();
        let mut controller = Controller::default();
        for (observed, toggle, expected) in [
            (None, true, None),
            (Some(false), true, Some(true)),
            (Some(true), false, Some(false)),
            (None, true, None),
            (Some(false), true, Some(true)),
        ] {
            let mut input = egui::RawInput {
                focused: true,
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .fullscreen = observed;
            let output = ctx.run(input, |ctx| {
                controller.apply_display(
                    ctx,
                    State {
                        fullscreen_toggle: toggle,
                        ..State::default()
                    },
                    false,
                )
            });
            let commands: Vec<_> = output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .filter_map(|c| {
                    if let ViewportCommand::Fullscreen(value) = c {
                        Some(*value)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(
                commands,
                expected.into_iter().collect::<Vec<_>>(),
                "{observed:?}, parity {toggle}"
            );
        }
    }
}
