//! Bounded, session-private diagnostics. No paths or native console I/O.
use super::{Error, Result};
use std::collections::VecDeque;

pub const MAX_VALUES: usize = 32;
pub const MAX_EVENTS: usize = 128;
pub const MAX_EVENT_BYTES: usize = 128 * 1024;
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const SUMMARY_BYTES: usize = 256;
/// Conservative player reservation for retained payload, incoming copy,
/// metadata and escaped summaries. Drained data is owned by the host caller.
pub const RESERVED_BYTES: u64 = 2 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub sequence: u64,
    pub monotonic_ns: u64,
    /// 0 debug, 1 information, 2 warning, 3 error.
    pub level: u32,
    /// Game-defined identifier, always scoped to its session.
    pub code: u32,
    pub values: Vec<u64>,
    /// None for numeric events; byte events retain exact data, including NULs
    /// and non-UTF-8 bytes. Human summaries always escape it.
    pub bytes: Option<Vec<u8>>,
}
#[derive(Default)]
pub(super) struct Diagnostics {
    events: VecDeque<Event>,
    last: u64,
    dropped: u64,
    payload_bytes: usize,
}
impl Diagnostics {
    pub(super) fn record(
        &mut self,
        now: u64,
        level: u32,
        code: u32,
        values: Vec<u64>,
    ) -> Result<u64> {
        if level > 3 || values.len() > MAX_VALUES {
            return Err(Error::Invalid);
        }
        self.record_event(now, level, code, values, None)
    }
    pub(super) fn record_bytes(
        &mut self,
        now: u64,
        level: u32,
        code: u32,
        bytes: Vec<u8>,
    ) -> Result<u64> {
        if level > 3 || bytes.len() > MAX_EVENT_BYTES {
            return Err(Error::Invalid);
        }
        self.record_event(now, level, code, Vec::new(), Some(bytes))
    }
    fn record_event(
        &mut self,
        now: u64,
        level: u32,
        code: u32,
        values: Vec<u64>,
        bytes: Option<Vec<u8>>,
    ) -> Result<u64> {
        let sequence = self.last.checked_add(1).ok_or(Error::Failed)?;
        let cost = values.len() * 8 + bytes.as_ref().map_or(0, Vec::len);
        // Validate everything before eviction or mutation. Storage never grows
        // with the number of calls, and each record owns its copied values.
        while self.events.len() == MAX_EVENTS || self.payload_bytes + cost > MAX_PAYLOAD_BYTES {
            let evicted = self.events.pop_front().ok_or(Error::Failed)?;
            self.payload_bytes -=
                evicted.values.len() * 8 + evicted.bytes.as_ref().map_or(0, Vec::len);
            self.dropped = self.dropped.saturating_add(1);
        }
        self.events.push_back(Event {
            sequence,
            monotonic_ns: now,
            level,
            code,
            values,
            bytes,
        });
        self.payload_bytes += cost;
        self.last = sequence;
        Ok(sequence)
    }
    pub(super) fn summary(&self) -> String {
        use std::fmt::Write;
        if self.events.is_empty() && self.dropped == 0 {
            return String::new();
        }
        let mut text = format!(
            "\n[GamePKG diagnostics: {} retained, {} evicted since drain]\n",
            self.events.len(),
            self.dropped
        );
        for event in &self.events {
            let _ = write!(
                text,
                "#{} t={}ns level={} code=0x{:08X}",
                event.sequence, event.monotonic_ns, event.level, event.code
            );
            for value in &event.values {
                let _ = write!(text, " {value:016X}");
            }
            if let Some(bytes) = &event.bytes {
                let _ = write!(text, " bytes={} \"", bytes.len());
                for byte in bytes.iter().take(SUMMARY_BYTES) {
                    for escaped in std::ascii::escape_default(*byte) {
                        text.push(char::from(escaped));
                    }
                }
                text.push('"');
                if bytes.len() > SUMMARY_BYTES {
                    let _ = write!(
                        text,
                        " [{} more bytes retained]",
                        bytes.len() - SUMMARY_BYTES
                    );
                }
            }
            text.push('\n');
        }
        text
    }
    pub(super) fn take(&mut self) -> (Vec<Event>, u64) {
        self.payload_bytes = 0;
        (
            self.events.drain(..).collect(),
            std::mem::take(&mut self.dropped),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_cannot_reuse_sequence_or_mutate_the_ring_and_drop_count_saturates() {
        let mut log = Diagnostics {
            last: u64::MAX,
            ..Diagnostics::default()
        };
        assert_eq!(log.record(0, 1, 0, vec![]), Err(Error::Failed));
        assert_eq!(
            log.record_bytes(0, 1, 0, vec![1; MAX_EVENT_BYTES]),
            Err(Error::Failed)
        );
        assert_eq!(log.payload_bytes, 0);
        assert!(log.take().0.is_empty());
        log.last = 0;
        for _ in 0..MAX_EVENTS {
            log.record(0, 1, 0, vec![]).unwrap();
        }
        log.dropped = u64::MAX;
        log.record(0, 1, 0, vec![]).unwrap();
        let (events, dropped) = log.take();
        assert_eq!(events.len(), MAX_EVENTS);
        assert_eq!(dropped, u64::MAX);
        assert_eq!(log.take(), (vec![], 0));
    }
    #[test]
    fn byte_and_numeric_payloads_share_the_budget_order_and_reclaim_on_drain() {
        let mut log = Diagnostics::default();
        for value in 0..8u8 {
            log.record_bytes(0, 1, 10, vec![value; MAX_EVENT_BYTES])
                .unwrap();
        }
        assert_eq!(log.payload_bytes, MAX_PAYLOAD_BYTES);
        log.record(0, 2, 11, vec![123; 32]).unwrap();
        assert_eq!(log.payload_bytes, 7 * MAX_EVENT_BYTES + 256);
        let (events, lost) = log.take();
        assert_eq!(lost, 1);
        assert_eq!(events.len(), 8);
        assert_eq!(events[0].sequence, 2);
        assert_eq!(events[0].bytes, Some(vec![1; MAX_EVENT_BYTES]));
        assert_eq!(events[7].sequence, 9);
        assert_eq!(events[7].values, vec![123; 32]);
        assert!(events[7].bytes.is_none());
        assert_eq!(log.payload_bytes, 0);
        log.record_bytes(0, 0, 0, vec![0, 0x1b, 0xff, b'\n', b'"', b'\\'])
            .unwrap();
        let summary = log.summary();
        assert!(summary.contains("\\x1b"));
        assert!(summary.contains("\\xff"));
        assert!(!summary.contains('\u{1b}'));
        assert!(!summary.contains('\0'));
        log.take();
        log.record_bytes(0, 0, 0, vec![b'a'; 1024]).unwrap();
        assert!(log.summary().len() < 1024);
        assert!(log.summary().contains("768 more bytes retained"));
        assert_eq!(log.take().0[0].bytes.as_ref().unwrap().len(), 1024);
    }
}
