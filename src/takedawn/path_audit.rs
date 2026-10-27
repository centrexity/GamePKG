//! Sticky executed-path failures, independent of bounded diagnostic storage.
const MARKERS: [(&[u8], &str); 2] = [
    (
        b"[UNIMPL] untranslated instruction REACHED:",
        "an unsupported native instruction",
    ),
    (
        b"[STUB] unresolved 0x",
        "an unresolved native game function",
    ),
];

#[derive(Default)]
pub(crate) struct PathAudit {
    tail: Vec<u8>,
    failure: Option<&'static str>,
}
impl PathAudit {
    pub(crate) fn observe(&mut self, bytes: &[u8]) {
        if self.failure.is_some() {
            return;
        }
        for chunk in bytes.chunks(4096) {
            self.tail.extend_from_slice(chunk);
            for (marker, reason) in MARKERS {
                if self.tail.windows(marker.len()).any(|w| w == marker) {
                    self.failure = Some(reason);
                    return;
                }
            }
            self.tail.drain(..self.tail.len().saturating_sub(64));
        }
    }
    pub(crate) fn failure(&self) -> Option<&'static str> {
        self.failure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_survive_every_fragment_boundary_and_later_noise() {
        for (marker, reason) in MARKERS {
            for split in 0..=marker.len() {
                let mut audit = PathAudit::default();
                audit.observe(&marker[..split]);
                if split < marker.len() {
                    assert!(audit.failure().is_none());
                }
                audit.observe(&marker[split..]);
                audit.observe(&vec![b'x'; 1024 * 1024]);
                assert_eq!(audit.failure(), Some(reason));
            }
        }
    }

    #[test]
    fn verbose_normal_output_and_incomplete_markers_do_not_fail() {
        let mut audit = PathAudit::default();
        for _ in 0..100 {
            audit.observe(&[b'x'; 8192]);
            assert!(audit.tail.len() <= 64);
        }
        audit.observe(b"[UNIMPL] instruction declarations\n[STUB] declarations\n");
        audit.observe(b"[UNIMPL] untranslated instruction REACHED");
        assert!(audit.failure().is_none());
        audit.observe(b": anything-new at 0x1234\n");
        assert_eq!(audit.failure(), Some(MARKERS[0].1));
    }
}
