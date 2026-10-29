use super::{App, Work};
use eframe::egui;
#[derive(Clone, Default)]
pub(super) struct Draft {
    pub value: crate::server_selections::Selection,
    pub saved: crate::server_selections::Selection,
}
impl Draft {
    pub fn loaded(&mut self, current: crate::server_selections::Selection) {
        if self.value == self.saved {
            self.value = current.clone();
        }
        self.saved = current;
    }
}
impl From<crate::server_selections::Selection> for Draft {
    fn from(value: crate::server_selections::Selection) -> Self {
        Self {
            saved: value.clone(),
            value,
        }
    }
}
pub(super) fn scope(manifest: &crate::manifest::Manifest) -> Option<String> {
    if !manifest.permissions.network {
        return None;
    }
    crate::server_selections::identity(manifest).ok()
}
impl App {
    pub(super) fn game_network_editor(&mut self, ui: &mut egui::Ui) {
        let Some(manifest) = self
            .installed
            .iter()
            .find(|m| Some(&m.id) == self.selected_game.as_ref() && m.uses_selected_servers())
            .cloned()
        else {
            return;
        };
        if !manifest.permissions.network {
            ui.label("Networking is disabled by this package.");
            ui.add_space(12.0);
            return;
        }
        let Some(scope) = scope(&manifest) else {
            return;
        };
        let mut chosen = None;
        egui::CollapsingHeader::new("Multiplayer").default_open(true).show(ui,|ui| {
            let draft=&mut self.network_drafts.entry(scope.clone()).or_default().value;
            ui.label("Choose a server, hosting address or master to use on launch. Networking is enabled by this package.");
            ui.add_enabled_ui(self.result.is_none(),|ui| {
                ui.label("Join server (hostname:port)");
                ui.add(egui::TextEdit::singleline(&mut draft.server).id_salt("multiplayer-server").hint_text("server.example:28785").char_limit(256).desired_width(480.0));
                ui.label("Or host on this computer (local IPv4 address:port)");
                ui.add(egui::TextEdit::singleline(&mut draft.listen).id_salt("multiplayer-listen").hint_text("192.168.1.20:28785").char_limit(256).desired_width(480.0));
                ui.label("Server browser master (hostname:port, optional)");
                ui.add(egui::TextEdit::singleline(&mut draft.master).id_salt("multiplayer-master").hint_text("master.example:28787").char_limit(256).desired_width(480.0));
                ui.weak("Allows browsing and joining IPv4 servers listed by this master. Its list is read by the host; the game receives only validated server addresses.");

                ui.label("These addresses are launch preferences. Networking can reach other addresses too. Hosting stays off public master-server lists.");
                ui.horizontal(|ui| {
                    if ui.button("Save multiplayer settings").clicked() {chosen=Some(draft.clone());}
                    if ui.button("Clear launch addresses").clicked() {*draft=Default::default();chosen=Some(draft.clone());}
                });
            });
            ui.weak("Choices apply the next time you press Play. Save remembers them without launching. Running games keep their current connection.");
        });
        ui.add_space(12.0);
        if let Some(selection) = chosen {
            let root =
                match crate::registry::default_root().and_then(|p| Ok(std::fs::canonicalize(p)?)) {
                    Ok(root) => root,
                    Err(error) => {
                        self.status = format!("Unable to save multiplayer settings: {error:#}");
                        return;
                    }
                };
            self.begin_work(move || {
                anyhow::ensure!(
                    std::fs::canonicalize(crate::registry::default_root()?)? == root,
                    "The library changed. Review multiplayer settings again."
                );
                let _change = crate::registry::LibraryChange::acquire(&root)?;
                let current = crate::registry::installed_manifest(&root, &manifest.id)?;
                anyhow::ensure!(
                    crate::server_selections::identity(&current)? == scope,
                    "The game changed. Review multiplayer settings again."
                );
                let saved = crate::server_selections::remember(&root, &current, &selection)?;
                Ok(Work::NetworkSaved(scope, saved))
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_packages_render_disabled_networking_without_creating_launch_drafts() {
        let mut manifest =
            crate::manifest::Manifest::parse(include_bytes!("../../ports/cube2/manifest.toml"))
                .unwrap();
        manifest.permissions.network = false;
        let mut app = App {
            selected_game: Some(manifest.id.clone()),
            installed: vec![manifest],
            ..Default::default()
        };
        let context = egui::Context::default();
        let output = context.run(Default::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| app.game_network_editor(ui));
        });
        assert!(
            app.network_drafts.is_empty(),
            "disabled game created an editable launch draft"
        );
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::epaint::Shape::Text(text) if text.galley.text().contains("Networking is disabled by this package"))));
    }
    #[test]
    fn refreshes_follow_saved_choices_without_overwriting_unsaved_edits() {
        let original = crate::server_selections::Selection {
            server: "one.example:28785".into(),
            ..Default::default()
        };
        let external = crate::server_selections::Selection {
            server: "two.example:28785".into(),
            ..Default::default()
        };
        let mut draft = Draft::from(original.clone());
        draft.loaded(external.clone());
        assert_eq!(draft.value, external);
        draft.value.server = "unsaved.example:28785".into();
        draft.loaded(original.clone());
        assert_eq!(draft.value.server, "unsaved.example:28785");
        assert_eq!(draft.saved, original);
    }
}
