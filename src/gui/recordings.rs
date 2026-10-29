//! Host-owned recording browser; game code never selects export destinations.
use super::{egui, App, Work};
use std::path::PathBuf;
pub(super) struct Loaded {
    root: PathBuf,
    manifest: crate::manifest::Manifest,
    entries: Vec<crate::cube2::recordings::Recording>,
}
fn check_scope(root: &std::path::Path, selected: &crate::manifest::Manifest) -> anyhow::Result<()> {
    let current = crate::registry::installed_manifest(root, &selected.id)?;
    anyhow::ensure!(
        current.creator_key == selected.creator_key
            && current.save_schema == selected.save_schema
            && current.builtin_engine.as_deref() == Some(crate::cube2::PROFILE),
        "The game changed. Refresh the library before exporting recordings."
    );
    Ok(())
}
impl App {
    pub(super) fn game_recordings(&mut self, ui: &mut egui::Ui) {
        let Some(manifest) = self
            .installed
            .iter()
            .find(|m| {
                Some(&m.id) == self.selected_game.as_ref()
                    && m.builtin_engine.as_deref() == Some(crate::cube2::PROFILE)
            })
            .cloned()
        else {
            return;
        };
        ui.heading("Recordings");
        ui.label("Saved demos and movies. New recordings appear after the game saves or closes.");
        let loaded = self.recordings.as_ref().filter(|loaded| {
            loaded.manifest.id == manifest.id
                && loaded.manifest.creator_key == manifest.creator_key
                && loaded.manifest.save_schema == manifest.save_schema
        });
        let ready = loaded.is_some();
        let mut export = None;
        if let Some(loaded) = loaded {
            if loaded.entries.is_empty() {
                ui.weak("No saved recordings yet.");
            }
            for entry in &loaded.entries {
                ui.horizontal_wrapped(|ui| {
                    ui.label(&entry.name);
                    ui.weak(crate::storage::bytes(entry.bytes as u64));
                    if ui
                        .add_enabled(
                            self.result.is_none(),
                            egui::Button::new(if entry.kind == "movie" {
                                "Export movie"
                            } else {
                                "Export demo"
                            }),
                        )
                        .clicked()
                    {
                        export =
                            Some((loaded.root.clone(), loaded.manifest.clone(), entry.clone()));
                    }
                });
                ui.add_space(6.0);
            }
        }
        if ui
            .add_enabled(
                cfg!(target_os = "linux") && self.result.is_none(),
                egui::Button::new(if ready {
                    "Refresh recordings"
                } else {
                    "Load recordings"
                }),
            )
            .clicked()
        {
            let manifest = manifest.clone();
            self.begin_work(move || {
                let root = std::fs::canonicalize(crate::registry::default_root()?)?;
                let _change = crate::registry::LibraryChange::acquire(&root)?;
                check_scope(&root, &manifest)?;
                let entries = crate::cube2::recordings::list(&root, &manifest.id)?;
                Ok(Work::Recordings(Loaded {
                    root,
                    manifest,
                    entries,
                }))
            });
        }
        if let Some((root, manifest, entry)) = export {
            self.begin_work(move || {
                let filename = std::path::Path::new(&entry.name)
                    .file_name()
                    .unwrap()
                    .to_string_lossy();
                let extension = if entry.kind == "movie" { "avi" } else { "dmo" };
                let Some(output) = rfd::FileDialog::new()
                    .set_title("Save recording")
                    .set_file_name(filename.as_ref())
                    .add_filter("Recording", &[extension])
                    .save_file()
                else {
                    return Ok(Work::RecordingExported(
                        "Recording export cancelled.".into(),
                    ));
                };
                anyhow::ensure!(
                    std::fs::canonicalize(crate::registry::default_root()?)? == root,
                    "The library changed. Refresh recordings before exporting."
                );
                let _change = crate::registry::LibraryChange::acquire(&root)?;
                check_scope(&root, &manifest)?;
                crate::cube2::recordings::export(&root, &manifest.id, &entry.name, &output)?;
                Ok(Work::RecordingExported(format!(
                    "Recording saved to {}",
                    output.display()
                )))
            });
        }
        let import_ready = cfg!(target_os = "linux")
            && self.result.is_none()
            && !self.game_processes.iter().any(|(id, _)| id == &manifest.id);
        if ui
            .add_enabled(import_ready, egui::Button::new("Import recording"))
            .clicked()
        {
            self.begin_work(move || {
                let root = std::fs::canonicalize(crate::registry::default_root()?)?;
                let Some(source) = rfd::FileDialog::new()
                    .set_title("Import recording")
                    .add_filter("Recordings", &["dmo", "avi"])
                    .pick_file()
                else {
                    return Ok(Work::RecordingExported(
                        "Recording import cancelled.".into(),
                    ));
                };
                anyhow::ensure!(
                    std::fs::canonicalize(crate::registry::default_root()?)? == root,
                    "The library changed. Select the game again before importing."
                );
                crate::cube2::recordings::import_selected(&root, &manifest, &source)?;
                let _change = crate::registry::LibraryChange::acquire(&root)?;
                check_scope(&root, &manifest)?;
                let entries = crate::cube2::recordings::list(&root, &manifest.id)?;
                Ok(Work::Recordings(Loaded {
                    root,
                    manifest,
                    entries,
                }))
            });
        }
        ui.weak("Close the game before importing. Existing recordings are kept.");
        ui.weak("Choose a new filename when exporting. Existing files are kept.");
        ui.add_space(12.0);
    }
    pub(super) fn loaded_recordings(&mut self, loaded: Loaded) {
        self.status = format!("{} saved recordings.", loaded.entries.len());
        self.recordings = Some(loaded);
    }
}
