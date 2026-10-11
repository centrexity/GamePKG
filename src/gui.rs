use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc,
};
enum Work {
    Store(crate::store::Bundle),
    Preview(Vec<Vec<u8>>),
    Game(Box<crate::runtime::Game>),
    Package(crate::manifest::Manifest),
    Update(crate::manifest::Manifest, tempfile::NamedTempFile),
    Message(String),
}
#[derive(Default)]
pub struct App {
    gl: Option<std::sync::Arc<eframe::glow::Context>>,
    gpu: Option<std::sync::Arc<crate::gpu::Renderer>>,
    applied_settings: Option<crate::settings::Settings>,
    updates: crate::updates::Monitor,
    source: String,
    rom_path: String,
    show_settings: bool,
    settings_draft: crate::settings::Settings,
    player: Option<Box<crate::runtime::Game>>,
    next_frame: Option<std::time::Instant>,
    pending_input: u32,
    audio_player: Option<crate::audio::Player>,
    audio_error: String,
    status: String,
    result: Option<mpsc::Receiver<Result<Work, String>>>,
    manifest: Option<crate::manifest::Manifest>,
    pending: Option<tempfile::NamedTempFile>,
    pending_store: Option<crate::store::Bundle>,
    installed: Vec<crate::manifest::Manifest>,
    catalog: Vec<crate::catalog::Entry>,
    loaded: bool,
    icons: HashMap<String, egui::TextureHandle>,
    built: HashSet<String>,
    preview: Option<(Vec<Vec<u8>>, std::time::Instant, egui::TextureHandle)>,
}
impl App {
    pub fn new(play: Option<String>, gl: Option<std::sync::Arc<eframe::glow::Context>>) -> Self {
        let mut app = Self {
            gl,
            ..Self::default()
        };
        match crate::settings::load() {
            Ok(settings) => {
                app.applied_settings = Some(settings.clone());
                app.settings_draft = settings;
            }
            Err(e) => app.status = format!("Unable to load settings: {e:#}"),
        }
        if let Some(id) = play {
            app.begin_work(move || {
                Ok(Work::Game(Box::new(crate::runtime::Game::load(
                    &crate::registry::default_root()?,
                    &id,
                )?)))
            });
        }
        app
    }
    fn refresh(&mut self, ctx: &egui::Context) {
        match crate::registry::default_root().and_then(|r| crate::catalog::entries(&r)) {
            Ok(entries) => self.catalog = entries,
            Err(e) => self.status = format!("Unable to read catalog: {e:#}"),
        }
        match crate::registry::default_root().and_then(|root| crate::registry::list(&root)) {
            Ok(games) => {
                self.icons.clear();
                self.built.clear();
                if let Ok(root) = crate::registry::default_root() {
                    for m in &games {
                        if crate::build::is_built(&root, &m.id).unwrap_or(false) {
                            self.built.insert(m.id.clone());
                        }
                        let decoded = (|| -> anyhow::Result<_> {
                            let mut pkg = crate::archive::Package::open(
                                &root.join(format!("{}.gamepkg", m.id)),
                            )?;
                            let bytes = pkg.read("thumbnail.png")?;
                            let mut reader = image::ImageReader::with_format(
                                std::io::Cursor::new(bytes),
                                image::ImageFormat::Png,
                            );
                            let mut limits = image::Limits::default();
                            limits.max_image_width = Some(512);
                            limits.max_image_height = Some(512);
                            limits.max_alloc = Some(4 * 1024 * 1024);
                            reader.limits(limits);
                            let icon = reader.decode()?.to_rgba8();
                            Ok(egui::ColorImage::from_rgba_unmultiplied(
                                [icon.width() as usize, icon.height() as usize],
                                icon.as_raw(),
                            ))
                        })();
                        if let Ok(icon) = decoded {
                            self.icons.insert(
                                m.id.clone(),
                                ctx.load_texture(&m.id, icon, egui::TextureOptions::LINEAR),
                            );
                        }
                    }
                }
                self.installed = games;
            }
            Err(e) => self.status = format!("Unable to read installed library: {e:#}"),
        }
    }
    fn begin(
        &mut self,
        task: impl FnOnce() -> anyhow::Result<crate::manifest::Manifest> + Send + 'static,
    ) {
        self.begin_work(move || task().map(Work::Package));
    }
    fn begin_work(&mut self, task: impl FnOnce() -> anyhow::Result<Work> + Send + 'static) {
        self.pending_store = None;
        let (tx, rx) = mpsc::channel();
        self.result = Some(rx);
        self.status = "Working…".into();
        std::thread::spawn(move || {
            let _ = tx.send(task().map_err(|e| format!("{e:#}")));
        });
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if !self.loaded {
            self.loaded = true;
            ctx.set_visuals(if self.settings_draft.dark_mode {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            self.refresh(ctx);
        }
        if let Some(rx) = &self.result {
            match rx.try_recv() {
                Ok(result) => {
                    self.result = None;
                    match result {
                        Ok(Work::Store(bundle)) => {
                            self.preview = None;
                            self.manifest = None;
                            self.pending = None;
                            self.status = format!(
                                "Verified store: {} games. Review creators before installing.",
                                bundle.games().len()
                            );
                            self.pending_store = Some(bundle);
                        }
                        Ok(Work::Preview(frames)) => {
                            let texture = ctx.load_texture(
                                "game-preview",
                                egui::ColorImage::from_rgb([320, 240], &frames[0]),
                                egui::TextureOptions::LINEAR,
                            );
                            self.preview = Some((frames, std::time::Instant::now(), texture));
                            self.status = "Preview ready.".into();
                        }
                        Ok(Work::Game(game)) => {
                            self.preview = None;
                            self.gpu = None;
                            if !game.shader_sources.is_empty() {
                                let renderer = self
                                    .gl
                                    .clone()
                                    .ok_or_else(|| anyhow::anyhow!("OpenGL renderer unavailable"))
                                    .and_then(|gl| {
                                        crate::gpu::Renderer::new(gl, &game.shader_sources)
                                    });
                                match renderer {
                                    Ok(renderer) => self.gpu = Some(std::sync::Arc::new(renderer)),
                                    Err(error) => {
                                        self.status = format!("Unable to play: {error:#}");
                                        return;
                                    }
                                }
                            }
                            self.next_frame = None;
                            if let Some(audio) = &self.audio_player {
                                audio.clear();
                            }
                            self.pending_input = 0;
                            self.audio_player = None;
                            self.audio_error.clear();
                            if game.audio_allowed {
                                match crate::settings::load() {
                                    Ok(settings)
                                        if !settings.mute_audio && settings.audio_volume > 0 =>
                                    {
                                        match crate::audio::Player::new(settings.audio_volume) {
                                            Ok(player) => self.audio_player = Some(player),
                                            Err(error) => {
                                                self.audio_error =
                                                    format!("Sound unavailable: {error:#}")
                                            }
                                        }
                                    }
                                    Ok(_) => {}
                                    Err(error) => {
                                        self.audio_error = format!("Sound disabled: {error:#}")
                                    }
                                }
                            }
                            self.player = Some(game);
                        }
                        Ok(Work::Package(m)) => {
                            self.preview = None;
                            self.status = format!("Verified {} {}", m.id, m.version);
                            self.manifest = Some(m);
                            self.refresh(ctx);
                        }
                        Ok(Work::Update(m, file)) => {
                            self.preview = None;
                            self.status = format!(
                                "Package ready: {} {}. Review and install below.",
                                m.id, m.version
                            );
                            self.manifest = Some(m);
                            self.pending = Some(file);
                        }
                        Ok(Work::Message(message)) => {
                            self.status = message;
                            self.refresh(ctx);
                        }
                        Err(e) => {
                            self.status = format!("Failed: {e}");
                            self.manifest = None;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = "Worker stopped.".into();
                    self.result = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100))
                }
            }
        }
        if self.player.is_none() && self.result.is_none() && !self.show_settings {
            if let (Some(settings), Ok(root)) =
                (&self.applied_settings, crate::registry::default_root())
            {
                self.updates.tick(
                    &root,
                    &self.installed,
                    settings.automatic_update_checks,
                    settings.update_interval_hours,
                    std::time::Instant::now(),
                );
                ctx.request_repaint_after(std::time::Duration::from_secs(
                    if self.updates.checking() { 1 } else { 60 },
                ));
            }
        }
        if let Some(game) = self.player.as_mut() {
            let mut close = false;
            let mut failure = None;
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(&game.id);
                    if ui.button("Back to library").clicked() {
                        close = true;
                    }
                    if ui.button("Restart").clicked() {
                        game.state = 0;
                        self.next_frame = None;
                        if let Some(audio) = &self.audio_player {
                            audio.clear();
                        }
                    }
                });
                let input = ctx.input(|i| {
                    (u32::from(i.key_down(egui::Key::ArrowLeft) || i.key_down(egui::Key::A)) << 8)
                        | (u32::from(i.key_down(egui::Key::ArrowRight) || i.key_down(egui::Key::D))
                            << 9)
                        | (u32::from(i.key_down(egui::Key::ArrowUp) || i.key_down(egui::Key::W))
                            << 10)
                        | (u32::from(i.key_down(egui::Key::ArrowDown) || i.key_down(egui::Key::S))
                            << 11)
                        | u32::from(
                            i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::A),
                        )
                        | (u32::from(
                            i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::D),
                        ) << 1)
                        | (u32::from(
                            i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::W),
                        ) << 2)
                        | (u32::from(
                            i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::S),
                        ) << 3)
                });
                self.pending_input |= input & 15;
                let now = std::time::Instant::now();
                if self.next_frame.is_none_or(|next| now >= next) {
                    match game.step(self.pending_input | (input & 0xf00)) {
                        Err(e) => failure = Some(format!("Game stopped: {e:#}")),
                        Ok(_) => {
                            if let Some(audio) = &self.audio_player {
                                audio.submit(game.audio_frame());
                            }
                        }
                    }
                    self.pending_input = 0;
                    self.next_frame = Some(now + std::time::Duration::from_millis(16));
                }
                if let Some(error) = self.audio_player.as_ref().and_then(|a| a.error()) {
                    self.audio_error = format!("Sound stopped: {error}");
                    self.audio_player = None;
                }
                if let Some(audio) = &self.audio_player {
                    ui.colored_label(
                        if audio.non_silent_frames() > 0 {
                            egui::Color32::GREEN
                        } else {
                            egui::Color32::GRAY
                        },
                        "Sound on",
                    );
                } else if game.audio_allowed && self.audio_error.is_empty() {
                    ui.label("Sound muted");
                }
                if !self.audio_error.is_empty() {
                    ui.label(&self.audio_error);
                }
                if game.custom_renderer {
                    ui.label("Arrows or WASD to move.");
                } else {
                    ui.label(format!("Score: {} — Arrows or WASD to move.", game.score()));
                }
                if game.custom_renderer {
                    let scale = (ui.available_width() / 640.0)
                        .min(ui.available_height() / 480.0)
                        .clamp(0.1, 2.0);
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(640.0 * scale, 480.0 * scale),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter_at(rect);
                    let color = |rgba: u32| {
                        egui::Color32::from_rgba_unmultiplied(
                            (rgba >> 24) as u8,
                            (rgba >> 16) as u8,
                            (rgba >> 8) as u8,
                            rgba as u8,
                        )
                    };
                    painter.rect_filled(
                        rect,
                        0.0,
                        color(game.frame().background.unwrap_or(0x10171dff)),
                    );
                    for command in &game.frame().commands {
                        match *command {
                            crate::drawing::Command::Shader { index } => {
                                if let Some(renderer) = &self.gpu {
                                    let renderer = renderer.clone();
                                    painter.add(egui::PaintCallback {
                                        rect,
                                        callback: std::sync::Arc::new(
                                            eframe::egui_glow::CallbackFn::new(move |_, _| {
                                                renderer.paint(index)
                                            }),
                                        ),
                                    });
                                }
                            }
                            crate::drawing::Command::Rect {
                                x,
                                y,
                                width,
                                height,
                                rgba,
                            } => {
                                painter.rect_filled(
                                    egui::Rect::from_min_size(
                                        rect.min + egui::vec2(x * scale, y * scale),
                                        egui::vec2(width * scale, height * scale),
                                    ),
                                    0.0,
                                    color(rgba),
                                );
                            }
                            crate::drawing::Command::Circle { x, y, radius, rgba } => {
                                painter.circle_filled(
                                    rect.min + egui::vec2(x * scale, y * scale),
                                    radius * scale,
                                    color(rgba),
                                );
                            }
                        }
                    }
                    return;
                }
                let size = ui
                    .available_width()
                    .min(ui.available_height())
                    .clamp(160.0, 640.0);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                let painter = ui.painter_at(rect);
                let cell = size / 16.0;
                painter.rect_filled(rect, 0.0_f32, egui::Color32::from_rgb(16, 23, 29));
                for n in 0..=16 {
                    let v = n as f32 * cell;
                    painter.line_segment(
                        [
                            rect.min + egui::vec2(v, 0.0),
                            rect.min + egui::vec2(v, size),
                        ],
                        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(45)),
                    );
                    painter.line_segment(
                        [
                            rect.min + egui::vec2(0.0, v),
                            rect.min + egui::vec2(size, v),
                        ],
                        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(45)),
                    );
                }
                let position = |(x, y): (u8, u8)| {
                    rect.min
                        + egui::vec2(
                            (x.min(15) as f32 + 0.5) * cell,
                            (y.min(15) as f32 + 0.5) * cell,
                        )
                };
                painter.circle_filled(position(game.target()), cell * 0.28, egui::Color32::YELLOW);
                painter.circle_filled(
                    position(game.player()),
                    cell * 0.35,
                    egui::Color32::from_rgb(100, 220, 180),
                );
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
            if close || failure.is_some() {
                self.player = None;
                self.gpu = None;
                self.audio_player = None;
            }
            if let Some(e) = failure {
                self.status = e;
            }
            return;
        }
        egui::TopBottomPanel::top("navigation").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(!self.show_settings, "Library")
                    .clicked()
                {
                    self.show_settings = false;
                    self.preview = None;
                }
                if ui
                    .add_enabled(self.result.is_none(), egui::Button::new("Settings"))
                    .clicked()
                {
                    self.show_settings = true;
                    self.preview = None;
                    match crate::settings::load() {
                        Ok(settings) => self.settings_draft = settings,
                        Err(e) => self.status = format!("Unable to load settings: {e:#}"),
                    }
                }
            });
        });
        if self.show_settings {
            egui::CentralPanel::default().show(ctx,|ui| {
                ui.heading("Settings");
                ui.label("Choose how GamePKG stores, builds, and runs your games.");
                egui::ScrollArea::vertical().max_height((ui.available_height() - 110.0).max(100.0)).show(ui,|ui| {
                    ui.heading("Library");
                    ui.label("Library folder (absolute path; leave empty for the default)");
                    ui.text_edit_singleline(&mut self.settings_draft.library_directory);
                    ui.label("Changing the folder opens another library. Existing games are not moved.");
                    if std::env::var_os("GAMEPKG_HOME").is_some() { ui.label("The current launch uses a library override. The saved folder applies when launched without that override."); }
                    if let Ok(root) = crate::registry::default_root() { ui.label(format!("Current library: {}",root.display())); }
                    ui.separator();ui.heading("Tor downloads");
                    ui.label("Local SOCKS proxy (IP:port)");ui.text_edit_singleline(&mut self.settings_draft.tor_proxy);
                    ui.label("Run Tor separately. Onion downloads never fall back to a direct connection.");
                    if std::env::var_os("GAMEPKG_TOR_PROXY").is_some() { ui.label("GAMEPKG_TOR_PROXY overrides this setting for the current launch."); }
                    ui.separator();ui.heading("Game resource ceilings");
                    ui.horizontal(|ui| { ui.label("Memory ceiling (MiB)");ui.add(egui::DragValue::new(&mut self.settings_draft.memory_ceiling_mib).range(1..=512)); });
                    ui.horizontal(|ui| { ui.label("Fuel ceiling per tick");ui.add(egui::DragValue::new(&mut self.settings_draft.fuel_ceiling).range(1..=100000000).speed(1000)); });
                    ui.horizontal(|ui| { ui.label("Tick deadline (ms)");ui.add(egui::Slider::new(&mut self.settings_draft.tick_deadline_ms,10..=100).step_by(10.0)); });
                    ui.label("Memory and fuel use the lower of your ceiling and the signed game limit. These settings cannot raise a game's hard limit. Changes apply the next time you launch a game.");
                    ui.separator();ui.heading("Builds and appearance");
                    ui.label("Slang 2026.19 installation folder");
                    ui.text_edit_singleline(&mut self.settings_draft.slang_directory);
                    egui::ComboBox::from_label("Shader backend").selected_text(&self.settings_draft.shader_backend).show_ui(ui, |ui| {
                        for backend in ["spirv", "wgsl", "metal", "dxil", "glsl"] { ui.selectable_value(&mut self.settings_draft.shader_backend, backend.to_string(), backend); }
                    });
                    ui.label("GLSL supports resource-free fragment passes in this desktop player. Other backends compile and cache shaders.");
                    if self.settings_draft.shader_backend == "dxil" {
                        ui.label("DXC 1.9.2609 installation folder");
                        ui.text_edit_singleline(&mut self.settings_draft.dxc_directory);
                        ui.label("Choose the pinned DXC installation. GamePKG verifies its compiler libraries before each build.");
                        if std::env::var_os("GAMEPKG_DXC_ROOT").is_some() { ui.label("GAMEPKG_DXC_ROOT overrides this folder for the current launch."); }
                    }
                    ui.checkbox(&mut self.settings_draft.optimize_builds,"Optimize new builds");
                    ui.label("Applies on Build/Rebuild; existing compiled games are kept.");
                    ui.checkbox(&mut self.settings_draft.dark_mode,"Dark appearance");
                    ui.separator(); ui.heading("Sound");
                    ui.checkbox(&mut self.settings_draft.mute_audio,"Mute game audio");
                    ui.add(egui::Slider::new(&mut self.settings_draft.audio_volume,0..=100).text("Volume"));
                    ui.label("Applies when you next play a game. Only games with audio permission can submit sound.");
                    ui.separator(); ui.heading("Updates");
                    ui.checkbox(&mut self.settings_draft.automatic_update_checks,"Check for game updates automatically");
                    ui.add(egui::Slider::new(&mut self.settings_draft.update_interval_hours,1..=168).text("Hours between checks"));
                    ui.label("Checks run while the library is open and idle. They download signed packages to verify updates, then discard the download. Review and install updates yourself. Turning checks off stops new requests; an active request may finish.");
                });
                    ui.separator();
                    let validation = self.settings_draft.validate();
                    if let Err(error) = &validation {
                        ui.colored_label(egui::Color32::from_rgb(240, 130, 100), error.to_string());
                    }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(validation.is_ok(), egui::Button::new("Save settings")).clicked() {
                            let result=crate::settings::path().and_then(|path|self.settings_draft.save(&path));
                            match result {
                                Ok(()) => {
                                    ctx.set_visuals(if self.settings_draft.dark_mode { egui::Visuals::dark() } else { egui::Visuals::light() });
                                    self.applied_settings=Some(self.settings_draft.clone());
                                    self.pending=None;self.pending_store=None;self.manifest=None;self.installed.clear();self.catalog.clear();self.status="Settings saved.".into();self.refresh(ctx);
                                }
                                Err(e) => self.status=format!("Settings not saved: {e:#}"),
                            }
                        }
                        if ui.button("Cancel edits").clicked() { match crate::settings::load() { Ok(s) => {self.settings_draft=s;self.show_settings=false;} Err(e)=>self.status=format!("Unable to reload settings: {e:#}") } }
                        if ui.button("Reset form to defaults").clicked() {self.settings_draft=crate::settings::Settings::default();self.status="Defaults restored in the form. Save to apply.".into();}
                    });
                    ui.label(&self.status);
            });
            return;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("GamePKG Library");
            ui.label("Games — catalog entries download only when you choose");
            ui.horizontal(|ui| { ui.label("Your ROM file (for games that require one)"); ui.text_edit_singleline(&mut self.rom_path); });
            ui.label("Only the selected file is shared read-only for this play session.");
            let mut inspect = None;
            let mut update = None;
            let mut build = None;
            let mut download = None;
            let mut play = None;
            egui::ScrollArea::both().max_height(350.0).show(ui, |ui| {
                egui::Grid::new("installed_games").striped(true).min_col_width(70.0).show(ui, |ui| {
                    for label in ["Icon", "Game", "Version", "ROM", "State", "Actions", "Updates"] { ui.strong(label); }
                    ui.end_row();
                    for entry in &self.catalog {
                        if self.installed.iter().any(|m| m.id == entry.id) { continue; }
                        ui.label("🎮");
                        ui.label(format!("{}\n{}",entry.title,entry.id)).on_hover_text("Catalog information is unverified until the package is downloaded");
                        ui.label(entry.version.as_deref().unwrap_or("Unknown")).on_hover_text("Version claimed by the catalog");
                        ui.label(match entry.requires_rom { Some(true) => "User ROM required", Some(false) => "Not required", None => "Unknown" });
                        ui.label("Not downloaded");
                        if ui.add_enabled(self.result.is_none(),egui::Button::new("Download / inspect")).clicked() { download = Some(entry.id.clone()); }
                        ui.label("Download first"); ui.end_row();
                    }
                    for m in &self.installed {
                        if let Some(icon) = self.icons.get(&m.id) { ui.add(egui::Image::new(icon).fit_to_exact_size(egui::vec2(40.0,40.0))); }
                        else { ui.label("🎮").on_hover_text("Thumbnail unavailable or invalid"); }
                        ui.label(&m.id); ui.label(&m.version);
                        ui.label(if m.requires_rom { "User ROM required" } else { "Not required" });
                        let built = self.built.contains(&m.id);
                        ui.label(if built { "Built" } else { "Not built" }).on_hover_text(format!("Memory: {:.1} MiB recommended; {:.1} MiB hard limit", m.limits.recommended_memory_bytes() as f64 / 1048576.0, m.limits.hard_memory_bytes as f64 / 1048576.0));
                        ui.horizontal(|ui| {
                            if ui.add_enabled(self.result.is_none(),egui::Button::new("Details")).clicked() { inspect = Some(m.id.clone()); }
                            if ui.add_enabled(built && self.result.is_none() && (!m.requires_rom || !self.rom_path.trim().is_empty()), egui::Button::new("Play")).on_hover_text("Play this built game. ROM games need your file path above.").clicked() { play = Some(m.id.clone()); }
                            if ui.add_enabled(self.result.is_none(), egui::Button::new(if built { "Rebuild" } else { "Build" })).clicked() { build = Some(m.id.clone()); }
                        });
                        ui.vertical(|ui| {
                        ui.set_min_width(180.0);
                        if let Some(notice) = self.updates.notices.get(&m.id) {
                            match notice {
                                crate::updates::Notice::Available(version) => { ui.colored_label(egui::Color32::GREEN,format!("Update {version} available")); }
                                crate::updates::Notice::Current => { ui.label("Up to date"); }
                                crate::updates::Notice::Failed(error) => { ui.label("Automatic check failed").on_hover_text(error); }
                            }
                        }
                        if ui.add_enabled(self.result.is_none() && m.update_url.is_some(), egui::Button::new("Check for updates")).on_hover_text("Inspect the signed update without installing it").clicked() { update = Some(m.id.clone()); }
                        });
                        ui.end_row();
                    }
                });
            });
            if self.installed.is_empty() && self.catalog.is_empty() { ui.label("No games yet. Import a game list or install a package."); }
            if ui.add_enabled(self.result.is_none(), egui::Button::new("Refresh library")).clicked() { self.refresh(ctx); }
            if let Some(id) = inspect {
                self.pending = None;
                self.begin(move || {
                    let root = crate::registry::default_root()?;
                    crate::verify::verify_embedded(&mut crate::archive::Package::open(&root.join(format!("{id}.gamepkg")))?)
                });
            }
            if let Some(id) = play {
                let rom = self.installed.iter().find(|m| m.id == id).filter(|m| m.requires_rom).map(|_| PathBuf::from(self.rom_path.trim()));
                self.begin_work(move || Ok(Work::Game(Box::new(crate::runtime::Game::load_with_rom(&crate::registry::default_root()?, &id, rom.as_deref())?))));
            }
            if let Some(id) = download {
                self.pending = None; self.manifest = None;
                self.begin_work(move || {
                    let (m,file) = crate::catalog::fetch(&crate::registry::default_root()?, &id)?;
                    Ok(Work::Update(m,file))
                });
            }
            if let Some(id) = build {
                self.begin_work(move || { crate::build::compile(&crate::registry::default_root()?, &id)?; Ok(Work::Message(format!("Built {id}. Ready to play if this package has a playable entry point."))) });
            }
            if let Some(id) = update {
                self.pending = None; self.manifest = None;
                self.begin_work(move || match crate::registry::check_update(&crate::registry::default_root()?, &id)? {
                    Some((m,file)) => Ok(Work::Update(m,file)),
                    None => Ok(Work::Message(format!("{id} is up to date."))),
                });
            }
            ui.separator();
            ui.label("Local package, game list, or store bundle path; HTTPS or Tor onion URL");
            ui.text_edit_singleline(&mut self.source);
            ui.horizontal(|ui| {
            if ui.add_enabled(self.result.is_none(), egui::Button::new("Inspect .gamepkgstore bundle")).clicked() {
                self.pending_store = None;
                self.pending = None;
                self.manifest = None;
                self.preview = None;
                let source = self.source.trim().to_owned();
                self.begin_work(move || {
                    let downloaded;
                    let path = if source.contains("://") {
                        downloaded = crate::registry::download(&source)?;
                        downloaded.path()
                    } else { std::path::Path::new(&source) };
                    Ok(Work::Store(crate::store::Bundle::open(path)?))
                });
            }
            if ui.add_enabled(self.result.is_none(),egui::Button::new("Import .gamepkglist (no game downloads)")).clicked() {
                let source = self.source.trim().to_owned();
                self.begin_work(move || {
                    let downloaded;
                    let path = if source.contains("://") { downloaded = crate::registry::download(&source)?; downloaded.path() } else { std::path::Path::new(&source) };
                    let count = crate::catalog::import(&crate::registry::default_root()?, path)?;
                    Ok(Work::Message(format!("Added {count} games to the catalog. No game packages downloaded.")))
                });
            }
            });
            if ui.add_enabled(self.result.is_none(), egui::Button::new("Download / inspect")).clicked() {
                self.manifest = None;
                self.pending = None;
                let source = self.source.trim().to_owned();
                // A private staging file holds the verified download until approval.
                match tempfile::NamedTempFile::new() {
                    Ok(stage) => {
                        let path = stage.path().to_owned(); self.pending = Some(stage);
                        self.begin(move || {
                            if source.contains("://") {
                                let download = crate::registry::download(&source)?;
                                std::fs::copy(download.path(), &path)?;
                            } else {
                                use std::io::Read;
                                let mut input = std::fs::File::open(PathBuf::from(source))?.take(crate::archive::MAX_TOTAL + 1);
                                let mut output = std::fs::File::create(&path)?;
                                if std::io::copy(&mut input, &mut output)? > crate::archive::MAX_TOTAL { anyhow::bail!("package too large"); }
                            }
                            crate::verify::verify_embedded(&mut crate::archive::Package::open(&path)?)
                        });
                    }
                    Err(e) => self.status = format!("Unable to stage package: {e}"),
                }
            }
            ui.label(&self.status);
            if let Some(bundle) = &self.pending_store {
                ui.separator();
                ui.heading("Verified store bundle");
                ui.label("Signatures prove creator-key ownership. Confirm these creators before approving first installations. Existing games retain their creator keys.");
                egui::ScrollArea::vertical().id_salt("store-review").max_height(220.0).show(ui, |ui| {
                    for game in bundle.games() {
                        ui.label(format!("{} — {}{}", game.id, game.version, if game.requires_rom { " • User ROM required" } else { "" }));
                        ui.label(format!("Creator key: {}", game.creator_key));
                    }
                });
                let mut install = false;
                let mut cancel = false;
                ui.horizontal(|ui| {
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
            if let Some(m) = self.manifest.clone() {
                egui::ScrollArea::vertical().id_salt("game-details").max_height((ui.available_height() - 35.0).max(100.0)).show(ui, |ui| {
                ui.separator(); ui.heading(&m.id); ui.label(format!("Version: {}", m.version));
                ui.label(format!("Memory: {:.1} MiB recommended • {:.1} MiB hard limit", m.limits.recommended_memory_bytes() as f64 / 1048576.0, m.limits.hard_memory_bytes as f64 / 1048576.0));
                ui.label(if m.requires_rom { "Requires a ROM supplied by you. It is not included in this package." } else { "No user ROM required." });
                ui.label(format!("Creator fingerprint: {}", crate::verify::digest(&hex::decode(&m.creator_key).unwrap_or_default())));
                ui.label("A valid signature proves key ownership, not a real-world identity. Confirm the creator before first installation.");
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
            }
            ui.separator(); ui.label("Development preview. Supported games can play here; some package features are still in development.");
            ui.hyperlink_to("GamePKG • Apache 2.0", "https://gamepkg.com");
        });
    }
}
