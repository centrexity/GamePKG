//! Ordered, bounded input from the focused game surface. No global devices.
pub const RECORD_BYTES: usize = 16;
pub const MAX_EVENTS: usize = 2048;
pub const MAX_BYTES: usize = RECORD_BYTES * MAX_EVENTS;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub kind: u32,
    pub x: f32,
    pub y: f32,
    pub code: u32,
}
impl Event {
    pub const RESET: Self = Self {
        kind: 0,
        x: 0.0,
        y: 0.0,
        code: 0,
    };
    pub fn valid(self) -> bool {
        if !self.x.is_finite() || !self.y.is_finite() {
            return false;
        }
        match self.kind {
            0 | 8 => self.x == 0.0 && self.y == 0.0 && self.code == 0,
            1 => {
                (0.0..=640.0).contains(&self.x)
                    && (0.0..=480.0).contains(&self.y)
                    && self.code <= 31
            }
            2 | 3 => {
                (0.0..=640.0).contains(&self.x)
                    && (0.0..=480.0).contains(&self.y)
                    && matches!(self.code, 1 | 2 | 4 | 8 | 16)
            }
            4 => self.x.abs() <= 4096.0 && self.y.abs() <= 4096.0 && self.code <= 2,
            5 => {
                self.x == 0.0
                    && self.y == 0.0
                    && self.code & !0x101ff == 0
                    && (1..512).contains(&(self.code & 0xffff))
            }
            6 => self.x == 0.0 && self.y == 0.0 && (1..512).contains(&self.code),
            7 => {
                self.x == 0.0
                    && self.y == 0.0
                    && self.code != 0
                    && char::from_u32(self.code).is_some()
            }
            9 | 10 => {
                let key = self.code & 0x7fff_ffff;
                self.x == 0.0
                    && self.y == 0.0
                    && (self.kind == 9 || self.code & 0x8000_0000 == 0)
                    && if key & 0x4000_0000 != 0 {
                        key & !0x4000_01ff == 0 && key & 511 != 0
                    } else {
                        key != 0 && char::from_u32(key).is_some()
                    }
            }
            _ => false,
        }
    }
    fn encode(self, bytes: &mut Vec<u8>) {
        bytes.extend(self.kind.to_le_bytes());
        bytes.extend(self.x.to_le_bytes());
        bytes.extend(self.y.to_le_bytes());
        bytes.extend(self.code.to_le_bytes());
    }
}
pub(crate) struct Queue {
    allowed: bool,
    events: Vec<Event>,
}
impl Queue {
    pub fn new(allowed: bool) -> Self {
        Self {
            allowed,
            events: Vec::new(),
        }
    }
    pub fn push(&mut self, event: Event) -> bool {
        if !self.allowed || !event.valid() {
            return false;
        }
        if self.events.len() == MAX_EVENTS {
            self.events.clear();
            self.events.push(Event::RESET);
        }
        self.events.push(event);
        true
    }
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
    pub fn begin(&mut self, events: Vec<Event>) {
        self.events.clear();
        for event in events {
            self.push(event);
        }
    }
    pub fn read(&mut self, capacity: usize) -> Option<Vec<u8>> {
        if !self.allowed
            || capacity == 0
            || capacity > MAX_BYTES
            || !capacity.is_multiple_of(RECORD_BYTES)
        {
            return None;
        }
        let count = self.events.len().min(capacity / RECORD_BYTES);
        let mut bytes = Vec::with_capacity(count * RECORD_BYTES);
        for event in self.events.drain(..count) {
            event.encode(&mut bytes);
        }
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_are_fixed_width_little_endian_and_partial_reads_do_not_repeat() {
        let mut queue = Queue::new(true);
        assert!(queue.push(Event {
            kind: 1,
            x: 123.5,
            y: 44.25,
            code: 3
        }));
        assert!(queue.push(Event {
            kind: 7,
            x: 0.0,
            y: 0.0,
            code: 0x1f600
        }));
        let bytes = queue.read(16).unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 1);
        assert_eq!(f32::from_le_bytes(bytes[4..8].try_into().unwrap()), 123.5);
        assert_eq!(f32::from_le_bytes(bytes[8..12].try_into().unwrap()), 44.25);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 3);
        assert_eq!(queue.read(16).unwrap()[12..16], 0x1f600u32.to_le_bytes());
        assert!(queue.read(16).unwrap().is_empty());
    }
    #[test]
    fn denied_and_invalid_capacities_never_consume_events() {
        let mut denied = Queue::new(false);
        assert!(!denied.push(Event::RESET));
        assert!(denied.read(16).is_none());
        let mut queue = Queue::new(true);
        queue.push(Event::RESET);
        for capacity in [0, 1, 15, 17, MAX_BYTES + 1, usize::MAX] {
            assert!(queue.read(capacity).is_none());
        }
        assert_eq!(queue.read(16).unwrap().len(), 16);
    }
    #[test]
    fn malformed_coordinates_codes_unicode_and_types_are_rejected() {
        let valid = Event {
            kind: 1,
            x: 320.0,
            y: 240.0,
            code: 0,
        };
        for event in [
            Event { kind: 99, ..valid },
            Event {
                x: f32::NAN,
                ..valid
            },
            Event {
                x: f32::INFINITY,
                ..valid
            },
            Event { x: -0.01, ..valid },
            Event { x: 640.1, ..valid },
            Event { y: 480.1, ..valid },
            Event { code: 32, ..valid },
            Event {
                kind: 2,
                code: 3,
                ..valid
            },
            Event {
                kind: 5,
                x: 0.0,
                y: 0.0,
                code: 0,
            },
            Event {
                kind: 5,
                x: 0.0,
                y: 0.0,
                code: 512,
            },
            Event {
                kind: 5,
                x: 0.0,
                y: 0.0,
                code: 0x20004,
            },
            Event {
                kind: 7,
                x: 0.0,
                y: 0.0,
                code: 0xd800,
            },
            Event {
                kind: 7,
                x: 0.0,
                y: 0.0,
                code: 0x110000,
            },
        ] {
            assert!(!event.valid(), "{event:?}");
        }
        assert!(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 65536 + 511
        }
        .valid());
        assert!(Event {
            kind: 7,
            x: 0.0,
            y: 0.0,
            code: 0x1f600
        }
        .valid());
    }
    #[test]
    fn overflow_resets_held_state_and_unread_input_expires_at_next_tick() {
        let mut queue = Queue::new(true);
        for _ in 0..MAX_EVENTS {
            queue.push(Event {
                kind: 2,
                x: 10.0,
                y: 10.0,
                code: 1,
            });
        }
        queue.push(Event {
            kind: 1,
            x: 20.0,
            y: 30.0,
            code: 0,
        });
        let events = queue.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], Event::RESET);
        queue.begin(events);
        queue.begin(Vec::new());
        assert!(queue.read(16).unwrap().is_empty());
    }
}
