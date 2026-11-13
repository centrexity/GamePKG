//! Display power changes do not necessarily suspend the process or resize its
//! HWND. Intel Vulkan surfaces can therefore remain stale after a lid cycle.
use std::cell::Cell;
use std::{mem, ptr};

use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::System::Power::{
    RegisterPowerSettingNotification, UnregisterPowerSettingNotification, HPOWERNOTIFY,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DEVICE_NOTIFY_WINDOW_HANDLE, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND,
    PBT_POWERSETTINGCHANGE,
};

use super::display_restore_state::DisplayState;

const LID: GUID = GUID::from_u128(0xba3e0f4d_b817_4094_a2d1_d56379e6a0f3);
const DISPLAY: GUID = GUID::from_u128(0x6fe69556_704a_47a0_8f24_c28d936fda47);

#[repr(C)]
#[derive(Clone, Copy)]
struct SettingHeader {
    setting: GUID,
    length: u32,
}

fn same_guid(a: GUID, b: GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

pub(super) struct DisplayNotifications {
    handles: [Cell<HPOWERNOTIFY>; 2],
    state: Cell<DisplayState>,
}

impl Default for DisplayNotifications {
    fn default() -> Self {
        Self {
            handles: [Cell::new(0), Cell::new(0)],
            state: Cell::new(DisplayState::default()),
        }
    }
}

impl DisplayNotifications {
    // Register only after GWL_USERDATA is installed: registration may deliver
    // initial state messages synchronously, including an initially closed lid.
    pub(super) unsafe fn register(&self, window: HWND) {
        for (slot, setting) in self.handles.iter().zip([LID, DISPLAY]) {
            let handle = unsafe {
                RegisterPowerSettingNotification(window, &setting, DEVICE_NOTIFY_WINDOW_HANDLE)
            };
            slot.set(handle);
            if handle == 0 {
                tracing::warn!("Unable to subscribe to Windows display power notifications");
            }
        }
    }

    pub(super) fn available(&self) -> bool {
        self.state.get().available()
    }

    // The caller supplies a WM_POWERBROADCAST payload from Windows. Read only
    // its fixed header before validating the variable-length DWORD payload.
    pub(super) unsafe fn power_message(&self, event: WPARAM, payload: LPARAM) -> bool {
        match u32::try_from(event).ok() {
            Some(PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND) => self.available(),
            Some(PBT_POWERSETTINGCHANGE) if payload != 0 => {
                let data = payload as *const u8;
                let header = unsafe { ptr::read_unaligned(data.cast::<SettingHeader>()) };
                if header.length != mem::size_of::<u32>() as u32 {
                    return false;
                }
                let value = unsafe {
                    ptr::read_unaligned(data.add(mem::size_of::<SettingHeader>()).cast::<u32>())
                };
                let mut state = self.state.get();
                let restore = if same_guid(header.setting, LID) {
                    state.lid(value)
                } else if same_guid(header.setting, DISPLAY) {
                    state.display(value)
                } else {
                    false
                };
                self.state.set(state);
                restore
            }
            _ => false,
        }
    }
}

impl Drop for DisplayNotifications {
    fn drop(&mut self) {
        for handle in &self.handles {
            let handle = handle.get();
            if handle != 0 {
                unsafe { UnregisterPowerSettingNotification(handle) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C)]
    struct Payload {
        header: SettingHeader,
        value: u32,
    }

    fn setting(n: &DisplayNotifications, guid: GUID, length: u32, value: u32) -> bool {
        let payload = Payload {
            header: SettingHeader {
                setting: guid,
                length,
            },
            value,
        };
        unsafe {
            n.power_message(
                PBT_POWERSETTINGCHANGE as WPARAM,
                &payload as *const _ as LPARAM,
            )
        }
    }

    #[test]
    fn native_payload_layout_matches_windows_dword_contract() {
        assert_eq!(mem::size_of::<SettingHeader>(), 20);
        assert_eq!(mem::size_of::<Payload>(), 24);
        let n = DisplayNotifications::default();
        assert!(!setting(&n, LID, 4, 0));
        assert!(setting(&n, LID, 4, 1));
        assert!(!setting(&n, LID, 4, 1));
    }

    #[test]
    fn malformed_and_unknown_notifications_cannot_release_a_closed_lid() {
        let n = DisplayNotifications::default();
        assert!(!setting(&n, LID, 4, 0));
        for length in [0, 1, 3, 5, u32::MAX] {
            assert!(!setting(&n, LID, length, 1));
            assert!(!n.available());
        }
        assert!(!setting(&n, GUID::from_u128(0), 4, 1));
        assert!(!n.available());
        assert!(!unsafe { n.power_message(PBT_POWERSETTINGCHANGE as WPARAM, 0) });
        assert!(!unsafe { n.power_message(0, 0) });
        assert!(setting(&n, LID, 4, 1));
    }

    #[test]
    fn system_resume_waits_for_a_closed_lid_or_dark_display() {
        let n = DisplayNotifications::default();
        assert!(unsafe { n.power_message(PBT_APMRESUMEAUTOMATIC as WPARAM, 0) });
        assert!(!setting(&n, DISPLAY, 4, 0));
        assert!(!unsafe { n.power_message(PBT_APMRESUMESUSPEND as WPARAM, 0) });
        assert!(setting(&n, DISPLAY, 4, 2));
        assert!(unsafe { n.power_message(PBT_APMRESUMESUSPEND as WPARAM, 0) });
    }
}
