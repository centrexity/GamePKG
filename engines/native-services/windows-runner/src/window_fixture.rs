//! Actual Win32 window access inside the production AppContainer and job.
//! This verifies native window ownership/painting/resize/close, not Vulkan or
//! original-game rendering. No ROM, graphics fixture DLL or extra capability.
use std::{
    ffi::OsStr,
    io,
    mem::zeroed,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        CreateSolidBrush, GdiFlush, GetDC, GetPixel, InvalidateRect, ReleaseDC, UpdateWindow,
    },
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentProcessId},
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowThreadProcessId,
        IsWindow, RegisterClassW, SetWindowPos, ShowWindow, UnregisterClassW, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_SHOWNOACTIVATE, WNDCLASSW,
        WS_OVERLAPPEDWINDOW,
    },
};

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: this class only delegates Windows-owned window messages to the
    // system procedure, on the thread that created the window.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn check_paint(hwnd: HWND) -> RECT {
    // SAFETY: hwnd is this fixture's live window. RECT and all API output
    // pointers are initialized storage; the DC is released before returning.
    unsafe {
        let mut rect: RECT = zeroed();
        assert_ne!(
            GetClientRect(hwnd, &mut rect),
            0,
            "{}",
            io::Error::last_os_error()
        );
        assert!(rect.right > 100 && rect.bottom > 50);
        assert_ne!(InvalidateRect(hwnd, null(), 1), 0);
        assert_ne!(UpdateWindow(hwnd), 0);
        let dc = GetDC(hwnd);
        assert!(!dc.is_null(), "{}", io::Error::last_os_error());
        GdiFlush();
        let pixel = GetPixel(dc, rect.right / 2, rect.bottom / 2);
        assert_ne!(ReleaseDC(hwnd, dc), 0);
        assert_eq!(pixel, 0x0057_3311, "native window background did not paint");
        rect
    }
}

pub(crate) fn verify() {
    let class = crate::wide(OsStr::new("GamePKGConfinedWindowFixture")).unwrap();
    let title = crate::wide(OsStr::new("GamePKG native window fixture")).unwrap();
    // SAFETY: all strings outlive class/window registration. The window belongs
    // to this child process, is shown without activating, and is destroyed before
    // unregistering its class. Windows owns the registered background brush.
    unsafe {
        let instance = GetModuleHandleW(null());
        assert!(!instance.is_null());
        let brush = CreateSolidBrush(0x0057_3311);
        assert!(!brush.is_null());
        let class_info = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hbrBackground: brush,
            ..zeroed()
        };
        assert_ne!(
            RegisterClassW(&class_info),
            0,
            "{}",
            io::Error::last_os_error()
        );
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            30,
            30,
            320,
            200,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        assert!(!hwnd.is_null(), "{}", io::Error::last_os_error());
        let mut owner = 0;
        assert_ne!(GetWindowThreadProcessId(hwnd, &mut owner), 0);
        assert_eq!(owner, GetCurrentProcessId());
        // Keep the fixture's own pixels above a runner console without taking
        // keyboard focus, so occlusion cannot invalidate the paint sample.
        assert_ne!(
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE
            ),
            0
        );
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let small = check_paint(hwnd);
        assert_ne!(
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                640,
                400,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOZORDER
            ),
            0,
            "{}",
            io::Error::last_os_error()
        );
        let large = check_paint(hwnd);
        assert!(large.right > small.right && large.bottom > small.bottom);
        assert_ne!(DestroyWindow(hwnd), 0);
        assert_eq!(IsWindow(hwnd), 0);
        assert_ne!(UnregisterClassW(class.as_ptr(), instance), 0);
    }
    println!("CONFINED_NATIVE_WINDOW_OK");
}
