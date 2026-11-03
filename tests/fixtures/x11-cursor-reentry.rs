//! Real X11 focus/enter/motion stream; driven only on a private test display.
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};
#[derive(Default)]
struct Probe {
    parent: Option<Window>,
    child: Option<Window>,
    cursor: (f64, f64),
}
impl ApplicationHandler for Probe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.parent.is_none() {
            self.parent = Some(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("gamepkg-cursor-parent")
                            .with_inner_size(LogicalSize::new(600, 400))
                            .with_position(LogicalPosition::new(0, 0)),
                    )
                    .unwrap(),
            );
            self.child = Some(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("gamepkg-cursor-child")
                            .with_inner_size(LogicalSize::new(600, 400))
                            .with_position(LogicalPosition::new(0, 0)),
                    )
                    .unwrap(),
            );
            println!("READY");
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let parent = self.parent.as_ref().is_some_and(|window| window.id() == id);
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                if parent {
                    self.cursor = (position.x, position.y);
                }
                println!(
                    "{} {} {}",
                    if parent { "P" } else { "C" },
                    position.x,
                    position.y
                );
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } if parent => println!("CLICK {} {}", self.cursor.0, self.cursor.1),
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}
fn main() {
    EventLoop::new()
        .unwrap()
        .run_app(&mut Probe::default())
        .unwrap();
}
