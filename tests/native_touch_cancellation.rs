#![cfg(all(feature = "gui", target_os = "linux"))]
use eframe::egui;
use egui_winit::winit::{
    self,
    application::ApplicationHandler,
    event::{DeviceId, Touch, TouchPhase, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    platform::x11::EventLoopBuilderExtX11,
    window::{Window, WindowId},
};

fn touch(state: &mut egui_winit::State, window: &Window, id: u64, phase: TouchPhase) {
    let _ = state.on_window_event(
        window,
        &WindowEvent::Touch(Touch {
            device_id: DeviceId::dummy(),
            id,
            phase,
            location: winit::dpi::PhysicalPosition::new(20.0, 20.0),
            force: None,
        }),
    );
}
fn observed(
    state: &mut egui_winit::State,
    window: &Window,
    ctx: &egui::Context,
    time: f64,
) -> (bool, bool, bool) {
    let mut input = state.take_egui_input(window);
    input.time = Some(time);
    let mut result = (false, false, false);
    let _ = ctx.run(input, |ctx| {
        result = ctx.input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.button_released(egui::PointerButton::Primary),
                i.pointer.button_clicked(egui::PointerButton::Primary),
            )
        });
    });
    result
}
#[derive(Default)]
struct Proof {
    ran: bool,
}
impl ApplicationHandler for Proof {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.ran {
            return;
        }
        self.ran = true;
        let window = event_loop
            .create_window(
                Window::default_attributes().with_title("GamePKG private touch cancellation proof"),
            )
            .unwrap();
        let ctx = egui::Context::default();
        let mut state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(1.0),
            None,
            None,
        );
        let _ = state.on_window_event(&window, &WindowEvent::Focused(true));
        touch(&mut state, &window, 7, TouchPhase::Started);
        assert!(observed(&mut state, &window, &ctx, 0.0).0);
        touch(&mut state, &window, 8, TouchPhase::Cancelled);
        assert_eq!(
            observed(&mut state, &window, &ctx, 0.01),
            (true, false, false)
        );
        touch(&mut state, &window, 7, TouchPhase::Cancelled);
        assert_eq!(
            observed(&mut state, &window, &ctx, 0.02),
            (false, true, false)
        );
        touch(&mut state, &window, 7, TouchPhase::Ended);
        assert_eq!(
            observed(&mut state, &window, &ctx, 0.03),
            (false, false, false)
        );
        touch(&mut state, &window, 7, TouchPhase::Started);
        touch(&mut state, &window, 7, TouchPhase::Cancelled);
        assert_eq!(
            observed(&mut state, &window, &ctx, 0.04),
            (false, true, false)
        );
        touch(&mut state, &window, 7, TouchPhase::Started);
        assert!(observed(&mut state, &window, &ctx, 0.05).0);
        let _ = state.on_window_event(&window, &WindowEvent::Focused(false));
        assert_eq!(
            observed(&mut state, &window, &ctx, 0.06),
            (false, true, false)
        );
        let _ = state.on_window_event(&window, &WindowEvent::Focused(true));
        touch(&mut state, &window, 7, TouchPhase::Moved);
        assert_eq!(
            observed(&mut state, &window, &ctx, 0.07),
            (false, false, false)
        );
        touch(&mut state, &window, 7, TouchPhase::Started);
        touch(&mut state, &window, 7, TouchPhase::Ended);
        assert_eq!(
            observed(&mut state, &window, &ctx, 0.08),
            (false, true, true)
        );
        event_loop.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}
#[test]
#[ignore = "requires an isolated X11 display"]
fn actual_native_touch_cancel_and_focus_loss_never_click_or_stick() {
    let event_loop = EventLoop::builder()
        .with_x11()
        .with_any_thread(true)
        .build()
        .unwrap();
    let mut proof = Proof::default();
    event_loop.run_app(&mut proof).unwrap();
    assert!(proof.ran);
}
