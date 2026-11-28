use super::{App, Work};
use eframe::egui;
use std::path::PathBuf;
impl App {
    pub(super) fn play_blockers(&self, m: &crate::manifest::Manifest) -> Vec<String> {
        let mut blockers = Vec::new();
        if let Err(reason) = crate::native::require_compatibility(m) {
            blockers.push(reason.to_string());
        } else if !self.built.contains(&m.id) {
            blockers.push("Build this game first. If you already built it, recheck readiness or rebuild if the cached build is missing or invalid.".into());
        }
        if m.uses_service_runtime() {
            let required = m.limits.hard_memory_bytes.div_ceil(1048576);
            if self
                .applied_settings
                .as_ref()
                .is_some_and(|settings| settings.memory_ceiling_mib < required)
            {
                blockers.push(format!(
                    "Increase the memory ceiling to at least {required} MiB in Settings."
                ));
            }
        }
        if let Some(data) = &m.game_data {
            if !m.uses_service_runtime()
                && self
                    .applied_settings
                    .as_ref()
                    .is_none_or(|s| s.memory_ceiling_mib < 2048)
            {
                let current = self
                    .applied_settings
                    .as_ref()
                    .map(|s| s.memory_ceiling_mib)
                    .unwrap_or(512);
                blockers.push(format!("Increase the memory ceiling to at least 2048 MiB in Settings (currently {current} MiB)."));
            }
            match self.data_paths.get(&m.id).filter(|p| !p.is_empty()) {
                None => blockers.push(if m.uses_service_runtime() || data.layouts.is_empty() {
                    "Choose the original game-data folder below.".into()
                } else {
                    "Choose the original game-data file or folder below.".into()
                }),
                Some(path) => {
                    if let Err(error) =
                        crate::game_data::validate_for(m, std::path::Path::new(path))
                    {
                        blockers.push(format!("Selected game data: {error}. Choose a complete original installation below."));
                    }
                }
            }
        }
        if m.requires_rom {
            for index in 0..m.required_rom_count() {
                let path = self
                    .rom_paths
                    .get(&m.id)
                    .and_then(|paths| paths.get(index))
                    .map(String::as_str)
                    .unwrap_or("");
                if let Err(error) = crate::rom_selections::check_for(m, path) {
                    let title = m
                        .roms
                        .get(index)
                        .map(|rom| rom.title.clone())
                        .unwrap_or_else(|| format!("ROM {}", index + 1));
                    if path.trim().is_empty() {
                        blockers.push(format!("Choose an available file for {title} below."));
                    } else {
                        blockers.push(format!(
                            "File for {title}: {error}. Choose another file below."
                        ));
                    }
                }
            }
        }
        blockers
    }
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
        self.show_add = false;
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
    fn launch_selected(&mut self, id: String, native: bool) {
        let network = self
            .installed
            .iter()
            .find(|m| m.id == id)
            .and_then(super::multiplayer::scope)
            .and_then(|scope| {
                self.network_drafts
                    .get(&scope)
                    .map(|draft| (scope, draft.value.clone()))
            });
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
            let _change = if paths.is_empty() && network.is_none() {
                None
            } else {
                Some(crate::registry::LibraryChange::acquire(&root)?)
            };
            // A plain launch needs no parent-side source scan: the child
            // performs the authoritative strict verification. Only saved
            // ROM/network state needs this authenticated manifest here.
            let manifest = (!paths.is_empty() || network.is_some())
                .then(|| crate::registry::installed_signed_identity(&root, &id))
                .transpose()?;
            if let Some(manifest) = &manifest {
                if !paths.is_empty() {
                    anyhow::ensure!(
                        identity.is_some() && super::rom_identity(&root, manifest) == identity,
                        "The game or library changed. Select the ROM files again."
                    );
                }
            }
            let saved_network = if let Some((scope, selection)) = network {
                let manifest = manifest.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("The game changed. Review multiplayer settings again.")
                })?;
                anyhow::ensure!(
                    crate::server_selections::identity(manifest)? == scope,
                    "The game changed. Review multiplayer settings again."
                );
                let saved = crate::server_selections::remember(&root, manifest, &selection)?;
                Some((scope, saved))
            } else {
                None
            };
            if let Some(manifest) = &manifest {
                if manifest.requires_rom {
                    crate::rom_selections::save(
                        &root,
                        manifest,
                        &paths
                            .iter()
                            .map(|p| p.to_string_lossy().into_owned())
                            .collect::<Vec<_>>(),
                    )?;
                }
            }
            let mut child = std::process::Command::new(std::env::current_exe()?);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                child.creation_flags(0x08000000);
            }
            child
                .arg("--library")
                .arg(&root)
                .arg("--play")
                .arg(&id)
                .arg("--target")
                .arg(if native { "native" } else { "wasm" })
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            Ok(Work::Launched(id, child.spawn()?, saved_network))
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
        if installed.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        if installed.as_ref().is_some_and(|m| m.requires_rom) {
            self.rom_game = Some(id.clone());
            if let Ok(root) = crate::registry::default_root() {
                self.sync_rom_selection(&root);
            }
        }
        egui::CentralPanel::default().frame(egui::Frame::central_panel(&ctx.style()).inner_margin(20)).show(ctx,|ui|{
            egui::ScrollArea::vertical().id_salt("game-page").show(ui,|ui|{
            if ui.button("Back to library").clicked(){self.close_game_page();return;}
            ui.add_space(16.0);
            ui.horizontal(|ui|{
                if let Some(icon)=self.icons.get(&id){ui.add(egui::Image::new(icon).fit_to_exact_size(egui::vec2(72.0,72.0)));}else{super::theme::cover(ui,&id,72.0);}
                ui.vertical(|ui|{ui.heading(self.manifest.as_ref().map(|m|m.display_title()).or_else(||self.catalog.iter().find(|e|e.id==id).map(|e|e.title.as_str())).unwrap_or(&id));if let Some(m)=self.manifest.as_ref(){ui.label(format!("Version {}",m.version));}
                    if let Some(record)=self.history.get(&id){ui.label(format!("Played: {}",crate::progress::duration(record.played_ms)));}
                });
            });
            ui.add_space(16.0);
            if let Some(m)=self.manifest.as_ref() {
                if let Some(description)=&m.description {ui.label(description);ui.add_space(8.0);}
                if let Some(author)=&m.author {ui.label(format!("By {author}"));}
                if !m.genres.is_empty() {ui.label(m.genres.join("  •  "));}
                if let Some(url)=&m.homepage {ui.hyperlink_to("Project website",url);}
                if m.title.is_some() {ui.small(&id);}
                ui.add_space(12.0);
            } else if let Some(description)=self.catalog.iter().find(|e|e.id==id).and_then(|e|e.description.as_ref()) {
                ui.label(description);
                ui.weak("Catalog description — verified when the signed package is downloaded.");
                ui.add_space(12.0);
            }
            if self.pending.is_some() {
                ui.label("Review the creator and permissions before installing.");
                if let Some(m)=self.manifest.as_ref() {
                    ui.label(format!("Creator fingerprint: {}",crate::verify::digest(&hex::decode(&m.creator_key).unwrap_or_default())));
                }
                let already = self.manifest.as_ref().is_some_and(|m| self.installed.iter().any(|old|old.id==m.id));
                let label = if already { "Install verified update" } else { "Trust this creator and install" };
                if self.pending.is_some() && ui.add_enabled(self.result.is_none(), egui::Button::new(label)).clicked() {
                    if let Some(stage) = self.pending.take() {
                        self.begin(move || crate::registry::install(&crate::registry::default_root()?, stage.path(), true));
                    }
                }

                ui.add_space(12.0);
            }
            if let Some(m)=installed {
                let built=self.built.contains(&id);let reviewing=self.pending.is_some();
                let blockers = self.play_blockers(&m);
                let ready = blockers.is_empty();
                let wasm_ready = crate::build_policy::supported(m.builtin_engine.as_deref()) && self.wasm_built.contains(&id)
                    && (!m.requires_rom || self.rom_paths.get(&id).is_some_and(|paths| crate::rom_selections::ready_for(&m, paths)))
                    && (m.game_data.is_none() || (
                        self.applied_settings.as_ref().is_some_and(|settings| settings.memory_ceiling_mib >= 2048)
                        && self.data_paths.get(&id).is_some_and(|path| crate::game_data::validate(std::path::Path::new(path)).is_ok())
                    ));
                ui.label(if !crate::build_policy::supported(m.builtin_engine.as_deref()) {"Port requires migration to GamePKG services"} else if ready{"Ready to play"}else if built{"Native build verified — setup required"}else{"Native build: Not built or no longer valid"});
                if let Some(usage)=self.usage.get(&id){ui.label(format!("Storage: {}",usage.space().label())).on_hover_text(usage.space().details());}
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(!reviewing&&ready&&self.result.is_none(),super::theme::primary("Play")).clicked(){self.launch_selected(id.clone(),true);}
                    if ui.add_enabled(crate::native::build_supported(&m)&&!reviewing&&self.result.is_none(),egui::Button::new(if built{"Rebuild"}else{"Build"}).min_size(egui::vec2(100.0,36.0))).clicked(){let id=id.clone();self.begin_work(move||{crate::native::compile(&crate::registry::default_root()?,&id)?;Ok(Work::NativeBuilt(id))});}
                    if ui.add_enabled(!reviewing&&self.result.is_none()&&m.update_url.is_some(),egui::Button::new("Check for updates")).clicked(){let id=id.clone();self.begin_work(move||match crate::registry::check_update(&crate::registry::default_root()?,&id)?{Some((m,file))=>Ok(Work::Update(m,file)),None=>Ok(Work::Message("This game is up to date.".into()))});}
                    if ui.add_enabled(!m.uses_service_runtime() && crate::build_policy::supported(m.builtin_engine.as_deref()) && !reviewing&&self.result.is_none(),egui::Button::new("Build Wasm")).clicked(){let id=id.clone();self.begin_work(move||{crate::build::compile(&crate::registry::default_root()?,&id)?;Ok(Work::Message("Wasm build complete. Native build kept separately.".into()))});}
                    if ui.add_enabled(!reviewing&&wasm_ready&&self.result.is_none(),egui::Button::new("Play Wasm")).clicked(){self.launch_selected(id.clone(),false);}
                    ui.add_enabled_ui(!reviewing && self.result.is_none() && cfg!(any(target_os="linux", target_os="windows")), |ui| {
                        ui.menu_button("Add to Steam", |ui| {
                            let target = if ready { Some("native") } else if wasm_ready { Some("wasm") } else { None };
                            let Some(target) = target else {
                                ui.label("Build the game and choose its required files first.");
                                return;
                            };
                            let mut accounts = crate::steam_shortcuts::accounts();
                            if let (Ok(found), Some(root)) = (&mut accounts, &self.steam_custom_root) {
                                if let Ok(extra) = crate::steam_shortcuts::accounts_at(root) {
                                    for account in extra {
                                        if !found.iter().any(|old| old.shortcuts == account.shortcuts) { found.push(account); }
                                    }
                                }
                            }
                            match accounts {
                                Ok(found) if found.is_empty() => { ui.label("No Steam account folder found."); }
                                Ok(found) => for account in found {
                                    if ui.button(&account.label).on_hover_text(account.shortcuts.display().to_string()).clicked() {
                                        let manifest = m.clone();
                                        let target = target.to_owned();
                                        self.begin_work(move || {
                                            let root = crate::registry::default_root()?;
                                            let exe = crate::steam_shortcuts::current_launcher()?;
                                            Ok(Work::Message(crate::steam_shortcuts::add(&account, &manifest, &root, &exe, &target)?))
                                        });
                                        ui.close_menu();
                                    }
                                },
                                Err(error) => { ui.label(format!("Unable to find Steam accounts: {error:#}")); }
                            }
                            if ui.button("Choose Steam folder…").clicked() {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    self.steam_custom_root = Some(folder);
                                }
                            }
                            ui.weak("Close Steam before adding. Open it again to see the shortcut.");
                        });
                    });
                    ui.menu_button("More",|ui|{
                        if ui.add_enabled(!reviewing&&self.result.is_none(),egui::Button::new("Check integrity")).clicked(){let id=id.clone();self.begin_work(move||Ok(Work::Message(crate::integrity::check(&crate::registry::default_root()?,&id)?)));ui.close_menu();}
                    if ui.add_enabled(crate::native::build_supported(&m)&&!reviewing&&self.result.is_none(),egui::Button::new("Fix / rebuild native")).clicked(){let id=id.clone();self.begin_work(move||Ok(Work::Message(crate::integrity::fix_target(&crate::registry::default_root()?,&id,None,true)?)));ui.close_menu();}
                        if ui.add_enabled(!reviewing&&self.result.is_none(),egui::Button::new("Fix / rebuild Wasm")).clicked(){let id=id.clone();self.begin_work(move||Ok(Work::Message(crate::integrity::fix_target(&crate::registry::default_root()?,&id,None,false)?)));ui.close_menu();}
                        if !m.shared_assets.is_empty(){ui.label("Original .gamepkgstore path");ui.text_edit_singleline(&mut self.source);}
                        if !m.shared_assets.is_empty()&&ui.add_enabled(!reviewing&&self.result.is_none(),egui::Button::new("Repair assets from store")).clicked(){let id=id.clone();let source=self.source.clone();self.begin_work(move||Ok(Work::Message(crate::integrity::fix_target(&crate::registry::default_root()?,&id,Some(std::path::Path::new(source.trim())),true)?)));ui.close_menu();}
                        if ui.add_enabled(!reviewing&&self.wasm_built.contains(&id)&&self.result.is_none(),egui::Button::new("Export for web")).clicked(){let id=id.clone();self.begin_work(move||{let root=crate::registry::default_root()?;let suffix=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();let output=root.join("web-exports").join(format!("{id}-{suffix}"));crate::web_export::export(&root,&id,&output)?;Ok(Work::Message(format!("Web player exported to {}",output.display())))});ui.close_menu();}
                    });
                });
                if reviewing { ui.label("Finish or cancel the package review before playing."); }
                if self.result.is_some() { ui.label("Waiting for the current operation to finish. Play is rechecked automatically when it completes."); }
                for blocker in &blockers { ui.label(format!("Before playing: {blocker}")); }
                ui.horizontal_wrapped(|ui| {
                    if m.game_data.is_some() && !m.uses_service_runtime() && self.applied_settings.as_ref().is_none_or(|s|s.memory_ceiling_mib<2048)
                        && ui.add_enabled(self.result.is_none(), egui::Button::new("Open memory settings")).clicked() {
                        match crate::settings::load() {
                            Ok(settings) => { self.settings_draft=settings;self.show_settings=true;self.focus_memory_settings=true;self.show_add=false;self.show_support=false;self.preview=None; }
                            Err(error) => self.status=format!("Unable to load settings: {error:#}"),
                        }
                    }
                    if ui.add_enabled(self.result.is_none(), egui::Button::new("Recheck readiness")).clicked() {
                        self.begin_work(|| Ok(Work::Message("Game readiness refreshed.".into())));
                    }
                });
                if m.requires_rom&&!self.rom_paths.get(&id).is_some_and(|p|crate::rom_selections::ready_for(&m,p)){ui.label("Choose the required ROM files below to enable Play.");}
                self.game_launch_editor(ui, &m);
            }else if self.pending.is_none(){
                if let Some(entry)=self.catalog.iter().find(|e|e.id==id).cloned(){
                    ui.label(&entry.title);ui.label("Not downloaded. Catalog information is unverified until the signed package is checked.");
                    if entry.requires_rom==Some(true){ui.label("Requires your own ROM or game data files.");}
                    if ui.add_enabled(self.result.is_none(),egui::Button::new("Download / inspect").min_size(egui::vec2(160.0,36.0))).clicked(){let id=id.clone();self.begin_work(move||{let (m,file)=crate::catalog::fetch(&crate::registry::default_root()?,&id)?;Ok(Work::Update(m,file))});}
                }else{ui.label("This game is no longer in this library.");}
            }
            if let Some(notice)=self.updates.notices.get(&id){match notice {crate::updates::Notice::Available(version)=>{ui.label(format!("Update {version} available"));},crate::updates::Notice::Current=>{ui.label("Up to date");},crate::updates::Notice::Failed(error)=>{ui.label("Update check failed").on_hover_text(error);}}}
            ui.add_space(12.0);ui.label(&self.status);
            if self.pending.is_none(){self.game_rom_editor(ui);self.game_data_editor(ui);self.game_network_editor(ui);}
            self.game_details(ui,ctx);
            });
        });
    }

    fn pick_game_data(
        &mut self,
        selected: crate::manifest::Manifest,
        current: String,
        title: String,
        extensions: Vec<String>,
        pick_file: bool,
    ) {
        let external = selected
            .game_data
            .as_ref()
            .is_some_and(|data| !data.layouts.is_empty());
        self.begin_work(move || {
            let root = std::fs::canonicalize(crate::registry::default_root()?)?;
            let mut dialog = rfd::FileDialog::new().set_title(if external {
                title
            } else {
                "Choose original game data".into()
            });
            if !current.is_empty() {
                let path = std::path::Path::new(&current);
                if path.is_dir() {
                    dialog = dialog.set_directory(path);
                } else if let Some(parent) = path.parent() {
                    dialog = dialog.set_directory(parent);
                }
            }
            let picked = if pick_file {
                let refs = extensions.iter().map(String::as_str).collect::<Vec<_>>();
                dialog.add_filter("Game data", &refs).pick_file()
            } else {
                dialog.pick_folder()
            };
            Ok(Work::DataPicked(root, selected, picked))
        });
    }

    fn game_data_editor(&mut self, ui: &mut egui::Ui) {
        let Some(selected) = self
            .installed
            .iter()
            .find(|m| Some(&m.id) == self.selected_game.as_ref() && m.game_data.is_some())
            .cloned()
        else {
            return;
        };
        let external = selected
            .game_data
            .as_ref()
            .is_some_and(|data| !data.layouts.is_empty());
        let native_folder = selected.uses_service_runtime();
        let title = selected.game_data.as_ref().unwrap().title.clone();
        let current = self
            .data_paths
            .get(&selected.id)
            .cloned()
            .unwrap_or_default();
        ui.strong(&title);
        let extensions = selected
            .game_data
            .as_ref()
            .into_iter()
            .flat_map(|data| &data.layouts)
            .filter(|layout| layout.placement == "beside")
            .flat_map(|layout| &layout.extensions)
            .flat_map(|extension| [extension.clone(), extension.to_ascii_uppercase()])
            .collect::<Vec<_>>();
        let prefer_file = !extensions.is_empty()
            && (current.is_empty() || !std::path::Path::new(&current).is_dir());
        let label = if current.is_empty() {
            if prefer_file {
                "Choose game-data file…"
            } else {
                "Choose game-data folder…"
            }
        } else {
            &current
        };
        if ui
            .add_enabled(
                self.result.is_none(),
                egui::Button::new(label)
                    .truncate()
                    .min_size(egui::vec2(ui.available_width().min(620.0), 38.0)),
            )
            .clicked()
        {
            self.pick_game_data(
                selected.clone(),
                current.clone(),
                title.clone(),
                extensions.clone(),
                prefer_file,
            );
        }
        if !extensions.is_empty()
            && ui
                .add_enabled(
                    self.result.is_none(),
                    egui::Button::new(if prefer_file {
                        "Choose folder instead…"
                    } else {
                        "Choose file instead…"
                    }),
                )
                .clicked()
        {
            self.pick_game_data(selected, current, title, extensions, !prefer_file);
        }
        if native_folder {
            ui.label("Choose the original game-data folder. GamePKG reads only the declared subfolders in place.");
        } else if external {
            ui.label("Choose an original game-data file or a folder that matches this game's data layout. The files stay on your disk and are not copied into the package. Saves stay in the library.");
        } else {
            ui.label("Choose the original installation containing data and packages folders. GamePKG reads these files in place and remembers this folder for this signed game.");
        }
        if self
            .applied_settings
            .as_ref()
            .is_none_or(|s| s.memory_ceiling_mib < 2048)
        {
            ui.label("This engine needs a memory ceiling of at least 2048 MiB. Adjust it in Settings before playing.");
        }
        if !external {
            ui.label("A 500 ms tick deadline is recommended for first-use software shader compilation. Adjust it in Settings.");
        }
        ui.add_space(12.0);
    }
    fn game_rom_editor(&mut self, ui: &mut egui::Ui) {
        if let Some(selected) = self
            .installed
            .iter()
            .find(|m| Some(&m.id) == self.selected_game.as_ref() && m.requires_rom)
            .cloned()
        {
            let mut pick = None;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.strong("Your game files");
                for index in 0..selected.required_rom_count() {
                    let title = selected.roms.get(index).map(|r| r.title.clone())
                        .unwrap_or_else(|| format!("ROM {}", index + 1));
                    let path = if index == 0 { &self.rom_path } else { &self.additional_rom_paths[index - 1] };
                    ui.vertical(|ui| {
                        ui.label(&title);
                        let text = if path.is_empty() { "Choose ROM file…" } else { path };
                        if ui.add_enabled(self.result.is_none(), egui::Button::new(text).truncate()
                            .min_size(egui::vec2(ui.available_width().min(620.0), 38.0)))
                            .on_hover_text(if path.is_empty() { "Open the system file picker" } else { path })
                            .clicked() {
                            pick = Some((index, title, path.clone()));
                        }
                    });
                }
                ui.label("Click a file field to choose a ROM. Locations are remembered for this game. Files stay on your drive and are shared read-only.");
            });
            if let Some((index, title, current)) = pick {
                match crate::registry::default_root()
                    .and_then(|root| super::rom_picker::Selection::new(root, selected, index))
                {
                    Ok(selection) => self.begin_work(move || {
                        let mut dialog =
                            rfd::FileDialog::new().set_title(format!("Choose {title}"));
                        if let Some(parent) = std::path::Path::new(&current)
                            .parent()
                            .filter(|p| p.is_dir())
                        {
                            dialog = dialog.set_directory(parent);
                        }
                        Ok(Work::RomPicked(selection.with_path(dialog.pick_file())))
                    }),
                    Err(error) => self.status = format!("Unable to select ROM file: {error:#}"),
                }
            }
            ui.add_space(12.0);
        }
    }
    fn game_launch_editor(&mut self, ui: &mut egui::Ui, manifest: &crate::manifest::Manifest) {
        use crate::launch_options::{Antialiasing, AspectRatio, TextureLod, WindowMode};
        let id = manifest.id.clone();
        let current = self.launch_drafts.get(&id).copied().or_else(|| {
            crate::registry::default_root().ok().and_then(|root| {
                crate::launch_options::effective(&root, manifest)
                    .ok()
                    .flatten()
            })
        });
        let mut draft = current.unwrap_or_default();
        let before = draft;
        ui.collapsing("Graphics preferences (native)", |ui| {
            ui.horizontal(|ui| {
                ui.label("Window mode");
                egui::ComboBox::from_id_salt(format!("window-mode-{id}"))
                    .selected_text(draft.window_mode.label())
                    .show_ui(ui, |ui| {
                        for &value in WindowMode::ALL {
                            ui.selectable_value(&mut draft.window_mode, value, value.label());
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Aspect ratio");
                egui::ComboBox::from_id_salt(format!("aspect-ratio-{id}"))
                    .selected_text(draft.aspect_ratio.label())
                    .show_ui(ui, |ui| {
                        for &value in AspectRatio::ALL {
                            ui.selectable_value(&mut draft.aspect_ratio, value, value.label());
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut draft.render_scale, 1..=4).text("Render scale"));
            });
            ui.horizontal(|ui| {
                ui.label("Antialiasing");
                egui::ComboBox::from_id_salt(format!("antialiasing-{id}"))
                    .selected_text(draft.antialiasing.label())
                    .show_ui(ui, |ui| {
                        for &value in Antialiasing::ALL {
                            ui.selectable_value(&mut draft.antialiasing, value, value.label());
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Texture LOD");
                egui::ComboBox::from_id_salt(format!("texture-lod-{id}"))
                    .selected_text(draft.texture_lod.label())
                    .show_ui(ui, |ui| {
                        for &value in TextureLod::ALL {
                            ui.selectable_value(&mut draft.texture_lod, value, value.label());
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut draft.sharpening_percent, 0..=100)
                        .text("Sharpening (%)"),
                );
            });
            ui.checkbox(&mut draft.widescreen_hud, "Widescreen HUD");
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut draft.menu_hud_scale, 1..=4).text("Menu/HUD scale"),
                );
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.result.is_none(), egui::Button::new("Save graphics"))
                    .clicked()
                {
                    match draft.validate() {
                        Ok(()) => {
                            self.launch_drafts.insert(id.clone(), draft);
                            let id = id.clone();
                            let save = draft;
                            self.begin_work(move || {
                                let root = crate::registry::default_root()?;
                                let manifest =
                                    crate::registry::installed_signed_identity(&root, &id)?;
                                let _change = crate::registry::LibraryChange::acquire(&root)?;
                                crate::launch_options::remember(&root, &manifest, Some(save))?;
                                Ok(Work::Message(
                                    "Graphics preferences saved for the next launch.".into(),
                                ))
                            });
                        }
                        Err(error) => self.status = format!("{error:#}"),
                    }
                }
                if ui
                    .add_enabled(self.result.is_none(), egui::Button::new("Reset graphics"))
                    .clicked()
                {
                    self.launch_drafts.remove(&id);
                    let id = id.clone();
                    self.begin_work(move || {
                        let root = crate::registry::default_root()?;
                        let manifest = crate::registry::installed_signed_identity(&root, &id)?;
                        let _change = crate::registry::LibraryChange::acquire(&root)?;
                        crate::launch_options::remember(&root, &manifest, None)?;
                        Ok(Work::Message(
                            "Graphics preferences reset to the signed game defaults.".into(),
                        ))
                    });
                }
            });
            ui.weak("Saved choices override the signed defaults for this signed game identity only. Command-line flags override saved choices.");
        });
        if before != draft {
            self.launch_drafts.insert(id, draft);
        }
    }

    fn game_details(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(m) = self.manifest.clone() {
            ui.separator();
            ui.heading("About this game");
            ui.label(format!("Version: {}", m.version));
            ui.label(format!(
                "Memory: {:.1} MiB recommended • {:.1} MiB hard limit",
                m.limits.recommended_memory_bytes() as f64 / 1048576.0,
                m.limits.hard_memory_bytes as f64 / 1048576.0
            ));
            if let Some(profile) = m.limits.render_profile {
                ui.label(format!("Artwork cache: up to {} MiB, plus up to {} MiB per frame, game memory and graphics-driver storage.", profile.cache_bytes()/1048576, profile.frame_bytes()/1048576));
            }
            ui.label(if m.requires_rom { format!("Requires {} ROM file(s) supplied by you. They are not included in this package.",m.required_rom_count()) } else if m.game_data.is_some(){"Original game-data folder required (read-only).".into()} else { "No user ROM required.".into() });
            for (position, rom) in m.roms.iter().enumerate() {
                ui.label(format!("File {}: {}", position + 1, rom.title));
            }
            let package_size = self
                .pending
                .as_ref()
                .and_then(|file| file.as_file().metadata().ok())
                .map(|m| m.len())
                .or_else(|| self.usage.get(&m.id).map(|u| u.package_bytes));
            ui.label(format!(
                "Storage: {} package; {} expected after building",
                package_size
                    .map(crate::storage::bytes)
                    .unwrap_or_else(|| "Unknown".into()),
                m.limits
                    .recommended_storage_bytes
                    .map(|v| v.max(package_size.unwrap_or(0)))
                    .map(crate::storage::bytes)
                    .unwrap_or_else(|| "Unknown (no creator estimate)".into())
            ));
            ui.label(format!(
                "Creator fingerprint: {}",
                crate::verify::digest(&hex::decode(&m.creator_key).unwrap_or_default())
            ));
            ui.label("A valid signature proves key ownership, not a real-world identity. Confirm the creator before first installation.");
            if !m.achievements.is_empty() {
                let same_creator = self
                    .installed
                    .iter()
                    .any(|old| old.id == m.id && old.creator_key == m.creator_key);
                let record = if same_creator {
                    self.history.get(&m.id)
                } else {
                    None
                };
                egui::CollapsingHeader::new("Achievements")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label("Local progress reported by this game.");
                        for achievement in &m.achievements {
                            let status = match record {
                                Some(record) if record.unlocked.contains_key(&achievement.id) => {
                                    "Unlocked"
                                }
                                Some(_) => "Locked",
                                None if same_creator => "Unavailable",
                                None => "Locked",
                            };
                            ui.strong(format!("{status} — {}", achievement.title));
                            ui.label(&achievement.description);
                        }
                    });
            }
            if !m.shared_assets.is_empty() {
                ui.label(format!(
                    "Shared store assets: {} (must be available locally to install and play)",
                    m.shared_assets.len()
                ));
            }
            ui.label(if m.uses_service_runtime() || m.native_tick {
                "Runtime: Native GamePKG compatibility"
            } else {
                "Runtime: GamePKG"
            });
            ui.label(format!(
                "Engine requirement: {} • Save schema: {}",
                m.engine_version, m.save_schema
            ));
            if self.pending.is_some()
                && self
                    .installed
                    .iter()
                    .any(|old| old.id == m.id && old.save_schema != m.save_schema)
            {
                ui.label("This update uses a different save format. A separate save starts; your earlier save is kept.");
            }
            if self.pending.is_none() {
                if let Some(notice) = self.package_notices.get(&m.id) {
                    egui::CollapsingHeader::new("Credits and licenses").show(ui, |ui| {
                        ui.add(egui::Label::new(notice).selectable(true).wrap());
                    });
                }
            }
            ui.label(format!(
                "Lockfile / dependency inventory SHA-256: {}",
                m.lockfile_sha256
            ));
            ui.label(format!(
                "Permissions: input={}, audio={}, save={}, network={}",
                m.permissions.input, m.permissions.audio, m.permissions.save, m.permissions.network
            ));
            for permission in &m.permissions.files {
                ui.label(format!(
                    "Selected files: {} ({}) — {}{}; up to {}. Each file is chosen by you.",
                    permission.title,
                    permission.extensions.join(", "),
                    if permission.read { "import " } else { "" },
                    if permission.write { "export" } else { "" },
                    crate::storage::bytes(permission.max_bytes)
                ));
            }
            ui.label(if m.permissions.network {
                "Networking enabled: this game can use the internet and local network."
            } else {
                "Networking disabled."
            });
            if let Some(url) = &m.update_url {
                ui.label(format!("Updates: {url}"));
            }
            let preview_id = m.id.clone();
            ui.horizontal(|ui| {
                for (label, prefer_theora) in
                    [("Play preview", false), ("Play Theora fallback", true)]
                {
                    if ui
                        .add_enabled(self.result.is_none(), egui::Button::new(label))
                        .clicked()
                    {
                        self.preview = None;
                        let snapshot = self.pending.as_ref().and_then(|file| file.reopen().ok());
                        let id = preview_id.clone();
                        self.begin_work(move || {
                            let mut copy = tempfile::NamedTempFile::new()?;
                            let mut input = match snapshot {
                                Some(file) => file,
                                None => crate::files::open_regular(
                                    &crate::registry::default_root()?.join(format!("{id}.gamepkg")),
                                    u64::MAX,
                                    false,
                                )?,
                            };
                            std::io::copy(&mut input, &mut copy)?;
                            let mut pkg = crate::archive::Package::open(copy.path())?;
                            crate::verify::verify_embedded(&mut pkg)?;
                            let name =
                                if !prefer_theora && pkg.entries.contains_key("preview.av1.webm") {
                                    "preview.av1.webm"
                                } else {
                                    "preview.ogv"
                                };
                            Ok(Work::Preview(crate::previews::decode(
                                name,
                                &pkg.read(name)?,
                            )?))
                        });
                    }
                }
            });
            let mut stop_preview = false;
            if let Some((frames, start, texture)) = &mut self.preview {
                let index = ((start.elapsed().as_millis() / 200) as usize).min(frames.len() - 1);
                texture.set(
                    egui::ColorImage::from_rgb([320, 240], &frames[index]),
                    egui::TextureOptions::LINEAR,
                );
                ui.image((texture.id(), egui::vec2(320.0, 240.0)));
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
                stop_preview = ui.button("Close preview").clicked();
            }
            if stop_preview {
                self.preview = None;
            }
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
    fn manifest_limits_explain_the_memory_ceiling_before_play() {
        let mut manifest = crate::manifest::Manifest::parse(include_bytes!(
            "../../examples/minimal/manifest.toml"
        ))
        .unwrap();
        manifest.native_services = true;
        manifest.limits.hard_memory_bytes = 4096 * 1048576;
        let mut app = App {
            applied_settings: Some(crate::settings::Settings::default()),
            ..Default::default()
        };
        app.built.insert(manifest.id.clone());
        assert!(!app.ready_to_play(&manifest));
        assert!(app
            .play_blockers(&manifest)
            .iter()
            .any(|text| text.contains("4096 MiB")));
        app.applied_settings.as_mut().unwrap().memory_ceiling_mib = 4096;
        assert_eq!(
            app.ready_to_play(&manifest),
            crate::native::play_supported(&manifest)
        );
        if crate::native::play_supported(&manifest) {
            assert!(app.play_blockers(&manifest).is_empty());
        }
    }

    #[test]
    fn missing_named_roms_and_missing_builds_have_separate_play_explanations() {
        let mut manifest: crate::manifest::Manifest =
            toml::from_str(include_str!("../../examples/minimal/manifest.toml")).unwrap();
        manifest.native_services = true;
        manifest.requires_rom = true;
        manifest.roms = vec![
            crate::manifest::RomRequirement {
                id: "first".into(),
                title: "First ROM".into(),
            },
            crate::manifest::RomRequirement {
                id: "second".into(),
                title: "Second ROM".into(),
            },
        ];
        let folder = tempfile::tempdir().unwrap();
        let first = folder.path().join("first.rom");
        std::fs::write(&first, b"first").unwrap();
        let second = folder.path().join("second.rom");
        std::fs::write(&second, b"second").unwrap();
        let mut app = App::default();
        app.built.insert(manifest.id.clone());
        app.rom_paths.insert(
            manifest.id.clone(),
            vec![first.to_string_lossy().into_owned(), String::new()],
        );
        assert!(app
            .play_blockers(&manifest)
            .iter()
            .any(|text| text.contains("Second ROM")));
        app.rom_paths.get_mut(&manifest.id).unwrap()[1] = second.to_string_lossy().into_owned();
        if crate::native::play_supported(&manifest) {
            assert!(app.play_blockers(&manifest).is_empty());
        }
        app.built.clear();
        if crate::native::play_supported(&manifest) {
            assert!(app
                .play_blockers(&manifest)
                .iter()
                .any(|text| text.contains("Build this game")));
        }
        std::fs::remove_file(&first).unwrap();
        assert!(app
            .play_blockers(&manifest)
            .iter()
            .any(|text| text.contains("First ROM")));
    }

    #[test]
    fn opening_catalog_and_installed_pages_never_downloads_installs_or_plays() {
        let mut app = App::default();
        app.catalog.push(crate::catalog::Entry {
            id: "catalog.game".into(),
            title: "Catalog game".into(),
            description: None,
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
