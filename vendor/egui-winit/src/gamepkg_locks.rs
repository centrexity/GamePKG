//! Query only lock modifiers on the existing UI connection; never capture keys.
use winit::window::Window;

#[cfg(all(target_os = "linux", feature = "x11"))]
pub(crate) fn query(window: &Window) -> Option<(bool, bool)> {
    use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
    let RawDisplayHandle::Xlib(display) = window.display_handle().ok()?.as_raw() else {
        // An XWayland display is not authoritative for a native Wayland window.
        return None;
    };
    let display = display.display?.as_ptr().cast();
    static XLIB: std::sync::OnceLock<Option<x11_dl::xlib::Xlib>> = std::sync::OnceLock::new();
    let xlib = XLIB
        .get_or_init(|| x11_dl::xlib::Xlib::open().ok())
        .as_ref()?;
    // The caller is egui-winit's UI thread. Its Window owns this live connection.
    unsafe {
        let mut state: x11_dl::xlib::XkbStateRec = std::mem::zeroed();
        if (xlib.XkbGetState)(display, 0x100, &mut state) != 0 {
            return None;
        }
        let num = (xlib.XkbKeysymToModifiers)(display, x11_dl::keysym::XK_Num_Lock.into());
        let caps = (xlib.XkbKeysymToModifiers)(display, x11_dl::keysym::XK_Caps_Lock.into());
        let locked = u32::from(state.locked_mods);
        Some((locked & caps != 0, locked & num != 0))
    }
}
#[cfg(target_os = "windows")]
pub(crate) fn query(_: &Window) -> Option<(bool, bool)> {
    #[link(name = "user32")]
    extern "system" {
        fn GetKeyState(key: i32) -> i16;
    }
    // The low bit is the toggle state on the UI thread's input queue.
    unsafe { Some((GetKeyState(0x14) & 1 != 0, GetKeyState(0x90) & 1 != 0)) }
}
#[cfg(not(any(target_os = "windows", all(target_os = "linux", feature = "x11"))))]
pub(crate) fn query(_: &Window) -> Option<(bool, bool)> {
    None
}

#[cfg(all(test, target_os = "linux", feature = "x11"))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a private X11 display; run tests/desktop_input_capture.py --x11-locks"]
    #[allow(deprecated)]
    fn x11_lock_state_is_read_from_the_existing_window_connection() {
        use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
        use winit::platform::x11::EventLoopBuilderExtX11;
        let mut builder = winit::event_loop::EventLoop::builder();
        builder.with_x11().with_any_thread(true);
        let event_loop = builder.build().unwrap();
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();
        let RawDisplayHandle::Xlib(raw) = window.display_handle().unwrap().as_raw() else {
            panic!("X11 required")
        };
        let display = raw.display.unwrap().as_ptr().cast();
        let xlib = x11_dl::xlib::Xlib::open().unwrap();
        unsafe {
            let caps = (xlib.XkbKeysymToModifiers)(display, x11_dl::keysym::XK_Caps_Lock.into());
            let num = (xlib.XkbKeysymToModifiers)(display, x11_dl::keysym::XK_Num_Lock.into());
            assert!(caps != 0 && num != 0 && caps & num == 0);
            let mut original: x11_dl::xlib::XkbStateRec = std::mem::zeroed();
            assert_eq!((xlib.XkbGetState)(display, 0x100, &mut original), 0);
            struct Restore<'a>(&'a x11_dl::xlib::Xlib, *mut x11_dl::xlib::Display, u32, u32);
            impl Drop for Restore<'_> {
                fn drop(&mut self) {
                    unsafe {
                        (self.0.XkbLockModifiers)(self.1, 0x100, self.2, self.3);
                        (self.0.XSync)(self.1, 0);
                    }
                }
            }
            let _restore = Restore(&xlib, display, caps | num, u32::from(original.locked_mods));
            for (c, n) in [(true, false), (true, true), (false, true), (false, false)] {
                let values = if c { caps } else { 0 } | if n { num } else { 0 };
                assert_ne!(
                    (xlib.XkbLockModifiers)(display, 0x100, caps | num, values),
                    0
                );
                assert_eq!(query(&window), Some((c, n)));
                let viewport = egui::ViewportId::from_hash_of("lock-test");
                super::super::enable(viewport, true, true);
                super::super::sync_locks(viewport, &window);
                assert!(
                    matches!(super::super::take(viewport).as_slice(), [super::super::Event::Locks { caps, num }] if *caps == c && *num == n)
                );
                super::super::sync_locks(viewport, &window);
                assert!(super::super::take(viewport).is_empty());
                super::super::window_event(viewport, &winit::event::WindowEvent::Focused(false));
                super::super::take(viewport);
                assert_ne!(
                    (xlib.XkbLockModifiers)(display, 0x100, caps | num, values ^ (caps | num)),
                    0
                );
                super::super::sync_locks(viewport, &window);
                assert!(super::super::take(viewport).is_empty());
                super::super::window_event(viewport, &winit::event::WindowEvent::Focused(true));
                super::super::sync_locks(viewport, &window);
                assert!(
                    matches!(super::super::take(viewport).as_slice(), [super::super::Event::Focus(true), super::super::Event::Locks { caps, num }] if *caps == !c && *num == !n)
                );
                super::super::enable(viewport, false, false);
            }
        }
    }
}
