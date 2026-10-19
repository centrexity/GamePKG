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
    observed_fullscreen: Option<bool>,
    current: State,
    previous: State,
    staged: State,
    gesture: bool,
    homepage: Option<String>,
    link_current: u32,
    link_previous: u32,
    link_staged: u32,
    link_consumed: u32,
}
impl Service {
    pub fn new(allowed: bool) -> Self {
        Self {
            allowed,
            observed_fullscreen: None,
            current: State::default(),
            previous: State::default(),
            staged: State::default(),
            gesture: false,
            homepage: None,
            link_current: 0,
            link_previous: 0,
            link_staged: 0,
            link_consumed: 0,
        }
    }
    pub fn with_homepage(allowed: bool, homepage: Option<&str>) -> anyhow::Result<Self> {
        if let Some(url) = homepage {
            anyhow::ensure!(url.len() <= 2048, "homepage URL is too long");
            crate::registry::validate_url(url)?;
        }
        Ok(Self {
            homepage: homepage.map(str::to_owned),
            ..Self::new(allowed)
        })
    }
    /// Only the host consumes this committed request. Guest code supplies no URL.
    pub fn take_homepage(&mut self) -> Option<String> {
        if self.current.closed || self.link_current == self.link_consumed {
            return None;
        }
        self.link_consumed = self.link_current;
        self.homepage.clone()
    }
    pub fn observe_fullscreen(&mut self, fullscreen: Option<bool>) {
        self.observed_fullscreen = fullscreen;
    }
    /// Zero is unavailable; bit 0 marks an observation, bit 1 fullscreen.
    pub fn observation(&self) -> u32 {
        if !self.allowed || self.current.closed {
            return 0;
        }
        match self.observed_fullscreen {
            None => 0,
            Some(false) => 1,
            Some(true) => 3,
        }
    }
    pub fn begin(&mut self, gesture: bool) {
        self.previous = self.current;
        self.staged = self.current;
        self.gesture = gesture;
        self.link_previous = self.link_current;
        self.link_staged = self.link_current;
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
            (4, 0)
                if self.gesture
                    && self.homepage.is_some()
                    && self.link_staged == self.link_previous =>
            {
                let Some(next) = self.link_staged.checked_add(1) else {
                    return false;
                };
                self.link_staged = next;
            }
            _ => return false,
        }
        true
    }
    pub fn finish(&mut self, success: bool) {
        self.current = if success { self.staged } else { self.previous };
        self.link_current = if success {
            self.link_staged
        } else {
            self.link_previous
        };
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
    fn homepage_requests_are_bounded_transactional_and_never_accept_guest_urls() {
        for url in [
            "file:///tmp/game",
            "javascript:alert(1)",
            "https://user:password@example.org/",
            "http://example.org/",
        ] {
            assert!(Service::with_homepage(true, Some(url)).is_err());
        }
        let mut service =
            Service::with_homepage(true, Some("https://example.org/project")).unwrap();
        service.begin(false);
        assert!(!service.request(4, 0));
        service.begin(true);
        assert!(!service.request(4, 1));
        assert!(service.request(4, 0));
        assert!(!service.request(4, 0));
        assert!(service.take_homepage().is_none());
        service.finish(false);
        assert!(service.take_homepage().is_none());
        service.begin(true);
        assert!(service.request(4, 0));
        service.finish(true);
        assert_eq!(
            service.take_homepage().as_deref(),
            Some("https://example.org/project")
        );
        assert!(service.take_homepage().is_none());
        service.begin(true);
        assert!(service.request(4, 0));
        assert!(service.request(3, 0));
        service.finish(true);
        assert!(service.take_homepage().is_none());
    }
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

#[cfg(test)]
mod observation_tests {
    use super::Service;
    #[test]
    fn actual_feedback_is_independent_of_requests_rollbacks_and_other_games() {
        let mut service = Service::new(true);
        assert_eq!(service.observation(), 0);
        service.observe_fullscreen(Some(false));
        assert_eq!(service.observation(), 1);
        service.begin(true);
        assert!(service.request(2, 0));
        service.finish(true);
        assert_eq!(
            service.observation(),
            1,
            "toggle acceptance predicted physical fullscreen"
        );
        service.observe_fullscreen(Some(true));
        assert_eq!(service.observation(), 3);
        service.begin(true);
        assert!(service.request(3, 0));
        service.finish(false);
        assert_eq!(service.observation(), 3);
        let mut denied = Service::new(false);
        denied.observe_fullscreen(Some(true));
        assert_eq!(denied.observation(), 0);
        assert_eq!(Service::new(true).observation(), 0);
        service.observe_fullscreen(None);
        assert_eq!(service.observation(), 0);
        service.observe_fullscreen(Some(true));
        service.begin(true);
        service.request(3, 0);
        service.finish(true);
        assert_eq!(service.observation(), 0);
    }
}
