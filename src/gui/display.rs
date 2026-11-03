use super::{App, Work};
use eframe::egui;
#[derive(Clone, Default)]
pub(super) struct Draft {
    pub value: Option<crate::manifest::ZeldaVideo>,
    saved: Option<crate::manifest::ZeldaVideo>,
}
impl Draft {
    pub fn loaded(&mut self, current: Option<crate::manifest::ZeldaVideo>) {
        if self.value == self.saved {
            self.value = current;
        }
        self.saved = current;
    }
}
impl From<Option<crate::manifest::ZeldaVideo>> for Draft {
    fn from(value: Option<crate::manifest::ZeldaVideo>) -> Self {
        Self {
            value,
            saved: value,
        }
    }
}
impl App {
    pub(super) fn game_display_editor(&mut self, ui: &mut egui::Ui) {
        let Some(manifest) = self
            .installed
            .iter()
            .find(|m| {
                Some(&m.id) == self.selected_game.as_ref()
                    && m.builtin_engine.as_deref() == Some(crate::zelda3::PROFILE)
            })
            .cloned()
        else {
            return;
        };
        let Ok(scope) = crate::video_selections::identity(&manifest) else {
            return;
        };
        let mut chosen = None;
        egui::CollapsingHeader::new("Display").default_open(true).show(ui, |ui| {
            let draft = &mut self.video_drafts.entry(scope.clone()).or_default().value;
            let package = manifest.zelda_video.unwrap_or_default();
            ui.add_enabled_ui(self.result.is_none(), |ui| {
                let current = draft.unwrap_or(package);
                let mut mode = match *draft {
                    None => 0,
                    Some(v) if v.extra_width == 0 => 1,
                    Some(v) if v.extra_width == crate::video_selections::wide(v.extend_height).extra_width => 2,
                    Some(v) if v.extra_width == 96 => 3,
                    _ => 4,
                };
                let old_mode = mode;
                egui::ComboBox::from_label("View").selected_text(["Package default", "Original width", "Widescreen (16:9)", "Widest view", "Custom"][mode]).show_ui(ui, |ui| {
                    for (index, title) in ["Package default", "Original width", "Widescreen (16:9)", "Widest view"].into_iter().enumerate() {
                        ui.selectable_value(&mut mode, index, title);
                    }
                });
                if mode != old_mode {
                    *draft = match mode {
                        0 => None,
                        1 => Some(crate::manifest::ZeldaVideo { extra_width: 0, ..current }),
                        2 => Some(crate::manifest::ZeldaVideo { extra_width: crate::video_selections::wide(current.extend_height).extra_width, ..current }),
                        3 => Some(crate::manifest::ZeldaVideo { extra_width: 96, ..current }),
                        _ => *draft,
                    };
                }
                if let Some(video) = draft {
                    let wide = video.extra_width == crate::video_selections::wide(video.extend_height).extra_width;
                    if ui.checkbox(&mut video.extend_height, "Extended vertical view").changed() && wide {
                        video.extra_width = crate::video_selections::wide(video.extend_height).extra_width;
                    }
                }
                let mut video = draft.unwrap_or(package);
                if ui.checkbox(&mut video.no_sprite_limits, "Allow extra sprites").changed() {
                    *draft = Some(video);
                }
                ui.weak("Optional visual enhancement: bypasses the original per-line sprite limits.");
                if ui.checkbox(&mut video.dim_flashes, "Dim flashes").changed() {
                    *draft = Some(video);
                }
                ui.weak("Uses the original port’s dimmer flash effect.");
                if ui.checkbox(&mut video.skip_intro_on_keypress, "Skip intro earlier on button press").changed() {
                    *draft = Some(video);
                }
                ui.weak("Allows the original intro to be skipped at its earlier supported stage.");
                egui::CollapsingHeader::new("Gameplay enhancements").show(ui, |ui| {
                    ui.weak("Original port options. Each is off by default; some change gameplay or fix original bugs.");
                    for &(name, label) in crate::manifest::ZeldaEnhancements::CHOICES {
                        let mut enabled = video.enhancements.enabled(name).expect("reviewed enhancement");
                        let response = ui.checkbox(&mut enabled, label);
                        if name == "cancel_bird_travel" {
                            ui.weak("Press X while choosing a bird-travel destination to cancel.");
                        }
                        if response.changed() {
                            video.enhancements.set(name, enabled).expect("reviewed enhancement");
                            *draft = Some(video);
                        }
                    }
                });
                let (width, height) = draft.unwrap_or(package).dimensions();
                ui.label(format!("Game image: {width} × {height}. Proportions are preserved."));
                if ui.button("Save display settings").clicked() { chosen = Some(*draft); }
            });
            ui.weak("Save, then launch the game to apply. Package default follows the signed recipe. Presentation options require a current engine build.");
        });
        ui.add_space(12.0);
        if let Some(selection) = chosen {
            let root = match crate::registry::default_root()
                .and_then(|path| Ok(std::fs::canonicalize(path)?))
            {
                Ok(root) => root,
                Err(error) => {
                    self.status = format!("Unable to save display settings: {error:#}");
                    return;
                }
            };
            self.begin_work(move || {
                anyhow::ensure!(
                    std::fs::canonicalize(crate::registry::default_root()?)? == root,
                    "The library changed. Review display settings again."
                );
                let _change = crate::registry::LibraryChange::acquire(&root)?;
                let current = crate::registry::installed_manifest(&root, &manifest.id)?;
                anyhow::ensure!(
                    crate::video_selections::identity(&current)? == scope,
                    "The game changed. Review display settings again."
                );
                crate::video_selections::remember(&root, &current, selection)?;
                Ok(Work::VideoSaved(scope, selection))
            });
        }
    }
}
