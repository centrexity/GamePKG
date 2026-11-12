//! Bounded commands for the game's own host window. No window or device handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub visible: bool,
    pub confined: bool,
    pub fullscreen_toggle: bool,
    pub closed: bool,
    pub resize: Option<[u32; 2]>,
    pub resize_serial: u32,
    pub resize_origin: Option<[u32; 2]>,
    pub resize_ticks: u16,
    pub resize_revert: bool,
    /// Requested presentation preference for the next game-window launch.
    pub vsync: Option<bool>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            visible: true,
            confined: false,
            fullscreen_toggle: false,
            closed: false,
            resize: None,
            resize_serial: 0,
            resize_origin: None,
            resize_ticks: 0,
            resize_revert: false,
            vsync: None,
        }
    }
}
pub struct Service {
    allowed: bool,
    observed_fullscreen: Option<bool>,
    observed_size: Option<[u32; 2]>,
    observed_vsync: Option<bool>,
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
            observed_size: None,
            observed_vsync: None,
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
    /// The desktop host's launch preference; unavailable without persistent settings.
    pub fn observe_vsync(&mut self, vsync: Option<bool>) {
        self.observed_vsync = vsync;
    }
    /// Physical client-area pixels, supplied only by the game's host window.
    pub fn observe_size(&mut self, size: Option<[u32; 2]>) {
        self.observed_size = size
            .filter(|[width, height]| (1..=65535).contains(width) && (1..=65535).contains(height));
    }
    /// Zero is unavailable; low/high 16 bits hold physical width/height.
    pub fn size_observation(&self) -> u32 {
        if !self.allowed || self.current.closed {
            return 0;
        }
        self.observed_size
            .map(|[width, height]| width | (height << 16))
            .unwrap_or(0)
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
        // Reversion is host-owned and survives a failed subsequent game tick.
        if let Some(origin) = self.current.resize_origin {
            if self.current.resize_ticks == 0 {
                self.current.resize = Some(origin);
                self.current.resize_revert = true;
                self.current.resize_serial += 1; // Preview reserves this increment.
                self.current.resize_origin = None;
            } else {
                self.current.resize_ticks -= 1;
            }
        }
        self.previous = self.current;
        self.staged = self.current;
        self.gesture = gesture;
        self.link_previous = self.link_current;
        self.link_staged = self.link_current;
    }
    pub fn request(&mut self, operation: u32, value: u32) -> bool {
        // Query only committed preferences; a staged write or close is not visible.
        if operation == 9 {
            let preference =
                if self.allowed && !self.current.closed && self.observed_vsync.is_some() {
                    self.current.vsync.or(self.observed_vsync)
                } else {
                    None
                };
            return match value {
                0 => preference.is_some(),
                1 => preference == Some(true),
                _ => false,
            };
        }
        // Read committed state, including when a close is only staged.
        if operation == 8 && value == 0 {
            return self.allowed && !self.current.closed && self.current.resize_origin.is_some();
        }
        if !self.allowed || self.staged.closed {
            return false;
        }
        match (operation, value) {
            (10, 0..=1) if self.gesture && self.observed_vsync.is_some() => {
                self.staged.vsync = Some(value == 1);
            }
            (0, 0..=1) => self.staged.visible = value == 1,
            (1, 0..=1) => self.staged.confined = value == 1,
            (2, 0) if self.gesture => {
                self.staged.fullscreen_toggle = !self.staged.fullscreen_toggle
            }
            (3, 0) if self.gesture => {
                self.revert_resize();
                self.staged.closed = true;
            }
            (5, packed) if self.gesture && self.observed_fullscreen == Some(false) => {
                let size = [packed & 65535, packed >> 16];
                let Some(origin) = self.observed_size else {
                    return false;
                };
                if self.staged.resize_origin.is_some()
                    || self.staged.resize_serial >= u32::MAX - 1
                    || !(320..=10000).contains(&size[0])
                    || !(240..=10000).contains(&size[1])
                    || size == origin
                {
                    return false;
                }
                self.staged.resize = Some(size);
                self.staged.resize_revert = false;
                self.staged.resize_serial += 1;
                self.staged.resize_origin = Some(origin);
                self.staged.resize_ticks = 300;
            }
            (6, 0) => {
                if !self.revert_resize() {
                    return false;
                }
            }
            (7, 0) if self.gesture && self.staged.resize_origin.is_some() => {
                self.staged.resize_origin = None;
                self.staged.resize_ticks = 0;
            }
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
    fn revert_resize(&mut self) -> bool {
        let Some(origin) = self.staged.resize_origin.take() else {
            return false;
        };
        self.staged.resize = Some(origin);
        self.staged.resize_revert = true;
        self.staged.resize_serial += 1;
        self.staged.resize_ticks = 0;
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
    fn vsync_preferences_are_desktop_only_committed_and_gesture_gated() {
        let mut service = Service::new(true);
        service.begin(true);
        assert!(!service.request(9, 0));
        assert!(!service.request(9, 1));
        assert!(!service.request(10, 0));
        service.finish(true);
        service.observe_vsync(Some(true));
        service.begin(false);
        assert!(service.request(9, 0));
        assert!(service.request(9, 1));
        assert!(!service.request(10, 0));
        service.finish(true);
        service.begin(true);
        assert!(service.request(10, 0));
        assert!(service.request(9, 1)); // No staged prediction.
        service.finish(false);
        assert_eq!(service.state().vsync, None);
        service.begin(true);
        assert!(service.request(10, 0));
        assert!(!service.request(10, 2));
        assert!(!service.request(9, 2));
        service.finish(true);
        assert_eq!(service.state().vsync, Some(false));
        service.begin(true);
        assert!(service.request(9, 0));
        assert!(!service.request(9, 1));
        assert!(service.request(10, 1));
        service.finish(true);
        service.observe_vsync(None);
        assert!(!service.request(9, 0));
        service.observe_vsync(Some(false));
        service.begin(true);
        assert!(service.request(9, 1)); // Committed next-launch choice wins.
        assert!(service.request(3, 0));
        assert!(service.request(9, 0)); // A staged close cannot erase a read.
        service.finish(true);
        assert!(!service.request(9, 0));
        assert!(!service.request(10, 0));
        let mut denied = Service::new(false);
        denied.observe_vsync(Some(true));
        denied.begin(true);
        for (operation, value) in [(9, 0), (9, 1), (10, 0), (10, 1)] {
            assert!(!denied.request(operation, value));
        }
        denied.finish(true);
        assert_eq!(denied.state(), State::default());
    }
    fn resolution_service() -> Service {
        let mut service = Service::new(true);
        service.observe_fullscreen(Some(false));
        service.observe_size(Some([960, 720]));
        service
    }
    #[test]
    fn preview_query_reports_committed_state_without_prediction_or_mutation() {
        let mut service = resolution_service();
        service.begin(true);
        assert!(!service.request(8, 0));
        assert!(service.request(5, 1280 | (720 << 16)));
        assert!(!service.request(8, 0));
        service.finish(false);
        assert!(!service.request(8, 0));
        service.begin(true);
        assert!(service.request(5, 1280 | (720 << 16)));
        service.finish(true);
        let before = service.state();
        for _ in 0..64 {
            assert!(service.request(8, 0));
        }
        assert_eq!(service.state(), before);
        service.begin(true);
        assert!(service.request(7, 0));
        assert!(service.request(8, 0));
        service.finish(false);
        assert!(service.request(8, 0));
        service.begin(true);
        assert!(service.request(3, 0));
        assert!(service.request(8, 0));
        service.finish(true);
        assert!(!service.request(8, 0));
        let mut denied = Service::new(false);
        denied.current.resize_origin = Some([960, 720]);
        assert!(!denied.request(8, 0));
    }
    #[test]
    fn resolution_previews_commit_revert_confirm_and_preserve_actual_observation() {
        let mut service = resolution_service();
        let preview = 1280 | (720 << 16);
        service.begin(false);
        assert!(!service.request(5, preview));
        service.finish(true);
        service.begin(true);
        assert!(service.request(5, preview));
        assert!(!service.request(5, 1366 | (768 << 16)));
        service.finish(false);
        assert_eq!(service.state(), State::default());
        service.begin(true);
        assert!(service.request(5, preview));
        service.finish(true);
        assert_eq!(service.state().resize, Some([1280, 720]));
        assert_eq!(service.state().resize_origin, Some([960, 720]));
        assert_eq!(service.size_observation(), 960 | (720 << 16));
        service.observe_size(Some([1280, 720]));
        service.begin(false);
        assert!(!service.request(7, 0));
        assert!(!service.request(6, 1));
        assert!(service.request(6, 0));
        service.finish(false);
        assert_eq!(service.state().resize_origin, Some([960, 720]));
        service.begin(false);
        assert!(service.request(6, 0));
        service.finish(true);
        assert_eq!(service.state().resize, Some([960, 720]));
        assert!(service.state().resize_origin.is_none());
        service.observe_size(Some([960, 720]));
        service.begin(true);
        assert!(service.request(5, preview));
        service.finish(true);
        let serial = service.state().resize_serial;
        service.begin(true);
        assert!(service.request(7, 0));
        service.finish(true);
        assert_eq!(service.state().resize_serial, serial);
        for _ in 0..302 {
            service.begin(false);
            service.finish(true);
        }
        assert_eq!(service.state().resize, Some([1280, 720]));
    }
    #[test]
    fn resolution_timeout_close_bounds_and_reserved_serial_are_host_owned() {
        for fail in [false, true] {
            let mut service = resolution_service();
            service.begin(true);
            assert!(service.request(5, 1280 | (720 << 16)));
            service.finish(true);
            for _ in 0..300 {
                service.begin(false);
                service.finish(!fail);
            }
            assert!(service.state().resize_origin.is_some());
            service.begin(false);
            service.finish(!fail);
            assert_eq!(service.state().resize, Some([960, 720]));
            assert!(service.state().resize_origin.is_none());
        }
        let mut service = resolution_service();
        for size in [
            [0, 0],
            [319, 720],
            [1280, 239],
            [10001, 720],
            [1280, 10001],
            [960, 720],
        ] {
            service.begin(true);
            assert!(!service.request(5, size[0] | (size[1] << 16)));
            service.finish(true);
            assert_eq!(service.state(), State::default());
        }
        for fullscreen in [None, Some(true)] {
            service.observe_fullscreen(fullscreen);
            service.begin(true);
            assert!(!service.request(5, 1280 | (720 << 16)));
            service.finish(true);
        }
        service.observe_fullscreen(Some(false));
        service.observe_size(None);
        service.begin(true);
        assert!(!service.request(5, 1280 | (720 << 16)));
        service.finish(true);
        service.observe_size(Some([960, 720]));
        service.current.resize_serial = u32::MAX - 2;
        service.begin(true);
        assert!(service.request(5, 1280 | (720 << 16)));
        service.finish(true);
        service.begin(true);
        assert!(service.request(3, 0));
        service.finish(true);
        assert_eq!(service.state().resize_serial, u32::MAX);
        assert_eq!(service.state().resize, Some([960, 720]));
        assert!(service.state().closed && service.state().resize_origin.is_none());
        let mut denied = Service::new(false);
        denied.observe_size(Some([960, 720]));
        denied.observe_fullscreen(Some(false));
        denied.begin(true);
        for op in [5, 6, 7] {
            assert!(!denied.request(op, 0));
        }
    }
    #[test]
    fn size_observations_are_readonly_and_close_gated() {
        let mut service = Service::new(true);
        service.observe_size(Some([1280, 720]));
        let expected = 1280 | (720 << 16);
        assert_eq!(service.size_observation(), expected);
        service.begin(true);
        assert!(service.request(2, 0));
        service.finish(true);
        assert_eq!(
            service.size_observation(),
            expected,
            "request predicted physical size"
        );
        service.begin(true);
        assert!(service.request(3, 0));
        service.finish(false);
        assert_eq!(service.size_observation(), expected);
        service.begin(true);
        assert!(service.request(3, 0));
        service.finish(true);
        assert_eq!(service.size_observation(), 0);
    }
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
                closed: false,
                ..State::default()
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
