//! Original slot controls: native edges reject OS repeats/synthetic focus
//! presses; egui's matching press supplies its exact OS modifier snapshot.
use eframe::egui;
use egui_winit::{gamepkg_input::Event, winit::keyboard::KeyCode};
use std::collections::HashSet;
pub(super) fn settings(ui: &mut egui::Ui, settings: &mut crate::settings::Settings) {
    ui.collapsing("Snapshot shortcuts", |ui| {
        ui.label("For games with snapshot support. F1–F10 loads; Shift+F1–F10 saves. Choose any of the 20 slots, or turn a shortcut off. Saving replaces that slot.");
        egui::Grid::new("snapshot_shortcuts").striped(true).show(ui, |ui| {
            ui.label("Key"); ui.label("Load slot"); ui.label("Save slot (Shift)"); ui.end_row();
            for key in 0..10 {
                ui.label(format!("F{}", key + 1));
                for (save, slots) in [(false, &mut settings.snapshot_load_slots), (true, &mut settings.snapshot_save_slots)] {
                    let target = &mut slots[key];
                    egui::ComboBox::from_id_salt(("snapshot_slot", key, save))
                        .selected_text(if *target < 20 {format!("{}", *target + 1)} else {"Off".into()})
                        .show_ui(ui, |ui| {
                            ui.selectable_value(target, 20, "Off");
                            for slot in 0..20 {ui.selectable_value(target, slot, format!("{}", slot + 1));}
                        });
                }
                ui.end_row();
            }
        });
        if ui.button("Reset snapshot shortcuts").clicked() {
            settings.snapshot_load_slots = std::array::from_fn(|i| i as u8);
            settings.snapshot_save_slots = settings.snapshot_load_slots;
        }
    });
}
const PHYSICAL: [KeyCode; 10] = [
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
];
const LOGICAL: [egui::Key; 10] = [
    egui::Key::F1,
    egui::Key::F2,
    egui::Key::F3,
    egui::Key::F4,
    egui::Key::F5,
    egui::Key::F6,
    egui::Key::F7,
    egui::Key::F8,
    egui::Key::F9,
    egui::Key::F10,
];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Save(u8),
    Load(u8),
}
#[derive(Default)]
pub(super) struct Desktop {
    keys: HashSet<KeyCode>,
}
impl Desktop {
    pub fn request(
        &mut self,
        events: &[Event],
        input: &egui::InputState,
        settings: &crate::settings::Settings,
    ) -> Option<Action> {
        if !input.focused {
            self.keys.clear();
            return None;
        }
        let mut active = true;
        let mut action = None;
        let mut translated = input.events.iter();
        for event in events {
            match event {
                Event::Focus(value) => {
                    self.keys.clear();
                    active = *value;
                    if !active {
                        action = None;
                    }
                }
                Event::Overflow => {
                    self.keys.clear();
                    active = false;
                    action = None;
                }
                Event::Key {
                    code,
                    pressed,
                    repeat,
                } => {
                    let Some(slot) = PHYSICAL.iter().position(|key| key == code) else {
                        continue;
                    };
                    if !pressed {
                        self.keys.remove(code);
                        continue;
                    }
                    // Consume each matching press in order, including repeats,
                    // so two presses in one frame retain their own modifiers.
                    let modifiers = translated.find_map(|event| match event {
                        egui::Event::Key {
                            key,
                            physical_key,
                            pressed: true,
                            modifiers,
                            ..
                        } if physical_key.unwrap_or(*key) == LOGICAL[slot] => Some(*modifiers),
                        _ => None,
                    });
                    if !active {
                        continue;
                    }
                    let fresh = self.keys.insert(*code);
                    if *repeat || !fresh {
                        continue;
                    }
                    let Some(modifiers) = modifiers else {
                        continue;
                    };
                    if modifiers.ctrl || modifiers.command || modifiers.alt {
                        continue;
                    }
                    if action.is_none() {
                        let target = if modifiers.shift {
                            settings.snapshot_save_slots[slot]
                        } else {
                            settings.snapshot_load_slots[slot]
                        };
                        if target < 20 {
                            action = Some(if modifiers.shift {
                                Action::Save(target)
                            } else {
                                Action::Load(target)
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        action.filter(|_| active)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_slot_can_be_assigned_and_disabled_shortcuts_do_not_consume_other_actions() {
        for slot in 0..20 {
            let mut settings = crate::settings::Settings::default();
            settings.snapshot_load_slots[0] = slot;
            settings.snapshot_save_slots[0] = slot;
            let events = [key(KeyCode::F1, true, false)];
            for (modifiers, expected) in [
                (egui::Modifiers::NONE, Action::Load(slot)),
                (egui::Modifiers::SHIFT, Action::Save(slot)),
            ] {
                assert_eq!(
                    Desktop::default().request(
                        &events,
                        &input(&events, true, modifiers),
                        &settings
                    ),
                    Some(expected)
                );
            }
        }
        let mut settings = crate::settings::Settings::default();
        settings.snapshot_load_slots[0] = 20;
        settings.snapshot_save_slots[0] = 20;
        let events = [key(KeyCode::F1, true, false), key(KeyCode::F2, true, false)];
        for (modifiers, expected) in [
            (egui::Modifiers::NONE, Action::Load(1)),
            (egui::Modifiers::SHIFT, Action::Save(1)),
        ] {
            assert_eq!(
                Desktop::default().request(&events, &input(&events, true, modifiers), &settings),
                Some(expected)
            );
        }
    }
    fn key(code: KeyCode, pressed: bool, repeat: bool) -> Event {
        Event::Key {
            code,
            pressed,
            repeat,
        }
    }
    fn input(events: &[Event], focused: bool, modifiers: egui::Modifiers) -> egui::InputState {
        let mut input = egui::InputState::default();
        input.focused = focused;
        input.events = events
            .iter()
            .filter_map(|event| match event {
                Event::Key {
                    code,
                    pressed,
                    repeat,
                } => PHYSICAL
                    .iter()
                    .position(|key| key == code)
                    .map(|slot| egui::Event::Key {
                        key: LOGICAL[slot],
                        physical_key: Some(LOGICAL[slot]),
                        pressed: *pressed,
                        repeat: *repeat,
                        modifiers,
                    }),
                _ => None,
            })
            .collect();
        input
    }
    fn frame(
        desktop: &mut Desktop,
        events: &[Event],
        focused: bool,
        modifiers: egui::Modifiers,
    ) -> Option<Action> {
        desktop.request(
            events,
            &input(events, focused, modifiers),
            &Default::default(),
        )
    }
    #[test]
    fn original_bindings_use_press_time_modifiers_even_when_shift_predates_focus() {
        for (slot, code) in PHYSICAL.into_iter().enumerate() {
            let mut desktop = Desktop::default();
            let events = [key(code, true, false)];
            assert_eq!(
                frame(&mut desktop, &events, true, Default::default()),
                Some(Action::Load(slot as u8))
            );
            assert_eq!(
                frame(
                    &mut desktop,
                    &[key(code, true, true)],
                    true,
                    Default::default()
                ),
                None
            );
            assert_eq!(frame(&mut desktop, &events, true, Default::default()), None);
            let events = [key(code, false, false), key(code, true, false)];
            assert_eq!(
                frame(
                    &mut desktop,
                    &events,
                    true,
                    egui::Modifiers {
                        shift: true,
                        ..Default::default()
                    }
                ),
                Some(Action::Save(slot as u8))
            );
        }
        let events = [key(KeyCode::F1, true, false)];
        for modifiers in [
            egui::Modifiers::CTRL,
            egui::Modifiers::ALT,
            egui::Modifiers::COMMAND,
        ] {
            assert_eq!(
                frame(&mut Desktop::default(), &events, true, modifiers),
                None
            );
        }
        let events = [
            key(KeyCode::F1, true, false),
            key(KeyCode::F1, false, false),
            key(KeyCode::F1, true, false),
        ];
        let mut translated = input(&events, true, Default::default());
        if let egui::Event::Key { modifiers, .. } = &mut translated.events[0] {
            modifiers.ctrl = true;
        }
        if let egui::Event::Key { modifiers, .. } = &mut translated.events[2] {
            modifiers.shift = true;
        }
        assert_eq!(
            Desktop::default().request(&events, &translated, &Default::default()),
            Some(Action::Save(0))
        );
    }
    #[test]
    fn focus_overflow_repeats_and_unmatched_presses_cannot_overwrite_slots() {
        use KeyCode::*;
        let mut desktop = Desktop::default();
        let mut check =
            |events: &[Event], focused| frame(&mut desktop, events, focused, Default::default());
        assert_eq!(check(&[key(F1, true, false)], false), None);
        assert_eq!(
            check(
                &[
                    Event::Focus(false),
                    key(F1, true, false),
                    Event::Focus(true),
                    key(F1, true, true)
                ],
                true
            ),
            None
        );
        assert_eq!(
            check(&[key(F1, false, false), key(F1, true, false)], true),
            Some(Action::Load(0))
        );
        assert_eq!(
            check(
                &[
                    key(F1, false, false),
                    key(F1, true, false),
                    Event::Focus(false),
                    Event::Focus(true)
                ],
                true
            ),
            None
        );
        assert_eq!(check(&[key(F1, true, false), Event::Overflow], true), None);
        assert_eq!(
            check(&[key(F2, true, false), key(F3, true, false)], true),
            Some(Action::Load(1))
        );
        assert_eq!(check(&[key(F3, true, false)], true), None);
        assert_eq!(check(&[key(F11, true, false)], true), None);
        assert_eq!(
            Desktop::default().request(
                &[key(F1, true, false)],
                &input(&[], true, Default::default()),
                &Default::default()
            ),
            None
        );
    }
}
