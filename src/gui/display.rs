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

fn launch_choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    choices: &[T],
    name: impl Fn(T) -> &'static str,
) {
    egui::ComboBox::from_label(label)
        .selected_text(name(*value))
        .show_ui(ui, |ui| {
            for &choice in choices {
                ui.selectable_value(value, choice, name(choice));
            }
        });
}
impl App {
    pub(super) fn game_launch_editor(&mut self, ui: &mut egui::Ui) {
        use crate::launch_options::*;
        let Some(manifest) = self
            .installed
            .iter()
            .find(|m| {
                Some(&m.id) == self.selected_game.as_ref()
                    && m.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE)
            })
            .cloned()
        else {
            return;
        };
        let result = (|| -> anyhow::Result<_> {
            let root = std::fs::canonicalize(crate::registry::default_root()?)?;
            let scope = identity(&manifest)?;
            let saved = load(&root, &manifest)?;
            Ok((root, scope, saved))
        })();
        let (root, scope, saved) = match result {
            Ok(value) => value,
            Err(error) => {
                ui.label(format!("Unable to load launch options: {error:#}"));
                return;
            }
        };
        let draft_id = egui::Id::new(("launch-options", &scope, &manifest.version));
        let mut draft = ui
            .ctx()
            .data_mut(|data| data.get_temp::<LaunchOptions>(draft_id))
            .unwrap_or(saved.or(manifest.launch_options).unwrap_or_default());
        let mut selection = None;
        egui::CollapsingHeader::new("Launch options").default_open(true).show(ui, |ui| {
            ui.add_enabled_ui(self.result.is_none(), |ui| {
                launch_choice(ui, "Window", &mut draft.window_mode, WindowMode::ALL, WindowMode::label);
                launch_choice(ui, "Aspect ratio", &mut draft.aspect_ratio, AspectRatio::ALL, AspectRatio::label);
                egui::ComboBox::from_label("Internal resolution").selected_text(format!("{}×", draft.render_scale)).show_ui(ui, |ui| {
                    for value in 1..=4 { ui.selectable_value(&mut draft.render_scale, value, format!("{value}×")); }
                });
                launch_choice(ui, "Antialiasing", &mut draft.antialiasing, Antialiasing::ALL, Antialiasing::label);
                launch_choice(ui, "Texture sharpness / LOD", &mut draft.texture_lod, TextureLod::ALL, TextureLod::label);
                ui.add(egui::Slider::new(&mut draft.sharpening_percent, 0..=100).text("Sharpening %"));
                ui.checkbox(&mut draft.widescreen_hud, "Widescreen HUD correction");
                egui::ComboBox::from_label("Menu and HUD upscaling").selected_text(format!("{}×", draft.menu_hud_scale)).show_ui(ui, |ui| {
                    for value in [1, 2, 4] { ui.selectable_value(&mut draft.menu_hud_scale, value, if value == 1 { "Off".into() } else { format!("{value}×") }); }
                });
                ui.horizontal(|ui| {
                    if ui.button("Save launch options").clicked() { selection = Some(Some(draft)); }
                    if ui.button("Use package defaults").clicked() { selection = Some(None); }
                });
            });
            ui.weak("Applies on the next launch using the game’s built-in renderer. These launch options take precedence over the game’s saved display settings.");
        });
        ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
        if let Some(selection) = selection {
            let ctx = ui.ctx().clone();
            self.begin_work(move || {
                anyhow::ensure!(
                    std::fs::canonicalize(crate::registry::default_root()?)? == root,
                    "The library changed. Review launch options again."
                );
                let _change = crate::registry::LibraryChange::acquire(&root)?;
                let current = crate::registry::installed_manifest(&root, &manifest.id)?;
                anyhow::ensure!(
                    identity(&current)? == scope
                        && current.version == manifest.version
                        && current.launch_options == manifest.launch_options,
                    "The game changed. Review launch options again."
                );
                crate::launch_options::remember(&root, &current, selection)?;
                ctx.data_mut(|data| data.remove::<LaunchOptions>(draft_id));
                Ok(Work::Message(
                    "Launch options saved. They apply next time you play.".into(),
                ))
            });
        }
    }
}
