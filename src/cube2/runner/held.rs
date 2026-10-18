//! Last UI-observed physical state survives lease expiry, but not explicit reset.
use super::protocol;
use anyhow::{ensure, Result};
use std::collections::BTreeSet;
#[derive(Clone, Default)]
pub(super) struct Physical {
    keys: BTreeSet<u32>,
    buttons: BTreeSet<u32>,
    modifiers: u32,
}
pub(super) struct Prepared {
    pub physical: Physical,
    pub events: Vec<Vec<u8>>,
    pub restore: Vec<Vec<u8>>,
}
impl Physical {
    pub fn modifiers(&self) -> u32 {
        self.modifiers
    }
    pub fn prepare(&self, mut events: Vec<Vec<u8>>, resume: bool) -> Result<Prepared> {
        let mut next = self.clone();
        let mut touched_keys = BTreeSet::new();
        let mut touched_buttons = BTreeSet::new();
        let mut fresh_down = BTreeSet::new();
        let mut eligible = self.keys.clone();
        for event in &mut events {
            let number =
                |i| u32::from_le_bytes(event[i..i + 4].try_into().expect("checked packet"));
            if event.starts_with(b"GPI1") {
                ensure!(event.len() == 16, "invalid key recovery packet");
                let (key, flags, mods) = (number(4), number(8), number(12));
                ensure!(flags <= 3, "invalid key recovery flags");
                ensure!(
                    *event == protocol::key_event(key, flags & 1 != 0, flags & 2 != 0, mods)?,
                    "invalid key recovery packet"
                );
                touched_keys.insert(key);
                next.modifiers = mods;
                if flags & 1 != 0 {
                    next.keys.insert(key);
                    // The lease released this key. Its first repeat resumes an
                    // effective press; later repeats retain their normal meaning.
                    if resume && flags == 3 && eligible.contains(&key) && !fresh_down.contains(&key)
                    {
                        event[8..12].copy_from_slice(&1u32.to_le_bytes());
                    }
                    fresh_down.insert(key);
                } else {
                    next.keys.remove(&key);
                    fresh_down.remove(&key);
                    eligible.remove(&key);
                }
            } else if event.starts_with(b"GPC1") {
                ensure!(event.len() == 16, "invalid button recovery packet");
                let (button, pressed, mods) = (number(4), number(8), number(12));
                ensure!(
                    pressed <= 1
                        && *event == protocol::mouse_button_event(button, pressed != 0, mods)?,
                    "invalid button recovery packet"
                );
                touched_buttons.insert(button);
                next.modifiers = mods;
                if pressed != 0 {
                    next.buttons.insert(button);
                } else {
                    next.buttons.remove(&button);
                }
            } else if event.starts_with(b"GPM1") {
                ensure!(event.len() == 16, "invalid modifier recovery packet");
                let mods = number(4);
                ensure!(
                    *event == protocol::modifiers_event(mods)?,
                    "invalid modifier recovery packet"
                );
                next.modifiers = mods;
            } else if event.starts_with(b"GPW1") {
                ensure!(event.len() == 16, "invalid wheel recovery packet");
                let (x, y, mods) = (number(4) as i32, number(8) as i32, number(12));
                ensure!(
                    *event == protocol::wheel_event(x, y, mods)?,
                    "invalid wheel recovery packet"
                );
                next.modifiers = mods;
            } else if event.starts_with(b"GPR1") {
                ensure!(
                    event.as_slice() == protocol::RESET_INPUT,
                    "invalid reset recovery packet"
                );
                next = Self::default();
                eligible.clear();
                fresh_down.clear();
                touched_keys.extend(self.keys.iter().copied());
                touched_buttons.extend(self.buttons.iter().copied());
            }
        }
        let mut restore = Vec::new();
        if resume {
            let mut keys: Vec<_> = self
                .keys
                .intersection(&next.keys)
                .filter(|k| !touched_keys.contains(k))
                .copied()
                .collect();
            keys.sort_by_key(|key| (!(224..=231).contains(key), *key));
            for key in keys {
                restore.push(protocol::key_event(key, true, false, next.modifiers)?);
            }
            for button in self
                .buttons
                .intersection(&next.buttons)
                .filter(|b| !touched_buttons.contains(b))
            {
                restore.push(protocol::mouse_button_event(*button, true, next.modifiers)?);
            }
        }
        Ok(Prepared {
            physical: next,
            events,
            restore,
        })
    }
}
