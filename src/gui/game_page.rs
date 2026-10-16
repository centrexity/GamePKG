use super::{App, Work};
use eframe::egui;
use std::path::PathBuf;
impl App {
    pub(super) fn close_game_page(&mut self) {
        self.selected_game = None;
        self.preview = None;
        if self.pending.is_some() {
            self.pending = None;
            self.manifest = None;
            self.status = "Package review cancelled. Nothing installed.".into();
        }
    }
    pub(super) fn open_game_page(&mut self, id: &str) {
        if self.result.is_some() {
            return;
        }
        if self.pending_store.is_some()
            || (self.pending.is_some() && self.manifest.as_ref().is_none_or(|m| m.id != id))
        {
            self.drop_notice = Some("Finish or cancel the current package review first.".into());
            return;
        }
        self.selected_game = Some(id.to_owned());
        self.show_settings = false;
        self.show_support = false;
        self.preview = None;
        if self.pending.is_none() {
            self.manifest = self.installed.iter().find(|m| m.id == id).cloned();
        }
        if self.installed.iter().any(|m| m.id == id && m.requires_rom) {
            self.rom_game = Some(id.to_owned());
            if let Ok(root) = crate::registry::default_root() {
                self.sync_rom_selection(&root);
            }
        } else {
            self.rom_game = None;
            self.rom_identity = None;
        }
    }
    fn launch_selected(&mut self, id: String) {
        let paths = self
            .rom_paths
            .get(&id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let identity = self.installed.iter().find(|m| m.id == id).and_then(|m| {
            crate::registry::default_root()
                .ok()
                .and_then(|root| super::rom_identity(&root, m))
        });
        self.begin_work(move || {
            let root = crate::registry::default_root()?;
            let _change = if paths.is_empty() {
                None
            } else {
                Some(crate::registry::LibraryChange::acquire(&root)?)
            };
            if !paths.is_empty() {
                let manifest = crate::registry::installed_manifest(&root, &id)?;
                anyhow::ensure!(
                    identity.is_some() && super::rom_identity(&root, &manifest) == identity,
                    "The game or library changed. Select the ROM files again."
                );
            }
            Ok(Work::Game(Box::new(crate::runtime::Game::load_with_roms(
                &root, &id, &paths,
            )?)))
        });
    }
    pub(super) fn show_game_page(&mut self, ctx: &egui::Context) {
        let Some(id) = self.selected_game.clone() else {
            return;
        };
        if self.pending.is_none() {
            self.manifest = self.installed.iter().find(|m| m.id == id).cloned();
        }
        let installed = self.installed.iter().find(|m| m.id == id).cloned();
        if installed.as_ref().is_some_and(|m| m.requires_rom) {
            self.rom_game = Some(id.clone());
            if let Ok(root) = crate::registry::default_root() {
                self.sync_rom_selection(&root);
            }
        }
        egui::CentralPanel::default().show(ctx,|ui|{
            if ui.button("Back to library").clicked(){self.close_game_page();return;}
            ui.add_space(16.0);
            ui.horizontal(|ui|{
                if let Some(icon)=self.icons.get(&id){ui.add(egui::Image::new(icon).fit_to_exact_size(egui::vec2(72.0,72.0)));}
                ui.vertical(|ui|{ui.heading(&id);if let Some(m)=self.manifest.as_ref(){ui.label(format!("Version {}",m.version));}
                    if let Some(record)=self.history.get(&id){ui.label(format!("Played: {}",crate::progress::duration(record.played_ms)));}
                });
            });
            ui.add_space(16.0);
            if let Some(m)=installed {
                let built=self.built.contains(&id);let reviewing=self.pending.is_some();
                ui.label(if built{"Ready to play"}else{"Not built"});
                if let Some(usage)=self.usage.get(&id){ui.label(format!("Storage: {} used",crate::storage::bytes(usage.used_bytes())));}
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(!reviewing&&built&&self.result.is_none()&&(!m.requires_rom||self.rom_paths.get(&id).is_some_and(|p|crate::rom_selections::ready(p,m.required_rom_count()))),egui::Button::new("Play").min_size(egui::vec2(100.0,36.0))).clicked(){self.launch_selected(id.clone());}
                    if ui.add_enabled(!reviewing&&self.result.is_none(),egui::Button::new(if built{"Rebuild"}else{"Build"}).min_size(egui::vec2(100.0,36.0))).clicked(){let id=id.clone();self.begin_work(move||{crate::build::compile(&crate::registry::default_root()?,&id)?;Ok(Work::Message("Built. Ready to play.".into()))});}
                    if ui.add_enabled(!reviewing&&self.result.is_none()&&m.update_url.is_some(),egui::Button::new("Check for updates")).clicked(){let id=id.clone();self.begin_work(move||match crate::registry::check_update(&crate::registry::default_root()?,&id)?{Some((m,file))=>Ok(Work::Update(m,file)),None=>Ok(Work::Message("This game is up to date.".into()))});}
                    ui.menu_button("More",|ui|{
                        if ui.add_enabled(!reviewing&&self.result.is_none(),egui::Button::new("Check integrity")).clicked(){let id=id.clone();self.begin_work(move||Ok(Work::Message(crate::integrity::check(&crate::registry::default_root()?,&id)?)));ui.close_menu();}
                        if ui.add_enabled(!reviewing&&self.result.is_none(),egui::Button::new("Fix / rebuild")).clicked(){let id=id.clone();self.begin_work(move||Ok(Work::Message(crate::integrity::fix_with_store(&crate::registry::default_root()?,&id,None)?)));ui.close_menu();}
                        if !m.shared_assets.is_empty(){ui.label("Original .gamepkgstore path");ui.text_edit_singleline(&mut self.source);}
                        if !m.shared_assets.is_empty()&&ui.add_enabled(!reviewing&&self.result.is_none(),egui::Button::new("Repair assets from store")).clicked(){let id=id.clone();let source=self.source.clone();self.begin_work(move||Ok(Work::Message(crate::integrity::fix_with_store(&crate::registry::default_root()?,&id,Some(std::path::Path::new(source.trim())))?)));ui.close_menu();}
                        if ui.add_enabled(!reviewing&&built&&self.result.is_none(),egui::Button::new("Export for web")).clicked(){let id=id.clone();self.begin_work(move||{let root=crate::registry::default_root()?;let suffix=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();let output=root.join("web-exports").join(format!("{id}-{suffix}"));crate::web_export::export(&root,&id,&output)?;Ok(Work::Message(format!("Web player exported to {}",output.display())))});ui.close_menu();}
                    });
                });
                if m.requires_rom&&!self.rom_paths.get(&id).is_some_and(|p|crate::rom_selections::ready(p,m.required_rom_count())){ui.label("Choose the required ROM files below to enable Play.");}
            }else if self.pending.is_none(){
                if let Some(entry)=self.catalog.iter().find(|e|e.id==id).cloned(){
                    ui.label(&entry.title);ui.label("Not downloaded. Catalog information is unverified until the signed package is checked.");
                    if entry.requires_rom==Some(true){ui.label("Requires your own ROM or game data files.");}
                    if ui.add_enabled(self.result.is_none(),egui::Button::new("Download / inspect").min_size(egui::vec2(160.0,36.0))).clicked(){let id=id.clone();self.begin_work(move||{let (m,file)=crate::catalog::fetch(&crate::registry::default_root()?,&id)?;Ok(Work::Update(m,file))});}
                }else{ui.label("This game is no longer in this library.");}
            }
            if let Some(notice)=self.updates.notices.get(&id){match notice {crate::updates::Notice::Available(version)=>{ui.label(format!("Update {version} available"));},crate::updates::Notice::Current=>{ui.label("Up to date");},crate::updates::Notice::Failed(error)=>{ui.label("Update check failed").on_hover_text(error);}}}
            ui.add_space(12.0);ui.label(&self.status);
            egui::ScrollArea::vertical().id_salt("game-page-content").show(ui,|ui|{
                if self.pending.is_none(){self.game_rom_editor(ui);}
                self.game_details(ui,ctx);
            });
        });
    }

    fn game_rom_editor(&mut self, ui: &mut egui::Ui) {
        if let Some(selected) = self
            .installed
            .iter()
            .find(|m| Some(&m.id) == self.selected_game.as_ref() && m.requires_rom)
            .cloned()
        {
            egui::Frame::group(ui.style()).show(ui,|ui| {
                    ui.strong(format!("ROM files — {}",selected.id));
                    let mut changed=false;
                    for index in 0..selected.required_rom_count() {
                        let title=selected.roms.get(index).map(|r|r.title.clone()).unwrap_or_else(||format!("ROM {}",index+1));
                        let path=if index==0 {&mut self.rom_path}else{&mut self.additional_rom_paths[index-1]};
                        ui.horizontal(|ui|{ui.label(title);changed|=ui.add(egui::TextEdit::singleline(path).hint_text("File location").desired_width(420.0)).changed();});
                    }
                    ui.label("Locations are remembered for this game. Files stay on your drive and are shared read-only.");
                    if changed {
                        let paths=std::iter::once(self.rom_path.clone()).chain(self.additional_rom_paths.iter().cloned()).collect::<Vec<_>>();
                        if let Ok(root)=crate::registry::default_root() {
                            match crate::rom_selections::save(&root,&selected,&paths) {Ok(())=>{self.rom_paths.insert(selected.id.clone(),paths);},Err(e)=>self.status=format!("Unable to remember ROM files: {e:#}")}
                        }
                    }

                });
            ui.add_space(12.0);
        }
    }
    fn game_details(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(m) = self.manifest.clone() {
            egui::ScrollArea::vertical().id_salt("game-details").max_height((ui.available_height() - 35.0).max(100.0)).show(ui, |ui| {
                ui.separator(); ui.heading("About this game"); ui.label(format!("Version: {}", m.version));
                ui.label(format!("Memory: {:.1} MiB recommended • {:.1} MiB hard limit", m.limits.recommended_memory_bytes() as f64 / 1048576.0, m.limits.hard_memory_bytes as f64 / 1048576.0));
                ui.label(if m.requires_rom { format!("Requires {} ROM file(s) supplied by you. They are not included in this package.",m.required_rom_count()) } else { "No user ROM required.".into() });
                for (index,rom) in m.roms.iter().enumerate() {ui.label(format!("ROM {}: {} ({})",index+1,rom.title,rom.id));}
                let package_size = self.pending.as_ref().and_then(|file| file.as_file().metadata().ok()).map(|m|m.len()).or_else(||self.usage.get(&m.id).map(|u|u.package_bytes));
                ui.label(format!("Storage: {} package; {} expected after building", package_size.map(crate::storage::bytes).unwrap_or_else(||"Unknown".into()), m.limits.recommended_storage_bytes.map(|v|v.max(package_size.unwrap_or(0))).map(crate::storage::bytes).unwrap_or_else(||"Unknown (no creator estimate)".into())));
                ui.label(format!("Creator fingerprint: {}", crate::verify::digest(&hex::decode(&m.creator_key).unwrap_or_default())));
                ui.label("A valid signature proves key ownership, not a real-world identity. Confirm the creator before first installation.");
                if !m.achievements.is_empty() {
                    let same_creator=self.installed.iter().any(|old|old.id==m.id && old.creator_key==m.creator_key);
                    let record=if same_creator { self.history.get(&m.id) } else { None };
                    egui::CollapsingHeader::new("Achievements").default_open(true).show(ui, |ui| {
                        ui.label("Local progress reported by this game.");
                        egui::ScrollArea::vertical().id_salt("achievement-details").max_height(160.0).show(ui, |ui| {
                            for achievement in &m.achievements {
                                let status=match record { Some(record) if record.unlocked.contains_key(&achievement.id) => "Unlocked", Some(_) => "Locked", None if same_creator => "Unavailable", None => "Locked" };
                                ui.strong(format!("{status} — {}", achievement.title));
                                ui.label(&achievement.description);
                            }
                        });
                    });
                }
                if !m.shared_assets.is_empty() { ui.label(format!("Shared store assets: {} (must be available locally to install and play)", m.shared_assets.len())); }
                ui.label(format!("Runtime: {} • Entry world: {}",m.runtime_profile,m.entry_world));
                ui.label(format!("Engine requirement: {} • Save schema: {}",m.engine_version,m.save_schema));
                if self.pending.is_some() && self.installed.iter().any(|old| old.id == m.id && old.save_schema != m.save_schema) {
                    ui.label("This update uses a different save format. A separate save starts; your earlier save is kept.");
                }
                ui.label(format!("Lockfile / dependency inventory SHA-256: {}",m.lockfile_sha256));
                ui.label(format!("Permissions: input={}, audio={}, save={}, network={}", m.permissions.input, m.permissions.audio, m.permissions.save, m.permissions.network));
                if let Some(url) = &m.update_url { ui.label(format!("Updates: {url}")); }
                let preview_id = m.id.clone();
                ui.horizontal(|ui| {
                    for (label, prefer_theora) in [("Play preview", false), ("Play Theora fallback", true)] {
                        if ui.add_enabled(self.result.is_none(), egui::Button::new(label)).clicked() {
                            self.preview = None;
                            let snapshot = self.pending.as_ref().and_then(|file| file.reopen().ok());
                            let id = preview_id.clone();
                            self.begin_work(move || {
                                let mut copy = tempfile::NamedTempFile::new()?;
                                let mut input = match snapshot { Some(file) => file, None => std::fs::File::open(crate::registry::default_root()?.join(format!("{id}.gamepkg")))? };
                                std::io::copy(&mut std::io::Read::take(&mut input, crate::archive::MAX_TOTAL + 1), &mut copy)?;
                                let mut pkg = crate::archive::Package::open(copy.path())?;
                                crate::verify::verify_embedded(&mut pkg)?;
                                let name = if !prefer_theora && pkg.entries.contains_key("preview.av1.webm") { "preview.av1.webm" } else { "preview.ogv" };
                                Ok(Work::Preview(crate::previews::decode(name, &pkg.read(name)?)?))
                            });
                        }
                    }
                });
                let mut stop_preview = false;
                if let Some((frames, start, texture)) = &mut self.preview {
                    let index = ((start.elapsed().as_millis() / 200) as usize).min(frames.len() - 1);
                    texture.set(egui::ColorImage::from_rgb([320, 240], &frames[index]), egui::TextureOptions::LINEAR);
                    ui.image((texture.id(), egui::vec2(320.0, 240.0)));
                    ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    stop_preview = ui.button("Close preview").clicked();
                }
                if stop_preview { self.preview = None; }
                let already = self.installed.iter().any(|old| old.id == m.id);
                let label = if already { "Install verified update" } else { "Trust this creator and install" };
                if self.pending.is_some() && ui.add_enabled(self.result.is_none(), egui::Button::new(label)).clicked() {
                    if let Some(stage) = self.pending.take() {
                        self.begin(move || crate::registry::install(&crate::registry::default_root()?, stage.path(), true));
                    }
                }
                });
            if self.pending.is_some()
                && self.result.is_none()
                && ui.button("Cancel package review").clicked()
            {
                self.close_game_page();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opening_catalog_and_installed_pages_never_downloads_installs_or_plays() {
        let mut app = App::default();
        app.catalog.push(crate::catalog::Entry {
            id: "catalog.game".into(),
            title: "Catalog game".into(),
            download_url: "https://example.invalid/game.gamepkg".into(),
            version: Some("1.0.0".into()),
            requires_rom: Some(false),
        });
        app.open_game_page("catalog.game");
        assert_eq!(app.selected_game.as_deref(), Some("catalog.game"));
        assert!(app.manifest.is_none());
        assert!(app.result.is_none());
        assert!(app.player.is_none());
        let m: crate::manifest::Manifest =
            toml::from_str(include_str!("../../examples/minimal/manifest.toml")).unwrap();
        app.installed.push(m.clone());
        app.open_game_page(&m.id);
        assert_eq!(app.manifest.as_ref().unwrap().id, m.id);
        assert!(app.pending.is_none());
        assert!(app.result.is_none());
        app.close_game_page();
        assert!(app.selected_game.is_none());
    }
    #[test]
    fn review_stays_bound_to_its_package_and_back_cancels_without_installing() {
        let mut app = App::default();
        let m: crate::manifest::Manifest =
            toml::from_str(include_str!("../../examples/minimal/manifest.toml")).unwrap();
        app.manifest = Some(m.clone());
        app.pending = Some(tempfile::NamedTempFile::new().unwrap());
        let path = app.pending.as_ref().unwrap().path().to_path_buf();
        app.open_game_page(&m.id);
        assert_eq!(app.selected_game.as_deref(), Some(m.id.as_str()));
        app.open_game_page("other.game");
        assert_eq!(app.selected_game.as_deref(), Some(m.id.as_str()));
        assert_eq!(app.pending.as_ref().unwrap().path(), path);
        app.close_game_page();
        assert!(app.pending.is_none());
        assert!(app.manifest.is_none());
        assert!(app.installed.is_empty());
        assert!(!path.exists());
    }
}
