//! Observe only this host window; never expose native handles to game code.
use winit::window::Window;

pub(crate) fn fullscreen(window: &Window) -> Option<bool> {
    #[cfg(all(target_os = "linux", feature = "x11"))]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if matches!(
            window.window_handle().ok()?.as_raw(),
            RawWindowHandle::Xlib(_)
        ) {
            return x11_fullscreen(window);
        }
    }
    Some(window.fullscreen().is_some())
}

#[cfg(all(target_os = "linux", feature = "x11"))]
fn x11_fullscreen(window: &Window) -> Option<bool> {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
    let RawDisplayHandle::Xlib(display) = window.display_handle().ok()?.as_raw() else {
        return None;
    };
    let RawWindowHandle::Xlib(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    let display = display.display?.as_ptr().cast();
    static XLIB: std::sync::OnceLock<Option<x11_dl::xlib::Xlib>> = std::sync::OnceLock::new();
    let xlib = XLIB
        .get_or_init(|| x11_dl::xlib::Xlib::open().ok())
        .as_ref()?;
    // UI thread only, with the connection and window borrowed from winit.
    // Xlib stores format-32 values in native unsigned-long slots on both ABIs.
    unsafe {
        let property = (xlib.XInternAtom)(display, c"_NET_WM_STATE".as_ptr(), 1);
        let fullscreen = (xlib.XInternAtom)(display, c"_NET_WM_STATE_FULLSCREEN".as_ptr(), 1);
        if property == 0 || fullscreen == 0 {
            return None;
        }
        let mut kind = 0;
        let mut format = 0;
        let mut count = 0;
        let mut remaining = 0;
        let mut data = std::ptr::null_mut();
        let status = (xlib.XGetWindowProperty)(
            display,
            handle.window,
            property,
            0,
            64,
            0,
            x11_dl::xlib::XA_ATOM,
            &mut kind,
            &mut format,
            &mut count,
            &mut remaining,
            &mut data,
        );
        let observed = if status != 0 || remaining != 0 || count > 64 {
            None
        } else if kind == 0 && count == 0 {
            Some(false)
        } else if kind != x11_dl::xlib::XA_ATOM || format != 32 {
            None
        } else if count == 0 {
            Some(false)
        } else if data.is_null() {
            None
        } else {
            Some(
                std::slice::from_raw_parts(data.cast::<std::os::raw::c_ulong>(), count as usize)
                    .contains(&fullscreen),
            )
        };
        if !data.is_null() {
            (xlib.XFree)(data.cast());
        }
        observed
    }
}

#[cfg(all(target_os = "linux", feature = "x11"))]
pub(crate) fn set_fullscreen(window: &Window, enabled: bool) {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
    let Ok(display_handle) = window.display_handle() else {
        return;
    };
    let RawDisplayHandle::Xlib(display) = display_handle.as_raw() else {
        return;
    };
    let Ok(window_handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Xlib(handle) = window_handle.as_raw() else {
        return;
    };
    let screen = display.screen;
    let Some(display) = display.display else {
        return;
    };
    static XLIB: std::sync::OnceLock<Option<x11_dl::xlib::Xlib>> = std::sync::OnceLock::new();
    let Some(xlib) = XLIB
        .get_or_init(|| x11_dl::xlib::Xlib::open().ok())
        .as_ref()
    else {
        return;
    };
    // winit may skip a request when its cached state disagrees with an external
    // WM change. Repeat the same idempotent own-window request on that connection.
    unsafe {
        let display = display.as_ptr().cast();
        let property = (xlib.XInternAtom)(display, c"_NET_WM_STATE".as_ptr(), 1);
        let fullscreen = (xlib.XInternAtom)(display, c"_NET_WM_STATE_FULLSCREEN".as_ptr(), 1);
        if property == 0 || fullscreen == 0 {
            return;
        }
        let mut data = x11_dl::xlib::ClientMessageData::new();
        data.set_long(0, i32::from(enabled).into());
        data.set_long(1, fullscreen as std::os::raw::c_long);
        data.set_long(3, 1); // EWMH source: normal application.
        let mut event = x11_dl::xlib::XEvent {
            client_message: x11_dl::xlib::XClientMessageEvent {
                type_: x11_dl::xlib::ClientMessage,
                serial: 0,
                send_event: 1,
                display,
                window: handle.window,
                message_type: property,
                format: 32,
                data,
            },
        };
        let root = (xlib.XRootWindow)(display, screen);
        (xlib.XSendEvent)(
            display,
            root,
            0,
            x11_dl::xlib::SubstructureRedirectMask | x11_dl::xlib::SubstructureNotifyMask,
            &mut event,
        );
        (xlib.XFlush)(display);
    }
}
#[cfg(not(all(target_os = "linux", feature = "x11")))]
pub(crate) fn set_fullscreen(_: &Window, _: bool) {}

#[cfg(all(test, target_os = "linux", feature = "x11"))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a private X11 display; tests/window_observation_game.py"]
    #[allow(deprecated)]
    fn bounded_own_window_properties_preserve_native_atom_width() {
        use raw_window_handle::{
            HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
        };
        use winit::platform::x11::EventLoopBuilderExtX11;
        let mut builder = winit::event_loop::EventLoop::builder();
        builder.with_x11().with_any_thread(true);
        let event_loop = builder.build().unwrap();
        let window = event_loop
            .create_window(Window::default_attributes().with_visible(false))
            .unwrap();
        let other = event_loop
            .create_window(Window::default_attributes().with_visible(false))
            .unwrap();
        let RawDisplayHandle::Xlib(display) = window.display_handle().unwrap().as_raw() else {
            panic!("X11 required")
        };
        let RawWindowHandle::Xlib(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("X11 required")
        };
        let xlib = x11_dl::xlib::Xlib::open().unwrap();
        unsafe {
            let display = display.display.unwrap().as_ptr().cast();
            let state = (xlib.XInternAtom)(display, c"_NET_WM_STATE".as_ptr(), 0);
            let full = (xlib.XInternAtom)(display, c"_NET_WM_STATE_FULLSCREEN".as_ptr(), 0);
            let other_atom = (xlib.XInternAtom)(display, c"_NET_WM_STATE_ABOVE".as_ptr(), 0);
            for (length, expected) in [
                (0, Some(false)),
                (1, Some(true)),
                (64, Some(true)),
                (65, None),
            ] {
                let mut atoms = vec![other_atom; length];
                if let Some(last) = atoms.last_mut() {
                    *last = full;
                }
                (xlib.XChangeProperty)(
                    display,
                    handle.window,
                    state,
                    x11_dl::xlib::XA_ATOM,
                    32,
                    x11_dl::xlib::PropModeReplace,
                    atoms.as_ptr().cast(),
                    length as i32,
                );
                (xlib.XSync)(display, 0);
                assert_eq!(fullscreen(&window), expected, "{length} atoms");
                assert_eq!(fullscreen(&other), Some(false));
            }
            let bytes = [b'x'];
            (xlib.XChangeProperty)(
                display,
                handle.window,
                state,
                x11_dl::xlib::XA_STRING,
                8,
                x11_dl::xlib::PropModeReplace,
                bytes.as_ptr(),
                1,
            );
            (xlib.XSync)(display, 0);
            assert_eq!(fullscreen(&window), None);
            let shorts = [7u16];
            (xlib.XChangeProperty)(
                display,
                handle.window,
                state,
                x11_dl::xlib::XA_ATOM,
                16,
                x11_dl::xlib::PropModeReplace,
                shorts.as_ptr().cast(),
                1,
            );
            (xlib.XSync)(display, 0);
            assert_eq!(fullscreen(&window), None);
            (xlib.XDeleteProperty)(display, handle.window, state);
            (xlib.XSync)(display, 0);
            assert_eq!(fullscreen(&window), Some(false));
        }
    }
}
