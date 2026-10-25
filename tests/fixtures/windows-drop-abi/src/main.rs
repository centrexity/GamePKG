#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
#[allow(dead_code)]
#[path = "../../../../vendor/winit/src/platform_impl/windows/definitions.rs"]
pub mod definitions;

// Minimal event plumbing lets this standalone fixture compile the actual
// production handler without creating a window or using the user's library.
#[cfg(windows)]
#[allow(dead_code)]
mod event {
    #[derive(Debug, PartialEq)]
    pub enum WindowEvent {
        HoveredFile(std::path::PathBuf),
        DroppedFile(std::path::PathBuf),
        HoveredFileCancelled,
    }
    #[derive(Debug, PartialEq)]
    pub enum Event<T> {
        WindowEvent {
            window_id: crate::window::WindowId,
            event: WindowEvent,
        },
        UserEvent(T),
    }
}
#[cfg(windows)]
mod window {
    #[derive(Debug, PartialEq)]
    pub struct WindowId(pub crate::platform_impl::platform::WindowId);
}
#[cfg(windows)]
mod platform_impl {
    pub mod platform {
        pub use crate::definitions;
        #[derive(Debug, PartialEq)]
        pub struct WindowId(pub isize);
    }
}
#[cfg(windows)]
#[path = "../../../../vendor/winit/src/platform_impl/windows/drop_handler.rs"]
mod production_handler;

#[cfg(windows)]
mod check {
    use super::definitions::{IDropTarget, IDropTargetVtbl, IUnknownVtbl};
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr::null_mut};
    use windows_sys::{
        core::{IUnknown, GUID, HRESULT},
        Win32::{
            Foundation::{FreeLibrary, E_NOINTERFACE, POINTL, S_OK},
            System::{
                Com::IDataObject,
                LibraryLoader::{GetProcAddress, LoadLibraryW},
            },
        },
    };

    #[repr(C)]
    struct Target {
        interface: IDropTarget,
        stage: u32,
    }
    unsafe extern "system" fn query(
        _: *mut IUnknown,
        _: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if !out.is_null() {
            unsafe { *out = null_mut() };
        }
        E_NOINTERFACE
    }
    unsafe extern "system" fn reference(_: *mut IUnknown) -> u32 {
        1
    }
    unsafe fn receive(
        this: *mut IDropTarget,
        stage: u32,
        keys: u32,
        point: POINTL,
        effect: *mut u32,
    ) -> HRESULT {
        assert!(!this.is_null() && !effect.is_null());
        // Target's first repr(C) field is the production COM interface.
        let target = unsafe { &mut *this.cast::<Target>() };
        assert_eq!(target.stage + 1, stage);
        assert_eq!(keys, stage * 16);
        assert_eq!(point.x, 1_234_567 + stage as i32);
        assert_eq!(point.y, -7_654_321 - stage as i32);
        target.stage = stage;
        unsafe { *effect = stage };
        S_OK
    }
    unsafe extern "system" fn enter(
        this: *mut IDropTarget,
        data: *const IDataObject,
        keys: u32,
        point: POINTL,
        effect: *mut u32,
    ) -> HRESULT {
        assert_eq!(data as usize, 0x1357);
        unsafe { receive(this, 1, keys, point, effect) }
    }
    unsafe extern "system" fn over(
        this: *mut IDropTarget,
        keys: u32,
        point: POINTL,
        effect: *mut u32,
    ) -> HRESULT {
        unsafe { receive(this, 2, keys, point, effect) }
    }
    unsafe extern "system" fn leave(this: *mut IDropTarget) -> HRESULT {
        let target = unsafe { &mut *this.cast::<Target>() };
        assert_eq!(target.stage, 2);
        target.stage = 3;
        S_OK
    }
    unsafe extern "system" fn drop_file(
        this: *mut IDropTarget,
        data: *const IDataObject,
        keys: u32,
        point: POINTL,
        effect: *mut u32,
    ) -> HRESULT {
        assert_eq!(data as usize, 0x1357);
        unsafe { receive(this, 4, keys, point, effect) }
    }
    const CALLBACKS: IDropTargetVtbl = IDropTargetVtbl {
        parent: IUnknownVtbl {
            QueryInterface: query,
            AddRef: reference,
            Release: reference,
        },
        DragEnter: enter,
        DragOver: over,
        DragLeave: leave,
        Drop: drop_file,
    };

    pub fn run() {
        let dll = std::env::args_os()
            .nth(1)
            .expect("native C ABI fixture DLL");
        let path: Vec<_> = dll.encode_wide().chain(Some(0)).collect();
        let library = unsafe { LoadLibraryW(path.as_ptr()) };
        assert_ne!(library, 0, "cannot load native C fixture");
        let address = unsafe { GetProcAddress(library, c"check_drop_abi".as_ptr().cast()) }
            .or_else(|| unsafe { GetProcAddress(library, c"_check_drop_abi".as_ptr().cast()) })
            .expect("native C export missing");
        // The fixture exports this exact cdecl signature. Its vtable calls use
        // the Windows SDK declarations, independently of our Rust definitions.
        let call: unsafe extern "C" fn(*mut IDropTarget) -> i32 =
            unsafe { std::mem::transmute(address) };
        let mut target = Target {
            interface: IDropTarget { lpVtbl: &CALLBACKS },
            stage: 0,
        };
        let raw = &mut target as *mut Target;
        assert_eq!(unsafe { call(raw.cast::<IDropTarget>()) }, 0);
        assert_eq!(target.stage, 4);
        let address = unsafe { GetProcAddress(library, c"check_production_drop".as_ptr().cast()) }
            .or_else(|| unsafe {
                GetProcAddress(library, c"_check_production_drop".as_ptr().cast())
            })
            .expect("native production handler export missing");
        let call: unsafe extern "C" fn(*mut IDropTarget, u32) -> i32 =
            unsafe { std::mem::transmute(address) };
        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let received = events.clone();
        let handler = crate::production_handler::FileDropHandler::new(
            123,
            Box::new(move |event| received.borrow_mut().push(event)),
        );
        let mode = std::env::args()
            .nth(2)
            .map(|s| s.parse().unwrap())
            .unwrap_or(3);
        assert_eq!(unsafe { call(handler.data.cast::<IDropTarget>(), mode) }, 0);
        if mode & 2 != 0 {
            use crate::event::{Event, WindowEvent};
            let names = [
                "C:\\Games\\雪.gamepkg".to_owned(),
                format!("C:\\{}\\games.gamepkglist", "long".repeat(80)),
            ];
            let mut expected = Vec::new();
            for name in &names {
                expected.push(WindowEvent::HoveredFile(name.into()));
            }
            expected.push(WindowEvent::HoveredFileCancelled);
            for name in &names {
                expected.push(WindowEvent::DroppedFile(name.into()));
            }
            // Rejected data emits no files and cancels no valid hover.
            expected.extend(
                names
                    .iter()
                    .map(|name| WindowEvent::HoveredFile(name.into())),
            );
            expected.push(WindowEvent::HoveredFileCancelled);
            assert_eq!(
                *events.borrow(),
                expected
                    .into_iter()
                    .map(|event| Event::WindowEvent {
                        window_id: crate::window::WindowId(
                            crate::platform_impl::platform::WindowId(123)
                        ),
                        event,
                    })
                    .collect::<Vec<_>>()
            );
        }
        drop(handler);
        assert_eq!(
            std::rc::Rc::strong_count(&events),
            1,
            "COM references leaked the handler"
        );
        assert_ne!(unsafe { FreeLibrary(library) }, 0);
        println!(
            "PASS: production Windows drop vtable matches native SDK callbacks ({}-bit)",
            usize::BITS
        );
        println!("PASS: actual production drop handler queries, events and medium ownership");
    }
}

fn main() {
    #[cfg(windows)]
    check::run();
    #[cfg(not(windows))]
    panic!("Run this fixture on Windows");
}
