use crate::window_control::State;
use eframe::egui::{self, CursorGrab, CursorIcon, ViewportCommand};
#[derive(Clone, Copy)]
enum Restore {
    Size([u32; 2]),
    MaximizeAfterSize([u32; 2]),
    Maximized,
}
#[derive(Clone, Copy)]
struct PreviewOrigin {
    serial: u32,
    normal: [u32; 2],
    maximized: bool,
}
#[derive(Default)]
pub(super) struct Controller {
    released: bool,
    focused: bool,
    confined: bool,
    fullscreen_toggle: bool,
    resize_serial: u32,
    restore_pending: Option<Restore>,
    normal_size: Option<[u32; 2]>,
    preview_origin: Option<PreviewOrigin>,
    confirmed: bool,
    last_windowed_maximized: Option<bool>,
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
    pub fn normal_size(&mut self, size: [u32; 2]) {
        if size[0] > 0
            && size[1] > 0
            && self.preview_origin.is_none()
            && self.restore_pending.is_none()
        {
            self.normal_size = Some(size);
        }
    }
    pub fn preview_preferences(&self) -> Option<([u32; 2], bool)> {
        self.preview_origin
            .map(|origin| (origin.normal, origin.maximized))
    }
    pub fn take_confirmation(&mut self) -> Option<([u32; 2], bool)> {
        if !self.confirmed {
            return None;
        }
        let normal = self.normal_size?;
        self.confirmed = false;
        self.preview_origin = None;
        Some((normal, self.last_windowed_maximized.unwrap_or(false)))
    }
    pub fn apply_display(&mut self, ctx: &egui::Context, requested: State, blocked: bool) {
        let scale = ctx.pixels_per_point();
        let (focused, fullscreen, maximized, actual) = ctx.input(|input| {
            let viewport = input.viewport();
            (
                input.focused,
                viewport.fullscreen,
                viewport.maximized,
                viewport.inner_rect.map(|rect| {
                    let size = rect.size() * scale;
                    [size.x.round() as u32, size.y.round() as u32]
                }),
            )
        });
        if fullscreen == Some(false) {
            if let Some(maximized) = maximized {
                self.last_windowed_maximized = Some(maximized);
            }
        }
        if fullscreen == Some(false)
            && maximized == Some(false)
            && requested.resize_serial == self.resize_serial
            && self.restore_pending.is_none()
        {
            if let Some(size) = actual {
                self.normal_size = Some(size);
            }
        }
        if let Some(physical) = requested.resize_origin {
            self.confirmed = false;
            if self
                .preview_origin
                .is_none_or(|origin| origin.serial != requested.resize_serial)
            {
                self.restore_pending = None;
                self.preview_origin = Some(PreviewOrigin {
                    serial: requested.resize_serial,
                    normal: if maximized == Some(true) {
                        self.normal_size.unwrap_or(physical)
                    } else {
                        physical
                    },
                    maximized: maximized == Some(true),
                });
            }
        } else if self.preview_origin.is_some() && !requested.resize_revert {
            self.confirmed = true;
        }
        if ((focused && !blocked) || requested.resize_revert)
            && fullscreen == Some(false)
            && requested.resize_serial != self.resize_serial
        {
            if let Some(size) = requested.resize {
                let restore_max = requested.resize_revert
                    && self.preview_origin.is_some_and(|origin| origin.maximized);
                if requested.resize_revert && self.preview_origin.is_none() {
                    // The preview was canceled before this controller applied it.
                    self.resize_serial = requested.resize_serial;
                } else if maximized == Some(true) && !restore_max {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
                } else {
                    let physical = if restore_max {
                        let normal = self.preview_origin.expect("checked origin").normal;
                        self.restore_pending = Some(if maximized == Some(true) {
                            Restore::Maximized
                        } else {
                            Restore::MaximizeAfterSize(normal)
                        });
                        normal
                    } else {
                        self.restore_pending = (requested.resize_revert
                            || requested.resize_origin.is_none())
                        .then_some(Restore::Size(size));
                        size
                    };
                    if requested.resize_origin.is_none() && !requested.resize_revert {
                        self.confirmed = true;
                    }
                    if matches!(self.restore_pending, Some(Restore::Maximized)) {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
                    } else {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
                        ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(
                            physical[0] as f32 / scale,
                            physical[1] as f32 / scale,
                        )));
                    }
                    self.resize_serial = requested.resize_serial;
                }
            }
        }
        if let Some(Restore::MaximizeAfterSize(expected)) = self.restore_pending {
            if actual == Some(expected) && maximized == Some(false) && fullscreen == Some(false) {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
                self.restore_pending = Some(Restore::Maximized);
            }
        }
        let entering_waits_for_geometry = fullscreen == Some(false)
            && (requested.resize_serial != self.resize_serial
                || self.restore_pending.is_some()
                || (requested.resize_origin.is_some()
                    && requested.resize.is_some_and(|size| actual != Some(size))));
        if focused
            && !blocked
            && !requested.closed
            && !entering_waits_for_geometry
            && requested.fullscreen_toggle != self.fullscreen_toggle
        {
            if let Some(fullscreen) = fullscreen {
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
                self.fullscreen_toggle = requested.fullscreen_toggle;
            }
        }
    }
    pub fn resize_pending(&mut self, ctx: &egui::Context, requested: State) -> bool {
        if requested.resize_serial != self.resize_serial {
            return true;
        }
        let scale = ctx.pixels_per_point();
        let done = ctx.input(|input| match self.restore_pending {
            Some(Restore::Size(expected)) => {
                input.viewport().fullscreen == Some(false)
                    && input.viewport().maximized == Some(false)
                    && input.viewport().inner_rect.is_some_and(|rect| {
                        let size = rect.size() * scale;
                        [size.x.round() as u32, size.y.round() as u32] == expected
                    })
            }
            Some(Restore::Maximized) => {
                input.viewport().fullscreen == Some(false)
                    && input.viewport().maximized == Some(true)
            }
            _ => false,
        });
        if done {
            if let Some(Restore::Size(size)) = self.restore_pending {
                self.normal_size = Some(size);
            }
            self.restore_pending = None;
            if requested.resize_revert {
                self.preview_origin = None;
            }
        }
        self.restore_pending.is_some()
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
    fn physical_resize_uses_scale_once_and_waits_for_a_windowed_focused_host() {
        for scale in [1.0, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(scale);
            let mut controller = Controller::default();
            let request = State {
                resize: Some([1280, 720]),
                resize_serial: 1,
                resize_origin: Some([960, 720]),
                ..State::default()
            };
            for (focused, fullscreen, blocked, expected) in [
                (false, Some(false), false, false),
                (true, None, false, false),
                (true, Some(true), false, false),
                (true, Some(false), true, false),
                (true, Some(false), false, true),
                (true, Some(false), false, false),
            ] {
                let mut input = egui::RawInput {
                    focused,
                    ..Default::default()
                };
                let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
                viewport.fullscreen = fullscreen;
                viewport.native_pixels_per_point = Some(1.0);
                let output = ctx.run(input, |ctx| controller.apply_display(ctx, request, blocked));
                let sizes: Vec<_> = output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .iter()
                    .filter_map(|c| {
                        if let ViewportCommand::InnerSize(size) = c {
                            Some(*size)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(
                    sizes,
                    if expected {
                        vec![egui::vec2(1280.0 / scale, 720.0 / scale)]
                    } else {
                        vec![]
                    }
                );
            }
        }
    }
    #[test]
    fn reversion_runs_without_focus_and_preferences_wait_for_physical_acknowledgement() {
        for scale in [1.0, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(scale);
            let mut controller = Controller {
                resize_serial: 1,
                preview_origin: Some(PreviewOrigin {
                    serial: 1,
                    normal: [960, 720],
                    maximized: false,
                }),
                ..Default::default()
            };
            let request = State {
                resize: Some([960, 720]),
                resize_serial: 2,
                resize_revert: true,
                ..Default::default()
            };
            for (size, pending) in [([1000.0, 750.0], true), ([960.0, 720.0], false)] {
                let mut input = egui::RawInput {
                    focused: false,
                    ..Default::default()
                };
                let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
                viewport.fullscreen = Some(false);
                viewport.maximized = Some(false);
                viewport.native_pixels_per_point = Some(1.0);
                viewport.inner_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(size[0] / scale, size[1] / scale),
                ));
                let output = ctx.run(input, |ctx| controller.apply_display(ctx, request, true));
                if pending {
                    assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|c| matches!(c,ViewportCommand::InnerSize(s) if *s==egui::vec2(960.0/scale,720.0/scale))));
                }
                assert_eq!(controller.resize_pending(&ctx, request), pending);
            }
        }
    }
    fn resize_frame(
        ctx: &egui::Context,
        control: &mut Controller,
        request: State,
        size: [u32; 2],
        max: bool,
        scale: f32,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput {
            focused: true,
            ..Default::default()
        };
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.fullscreen = Some(false);
        viewport.maximized = Some(max);
        viewport.native_pixels_per_point = Some(1.0);
        viewport.inner_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale),
        ));
        ctx.run(input, |ctx| control.apply_display(ctx, request, false))
    }
    #[test]
    fn maximized_cancel_restores_normal_bounds_before_remaximizing() {
        for scale in [1.0, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(scale);
            let mut control = Controller::default();
            control.normal_size([1125, 800]);
            let preview = State {
                resize: Some([1000, 750]),
                resize_serial: 1,
                resize_origin: Some([1400, 952]),
                ..Default::default()
            };
            let output = resize_frame(&ctx, &mut control, preview, [1400, 952], true, scale);
            let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
            assert!(commands
                .iter()
                .any(|c| matches!(c, ViewportCommand::Maximized(false))));
            assert!(!commands
                .iter()
                .any(|c| matches!(c, ViewportCommand::InnerSize(_))));
            assert_eq!(control.preview_preferences(), Some(([1125, 800], true)));
            resize_frame(&ctx, &mut control, preview, [1125, 800], false, scale);
            let revert = State {
                resize: Some([1400, 952]),
                resize_serial: 2,
                resize_revert: true,
                ..Default::default()
            };
            let output = resize_frame(&ctx, &mut control, revert, [1000, 750], false, scale);
            let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
            assert!(commands.iter().any(|c|matches!(c,ViewportCommand::InnerSize(size) if *size==egui::vec2(1125.0/scale,800.0/scale))));
            assert!(!commands
                .iter()
                .any(|c| matches!(c, ViewportCommand::Maximized(true))));
            assert!(control.resize_pending(&ctx, revert));
            let output = resize_frame(&ctx, &mut control, revert, [1125, 800], false, scale);
            assert!(output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, ViewportCommand::Maximized(true))));
            assert!(control.resize_pending(&ctx, revert));
            resize_frame(&ctx, &mut control, revert, [1400, 952], true, scale);
            assert!(!control.resize_pending(&ctx, revert));
            assert!(control.preview_preferences().is_none());
        }
    }
    #[test]
    fn keeping_maximized_preview_and_coalesced_cancel_do_not_restore_stale_modes() {
        for scale in [1.0, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(scale);
            let mut control = Controller::default();
            control.normal_size([1125, 800]);
            let preview = State {
                resize: Some([1000, 750]),
                resize_serial: 1,
                resize_origin: Some([1400, 952]),
                ..Default::default()
            };
            resize_frame(&ctx, &mut control, preview, [1400, 952], true, scale);
            resize_frame(&ctx, &mut control, preview, [1125, 800], false, scale);
            let keep = State {
                resize_origin: None,
                ..preview
            };
            resize_frame(&ctx, &mut control, keep, [1000, 750], false, scale);
            assert!(!control.resize_pending(&ctx, keep));
            assert_eq!(control.take_confirmation(), Some(([1000, 750], false)));
            assert!(control.preview_preferences().is_none());
            assert!(control.take_confirmation().is_none());
            let mut fresh = Controller::default();
            let cancel = State {
                resize: Some([1400, 952]),
                resize_serial: 2,
                resize_revert: true,
                ..Default::default()
            };
            let output = resize_frame(&ctx, &mut fresh, cancel, [1400, 952], true, scale);
            assert!(!output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(
                    c,
                    ViewportCommand::InnerSize(_) | ViewportCommand::Maximized(_)
                )));
            assert!(!fresh.resize_pending(&ctx, cancel));
        }
    }
    fn fullscreen_frame(
        ctx: &egui::Context,
        control: &mut Controller,
        request: State,
        size: [u32; 2],
        scale: f32,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput {
            focused: true,
            ..Default::default()
        };
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.fullscreen = Some(true);
        viewport.maximized = Some(false);
        viewport.native_pixels_per_point = Some(1.0);
        viewport.inner_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale),
        ));
        ctx.run(input, |ctx| control.apply_display(ctx, request, false))
    }
    #[test]
    fn fullscreen_keep_receipt_uses_preview_normal_bounds_not_monitor_or_old_preferences() {
        for scale in [1.0, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(scale);
            let mut control = Controller::default();
            control.normal_size([1125, 800]);
            let preview = State {
                resize: Some([1000, 750]),
                resize_serial: 1,
                resize_origin: Some([1125, 800]),
                ..Default::default()
            };
            resize_frame(&ctx, &mut control, preview, [1125, 800], false, scale);
            resize_frame(&ctx, &mut control, preview, [1000, 750], false, scale);
            control.normal_size([1125, 800]);
            fullscreen_frame(&ctx, &mut control, preview, [1400, 1000], scale);
            let keep = State {
                resize_origin: None,
                ..preview
            };
            fullscreen_frame(&ctx, &mut control, keep, [1400, 1000], scale);
            assert!(!control.resize_pending(&ctx, keep));
            assert_eq!(control.take_confirmation(), Some(([1000, 750], false)));
            assert!(control.preview_preferences().is_none());
        }
    }
    #[test]
    fn fullscreen_pixels_never_acknowledge_a_deferred_windowed_restore() {
        for scale in [1.0, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(scale);
            let mut control = Controller::default();
            control.normal_size([1400, 1000]);
            let preview = State {
                resize: Some([1000, 750]),
                resize_serial: 1,
                resize_origin: Some([1400, 1000]),
                ..Default::default()
            };
            resize_frame(&ctx, &mut control, preview, [1400, 1000], false, scale);
            resize_frame(&ctx, &mut control, preview, [1000, 750], false, scale);
            let revert = State {
                resize: Some([1400, 1000]),
                resize_serial: 2,
                resize_revert: true,
                ..Default::default()
            };
            let output = fullscreen_frame(&ctx, &mut control, revert, [1400, 1000], scale);
            assert!(!output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(
                    c,
                    ViewportCommand::InnerSize(_) | ViewportCommand::Maximized(_)
                )));
            assert!(control.resize_pending(&ctx, revert));
            resize_frame(&ctx, &mut control, revert, [1000, 750], false, scale);
            fullscreen_frame(&ctx, &mut control, revert, [1400, 1000], scale);
            assert!(control.resize_pending(&ctx, revert));
            resize_frame(&ctx, &mut control, revert, [1400, 1000], false, scale);
            assert!(!control.resize_pending(&ctx, revert));
            let mut max = Controller::default();
            max.normal_size([1400, 1000]);
            let preview = State {
                resize_origin: Some([1400, 952]),
                ..preview
            };
            resize_frame(&ctx, &mut max, preview, [1400, 952], true, scale);
            resize_frame(&ctx, &mut max, preview, [1400, 1000], false, scale);
            let revert = State {
                resize: Some([1400, 952]),
                ..revert
            };
            resize_frame(&ctx, &mut max, revert, [1000, 750], false, scale);
            let output = fullscreen_frame(&ctx, &mut max, revert, [1400, 1000], scale);
            assert!(!output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, ViewportCommand::Maximized(true))));
            assert!(max.resize_pending(&ctx, revert));
        }
    }
    #[test]
    fn coalesced_preview_and_fullscreen_wait_for_preview_geometry() {
        for scale in [1.0, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(scale);
            let mut control = Controller::default();
            control.normal_size([1125, 800]);
            let preview = State {
                resize: Some([1000, 750]),
                resize_serial: 1,
                resize_origin: Some([1125, 800]),
                fullscreen_toggle: true,
                ..Default::default()
            };
            let output = resize_frame(&ctx, &mut control, preview, [1125, 800], false, scale);
            assert!(
                !output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .iter()
                    .any(|c| matches!(c, ViewportCommand::Fullscreen(true))),
                "fullscreen bypassed the pending geometry"
            );
            let output = resize_frame(&ctx, &mut control, preview, [1000, 750], false, scale);
            assert!(output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, ViewportCommand::Fullscreen(true))));
            let keep = State {
                resize_origin: None,
                ..preview
            };
            fullscreen_frame(&ctx, &mut control, keep, [1400, 1000], scale);
            assert!(!control.resize_pending(&ctx, keep));
            assert_eq!(control.take_confirmation(), Some(([1000, 750], false)));
        }
    }
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
                    ..State::default()
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
