//! Mouse emulation belongs to one complete native device/finger identity.
use super::winit::event::TouchPhase;

pub(crate) struct PointerOwner<D> {
    owner: Option<(D, u64)>,
}
impl<D> Default for PointerOwner<D> {
    fn default() -> Self {
        Self { owner: None }
    }
}
impl<D: Copy + Eq> PointerOwner<D> {
    pub(crate) fn cancel(&mut self) -> bool {
        self.owner.take().is_some()
    }
    pub(crate) fn accepts(&mut self, device: D, finger: u64, phase: TouchPhase) -> bool {
        let key = (device, finger);
        match phase {
            TouchPhase::Started if self.owner.is_none() => {
                self.owner = Some(key);
                true
            }
            TouchPhase::Moved => self.owner == Some(key),
            TouchPhase::Ended | TouchPhase::Cancelled if self.owner == Some(key) => {
                self.owner = None;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matching_finger_numbers_on_other_devices_never_steal_or_release_the_pointer() {
        use TouchPhase::*;
        for device in [0, 1, u64::MAX] {
            for finger in [0, 1, u64::MAX] {
                let mut owner = PointerOwner::default();
                assert!(owner.accepts(device, finger, Started));
                assert!(!owner.accepts(device, finger, Started));
                for phase in [Started, Moved, Ended, Cancelled] {
                    assert!(!owner.accepts(device ^ 1, finger, phase));
                    assert!(!owner.accepts(device, finger ^ 1, phase));
                }
                assert!(owner.accepts(device, finger, Moved));
                assert!(owner.accepts(device, finger, Ended));
                for phase in [Moved, Ended, Cancelled] {
                    assert!(!owner.accepts(device, finger, phase));
                }
                assert!(owner.accepts(device ^ 1, finger, Started));
                assert!(owner.accepts(device ^ 1, finger, Cancelled));
                assert!(owner.accepts(device, finger, Started));
            }
        }
    }
    #[test]
    fn orphan_moves_and_cancels_cannot_create_mouse_gestures() {
        let mut owner = PointerOwner::default();
        for device in 0..4 {
            for finger in 0..4 {
                for phase in [TouchPhase::Moved, TouchPhase::Ended, TouchPhase::Cancelled] {
                    assert!(!owner.accepts(device, finger, phase));
                }
            }
        }
        assert!(owner.accepts(1, 2, TouchPhase::Started));
        assert!(owner.cancel());
        assert!(!owner.cancel());
        assert!(!owner.accepts(1, 2, TouchPhase::Moved));
        assert!(owner.accepts(1, 2, TouchPhase::Started));
    }
}
