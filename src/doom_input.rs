//! Bounded desktop input for the reviewed DOOM engine; never guest-selected I/O.
use anyhow::{ensure, Result};
pub const MAX_PER_TICK: usize = 16;
pub const MAX_QUEUED: usize = 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Key { code: u8, down: bool },
    Mouse { buttons: u8, dx: i16, dy: i16 },
    Release,
}
impl Event {
    pub(crate) fn words(self) -> Result<[i32; 3]> {
        Ok(match self {
            Self::Key { code, down } => {
                ensure!(code != 0, "invalid DOOM key");
                [1, code.into(), i32::from(down)]
            }
            Self::Mouse { buttons, dx, dy } => {
                [2 | (i32::from(buttons) << 8), dx.into(), dy.into()]
            }
            Self::Release => [3, 0, 0],
        })
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn encode(self, output: &mut Vec<u8>) -> Result<()> {
        for word in self.words()? {
            output.extend(word.to_le_bytes());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_keeps_signed_motion_and_null_keys_are_rejected() {
        let mut bytes = Vec::new();
        Event::Mouse {
            buttons: 5,
            dx: -123,
            dy: 321,
        }
        .encode(&mut bytes)
        .unwrap();
        assert_eq!(
            bytes,
            [0x02, 0x05, 0, 0, 0x85, 0xff, 0xff, 0xff, 0x41, 1, 0, 0]
        );
        assert!(Event::Key {
            code: 0,
            down: true
        }
        .words()
        .is_err());
    }
}
