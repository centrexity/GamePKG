use super::{App, Work};
use eframe::egui;
use std::path::PathBuf;

impl App {
    pub(super) fn open_add_page(&mut self) {
        self.close_game_page();
        self.show_settings = false;
        self.show_support = false;
        self.show_add = true;
        self.preview = None;
    }

    pub(super) fn show_add_page(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(20))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().id_salt("add-page").show(ui, |ui| {
                    if ui.button("Back to library").clicked() {
                        self.show_add = false;
                        return;
                    }
                    ui.add_space(12.0);
                    ui.heading("Add games");
                    ui.label("Local package, game list, or store bundle path; HTTPS, GitHub project, or Tor URL");
                    ui.label("Or drop a .gamepkg or .gamepkglist file anywhere in this window (one at a time).");
                    ui.add(egui::TextEdit::singleline(&mut self.source).desired_width(f32::INFINITY).min_size(egui::vec2(0.0,40.0)).margin(egui::Margin::symmetric(12,10)).hint_text("Paste a link or enter a file path…"));
                    ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(self.result.is_none(), egui::Button::new("Inspect .gamepkgstore bundle")).clicked() {
                        self.pending_store = None;
                        self.pending = None;
                        self.manifest = None;
                        self.preview = None;
                        let source = self.source.trim().to_owned();
                        self.begin_work(move || {
                            let downloaded;
                            let path = if source.contains("://") {
                                downloaded = crate::registry::download_store(&source)?;
                                downloaded.path()
                            } else { std::path::Path::new(&source) };
                            Ok(Work::Store(crate::store::Bundle::open(path)?))
                        });
                    }
                    if ui.add_enabled(self.result.is_none(),egui::Button::new("Import .gamepkglist (no game downloads)")).clicked() {
                        let source = self.source.trim().to_owned();
                        self.begin_work(move || {
                            let downloaded;
                            let path = if source.contains("://") { downloaded = crate::registry::download_catalog(&source)?; downloaded.path() } else { std::path::Path::new(&source) };
                            let count = crate::catalog::import(&crate::registry::default_root()?, path)?;
                            Ok(Work::Message(format!("Added {count} games to the catalog. No game packages downloaded.")))
                        });
                    }
                    });
                    if ui.add_enabled(self.result.is_none(), egui::Button::new("Download / inspect")).clicked() {
                        let source = self.source.trim().to_owned();
                        let local = (!source.contains("://")).then(|| PathBuf::from(&source));
                        self.inspect_package(source, local);
                    }
                    ui.label(&self.status);
                    if let Some(bundle) = &self.pending_store {
                        ui.separator();
                        ui.heading("Verified store bundle");
                        ui.label("Signatures prove creator-key ownership. Confirm these creators before approving first installations. Existing games retain their creator keys.");
                        ui.vertical(|ui| {
                            for game in bundle.games() {
                                ui.label(format!("{} — {}{}", game.id, game.version, if game.requires_rom { " • User ROM required" } else { "" }));
                                ui.label(format!("Creator key: {}", game.creator_key));
                            }
                        });
                        let mut install = false;
                        let mut cancel = false;
                        ui.horizontal_wrapped(|ui| {
                            install = ui.add_enabled(self.result.is_none(), egui::Button::new("Trust listed creators and install store")).clicked();
                            cancel = ui.add_enabled(self.result.is_none(), egui::Button::new("Cancel store import")).clicked();
                        });
                        if install {
                            if let Some(bundle) = self.pending_store.take() {
                                let root = crate::registry::default_root();
                                self.begin_work(move || {
                                    let count = bundle.install(&root?, true)?;
                                    Ok(Work::Message(format!("Installed {count} store games. Build a game to play.")))
                                });
                            }
                        } else if cancel {
                            self.pending_store = None;
                            self.status = "Store import cancelled. No games installed.".into();
                        }
                    }
                });
            });
    }
}
