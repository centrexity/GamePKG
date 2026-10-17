//! Bounded commands for the game's own host window. No window or device handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub visible: bool,
    pub confined: bool,
    pub fullscreen_toggle: bool,
    pub closed: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            visible: true,
            confined: false,
            fullscreen_toggle: false,
            closed: false,
        }
    }
}
pub struct Service {
    allowed: bool,
    current: State,
    previous: State,
    staged: State,
    gesture: bool,
}
impl Service {
    pub fn new(allowed: bool) -> Self {
        Self {
            allowed,
            current: State::default(),
            previous: State::default(),
            staged: State::default(),
            gesture: false,
        }
    }
    pub fn begin(&mut self, gesture: bool) {
        self.previous = self.current;
        self.staged = self.current;
        self.gesture = gesture;
    }
    pub fn request(&mut self, operation: u32, value: u32) -> bool {
        if !self.allowed || self.staged.closed {
            return false;
        }
        match (operation, value) {
            (0, 0..=1) => self.staged.visible = value == 1,
            (1, 0..=1) => self.staged.confined = value == 1,
            (2, 0) if self.gesture => {
                self.staged.fullscreen_toggle = !self.staged.fullscreen_toggle
            }
            (3, 0) if self.gesture => self.staged.closed = true,
            _ => return false,
        }
        true
    }
    pub fn finish(&mut self, success: bool) {
        self.current = if success { self.staged } else { self.previous };
        self.gesture = false;
    }
    pub fn state(&self) -> State {
        self.current
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permissions_arguments_and_tick_commit_are_enforced() {
        let mut denied = Service::new(false);
        denied.begin(true);
        for op in 0..4 {
            assert!(!denied.request(op, 0));
        }
        denied.finish(true);
        assert_eq!(denied.state(), State::default());
        let mut service = Service::new(true);
        service.begin(false);
        assert!(service.request(0, 0));
        assert!(service.request(1, 1));
        assert!(!service.request(2, 0));
        assert!(!service.request(3, 0));
        assert_eq!(service.state(), State::default());
        service.finish(false);
        assert_eq!(service.state(), State::default());
        service.begin(true);
        for (op, value) in [(0, 2), (1, u32::MAX), (2, 1), (3, 1), (4, 0), (u32::MAX, 0)] {
            assert!(!service.request(op, value));
        }
        service.request(0, 0);
        service.request(1, 1);
        service.request(2, 0);
        service.finish(true);
        assert_eq!(
            service.state(),
            State {
                visible: false,
                confined: true,
                fullscreen_toggle: true,
                closed: false
            }
        );
        service.finish(false);
        assert_eq!(
            service.state(),
            State::default(),
            "host failure did not roll back an undisplayed command"
        );
    }
    #[test]
    fn fullscreen_toggles_preserve_parity_and_close_is_terminal() {
        let mut service = Service::new(true);
        service.begin(true);
        service.request(2, 0);
        service.request(2, 0);
        service.finish(true);
        assert!(!service.state().fullscreen_toggle);
        service.begin(true);
        service.request(3, 0);
        assert!(!service.request(0, 0));
        service.finish(true);
        assert!(service.state().closed);
        service.begin(true);
        assert!(!service.request(1, 1));
    }
}
