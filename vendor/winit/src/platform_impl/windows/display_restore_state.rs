//! Tracks whether a previously unavailable display can present again.
#[derive(Clone, Copy, Default)]
pub(super) struct DisplayState {
    lid: Option<bool>,
    display: Option<bool>,
}

impl DisplayState {
    pub(super) fn available(self) -> bool {
        self.lid != Some(false) && self.display != Some(false)
    }

    pub(super) fn lid(&mut self, value: u32) -> bool {
        let open = match value {
            0 => false,
            1 => true,
            _ => return false,
        };
        let before = self.available();
        self.lid = Some(open);
        !before && self.available()
    }

    pub(super) fn display(&mut self, value: u32) -> bool {
        let on = match value {
            0 => false,
            1 | 2 => true, // Both lit and dimmed panels can present.
            _ => return false,
        };
        let before = self.available();
        self.display = Some(on);
        !before && self.available()
    }
}

#[cfg(test)]
mod tests {
    use super::DisplayState;

    #[test]
    fn initial_notifications_do_not_resize_a_new_window() {
        let mut s = DisplayState::default();
        assert!(!s.lid(1));
        assert!(!s.display(1));
        assert!(!s.display(2));
        assert!(s.available());
    }

    #[test]
    fn closed_lid_recovers_once_without_a_monitor_notification() {
        let mut s = DisplayState::default();
        assert!(!s.lid(0));
        assert!(!s.available());
        assert!(s.lid(1));
        assert!(!s.lid(1));
    }

    #[test]
    fn display_can_recover_without_a_lid_device() {
        let mut s = DisplayState::default();
        assert!(!s.display(0));
        assert!(s.display(1));
        assert!(!s.display(1));
    }

    #[test]
    fn either_notification_order_waits_for_both_devices() {
        for lid_first in [false, true] {
            let mut s = DisplayState::default();
            assert!(!s.lid(0));
            assert!(!s.display(0));
            if lid_first {
                assert!(!s.lid(1));
                assert!(s.display(1));
            } else {
                assert!(!s.display(1));
                assert!(s.lid(1));
            }
            assert!(!s.lid(1));
            assert!(!s.display(1));
        }
    }

    #[test]
    fn dimming_is_not_a_suspend_and_a_dimmed_panel_can_return() {
        let mut s = DisplayState::default();
        assert!(!s.display(1));
        assert!(!s.display(2));
        assert!(!s.display(1));
        assert!(!s.display(0));
        assert!(s.display(2));
    }

    #[test]
    fn invalid_values_do_not_erase_an_unavailable_state() {
        let mut s = DisplayState::default();
        s.lid(0);
        s.display(0);
        for value in [2, 3, u32::MAX] {
            assert!(!s.lid(value));
            assert!(!s.available());
        }
        for value in [3, u32::MAX] {
            assert!(!s.display(value));
            assert!(!s.available());
        }
        assert!(!s.lid(1));
        assert!(s.display(1));
    }

    #[test]
    fn repeated_close_open_cycles_each_recover_once() {
        let mut s = DisplayState::default();
        for _ in 0..1000 {
            assert!(!s.lid(0));
            assert!(!s.lid(0));
            assert!(s.lid(1));
            assert!(!s.lid(1));
        }
    }
}
