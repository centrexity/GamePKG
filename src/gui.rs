use eframe::egui;
mod artwork;
mod drops;
mod game_page;
mod input;
mod library_view;
mod rom_picker;
mod sprites;
mod theme;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc,
};
enum Work {
    Launched(String, std::process::Child),
    Store(crate::store::Bundle),
    Preview(Vec<Vec<u8>>),
    Game(Box<crate::runtime::Game>),
    Package(crate::manifest::Manifest),
    Update(crate::manifest::Manifest, tempfile::NamedTempFile),
    RomPicked(rom_picker::Selection),
    Message(String),
}
#[derive(Default)]
pub struct App {
    gl: Option<std::sync::Arc<eframe::glow::Context>>,
    gpu: Option<std::sync::Arc<crate::gpu::Renderer>>,
    vulkan_state: Option<eframe::egui_wgpu::RenderState>,
    vulkan: Option<std::sync::Arc<crate::gpu_vulkan::Renderer>>,
    sprite_textures: sprites::Cache,
    applied_settings: Option<crate::settings::Settings>,
    updates: crate::updates::Monitor,
    source: String,
    drop_notice: Option<String>,
    selected_game: Option<String>,
    rom_paths: HashMap<String, Vec<String>>,
    rom_path: String,
    rom_game: Option<String>,
    rom_identity: Option<String>,
    additional_rom_paths: Vec<String>,
    show_settings: bool,
    show_support: bool,
    support_copied: Option<String>,
    brand_icon: Option<egui::TextureHandle>,
    settings_draft: crate::settings::Settings,
    player: Option<Box<crate::runtime::Game>>,
    game_window: bool,
    game_processes: Vec<(String, std::process::Child)>,
    next_frame: Option<std::time::Instant>,
    pending_input: u32,
    input_devices: input::Devices,
    audio_player: Option<crate::audio::Player>,
    audio_error: String,
    status: String,
    result: Option<mpsc::Receiver<Result<Work, String>>>,
    manifest: Option<crate::manifest::Manifest>,
    pending: Option<tempfile::NamedTempFile>,
    pending_store: Option<crate::store::Bundle>,
    installed: Vec<crate::manifest::Manifest>,
    rejected: Vec<(String, String)>,
    catalog: Vec<crate::catalog::Entry>,
    loaded: bool,
    icons: HashMap<String, egui::TextureHandle>,
    package_notices: HashMap<String, String>,
    built: HashSet<String>,
    wasm_built: HashSet<String>,
    history: HashMap<String, crate::progress::Record>,
    usage: HashMap<String, crate::storage::Usage>,
    library_bytes: Option<u64>,
    preview: Option<(Vec<Vec<u8>>, std::time::Instant, egui::TextureHandle)>,
}
impl App {
    pub fn set_vulkan(&mut self, state: Option<eframe::egui_wgpu::RenderState>) {
        self.vulkan_state = state;
    }
    pub fn open_support(&mut self) {
        self.show_support = true;
        self.show_settings = false;
        self.preview = None;
        self.support_copied = None;
    }
    pub fn new(play: Option<String>, gl: Option<std::sync::Arc<eframe::glow::Context>>) -> Self {
        Self::new_with_target(play, gl, None)
    }
    pub fn new_with_target(
        play: Option<String>,
        gl: Option<std::sync::Arc<eframe::glow::Context>>,
        target: Option<bool>,
    ) -> Self {
        let mut app = Self {
            game_window: play.is_some(),
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
                let root = crate::registry::default_root()?;
                let manifest = crate::registry::installed_manifest(&root, &id)?;
                let paths = crate::rom_selections::load(&root, &manifest)?
                    .into_iter()
                    .map(PathBuf::from)
                    .collect::<Vec<_>>();
                Ok(Work::Game(Box::new(crate::runtime::Game::load_target(
                    &root, &id, &paths, target,
                )?)))
            });
        }
        app
    }
    fn sync_rom_selection(&mut self, root: &std::path::Path) {
        let selected = self
            .installed
            .iter()
            .find(|m| m.requires_rom && Some(&m.id) == self.rom_game.as_ref());
        let identity = selected.and_then(|m| rom_identity(root, m));
        if identity != self.rom_identity {
            let paths = selected
                .map(|m| crate::rom_selections::load(root, m))
                .transpose();
            match paths {
                Ok(paths) => {
                    let mut paths = paths.unwrap_or_default().into_iter();
                    self.rom_path = paths.next().unwrap_or_default();
                    self.additional_rom_paths = paths.collect();
                }
                Err(error) => {
                    self.rom_path.clear();
                    self.additional_rom_paths =
                        vec![
                            String::new();
                            selected.map_or(0, |m| m.required_rom_count().saturating_sub(1))
                        ];
                    self.status = format!("Unable to load remembered ROM files: {error:#}");
                }
            }
            self.rom_identity = identity;
        }
        if selected.is_none() {
            self.rom_game = None;
            self.rom_path.clear();
            self.additional_rom_paths.clear();
        }
    }
    fn refresh(&mut self, ctx: &egui::Context) {
        match crate::registry::default_root().and_then(|r| crate::catalog::entries(&r)) {
            Ok(entries) => self.catalog = entries,
            Err(e) => self.status = format!("Unable to read catalog: {e:#}"),
        }
        match crate::registry::default_root()
            .and_then(|root| crate::registry::inspect_library(&root))
        {
            Ok(report) => {
                self.rejected = report.rejected;
                let games = report.games;
                self.icons.clear();
                self.package_notices.clear();
                self.built.clear();
                self.wasm_built.clear();
                if let Ok(root) = crate::registry::default_root() {
                    self.rom_paths.clear();
                    self.history.clear();
                    self.usage.clear();
                    self.library_bytes = crate::storage::total(&root).ok();
                    for m in &games {
                        match crate::progress::load(&root, &m.id, &m.creator_key) {
                            Ok(record) => {
                                self.history.insert(m.id.clone(), record);
                            }
                            Err(e) => self.status = format!("Unable to read play history: {e}"),
                        }
                        if let Ok(usage) = crate::storage::usage(&root, m) {
                            self.usage.insert(m.id.clone(), usage);
                        }
                        if m.requires_rom {
                            match crate::rom_selections::load(&root, m) {
                                Ok(paths) => {
                                    self.rom_paths.insert(m.id.clone(), paths);
                                }
                                Err(error) => {
                                    self.status =
                                        format!("Unable to read remembered ROM files: {error:#}")
                                }
                            }
                        }
                        if crate::build::is_built(&root, &m.id).unwrap_or(false) {
                            self.wasm_built.insert(m.id.clone());
                        }
                        if crate::native::is_built(&root, &m.id).unwrap_or(false) {
                            self.built.insert(m.id.clone());
                        }
                        let decoded = (|| -> anyhow::Result<_> {
                            let mut pkg = crate::archive::Package::open(
                                &root.join(format!("{}.gamepkg", m.id)),
                            )?;
                            if let Ok(bytes) = pkg.read("NOTICE") {
                                if bytes.len() <= 65536 {
                                    if let Ok(text) = String::from_utf8(bytes) {
                                        self.package_notices.insert(m.id.clone(), text);
                                    }
                                }
                            }
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
                            anyhow::ensure!(
                                icon.width() >= 16 && icon.height() >= 16,
                                "placeholder thumbnail"
                            );
                            Ok(egui::ColorImage::from_rgba_unmultiplied(
                                [icon.width() as usize, icon.height() as usize],
                                icon.as_raw(),
                            ))
                        })();
                        let decoded = decoded.or_else(|error| -> anyhow::Result<_> {
                            let (bytes, notice) = artwork::fallback(&m.id).ok_or(error)?;
                            let icon = image::load_from_memory(bytes)?.to_rgba8();
                            self.package_notices
                                .entry(m.id.clone())
                                .or_default()
                                .push_str(&format!(
                                    "\n\nLibrary fallback artwork, provided by GamePKG:\n{notice}"
                                ));
                            Ok(egui::ColorImage::from_rgba_unmultiplied(
                                [icon.width() as usize, icon.height() as usize],
                                icon.as_raw(),
                            ))
                        });
                        if let Ok(icon) = decoded {
                            self.icons.insert(
                                m.id.clone(),
                                ctx.load_texture(&m.id, icon, egui::TextureOptions::LINEAR),
                            );
                        }
                    }
                }
                self.installed = games;
                if let Ok(root) = crate::registry::default_root() {
                    self.sync_rom_selection(&root);
                }
            }
            Err(e) => {
                self.installed.clear();
                self.rom_identity = None;
                self.rom_game = None;
                self.rom_path.clear();
                self.additional_rom_paths.clear();
                self.history.clear();
                self.usage.clear();
                self.library_bytes = None;
                self.rejected.clear();
                self.icons.clear();
                self.package_notices.clear();
                self.built.clear();
                self.wasm_built.clear();
                self.status = format!("Unable to read installed library: {e:#}");
            }
        }
    }
    fn inspect_package(&mut self, source: String, local: Option<PathBuf>) {
        self.manifest = None;
        self.pending = None;
        self.preview = None;
        match tempfile::NamedTempFile::new() {
            Ok(stage) => {
                let path = stage.path().to_owned();
                self.pending = Some(stage);
                self.begin(move || {
                    let bytes = if let Some(local) = local {
                        crate::files::read_regular(&local, crate::archive::MAX_TOTAL, true)?
                    } else {
                        let download = crate::registry::download(&source)?;
                        crate::files::read_regular(
                            download.path(),
                            crate::archive::MAX_TOTAL,
                            false,
                        )?
                    };
                    std::fs::write(&path, bytes)?;
                    crate::verify::verify_embedded(&mut crate::archive::Package::open(&path)?)
                });
            }
            Err(e) => self.status = format!("Unable to stage package: {e}"),
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
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Some(game) = self.player.as_mut() {
            let _ = game.finish();
        }
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let mut exited = false;
        self.game_processes
            .retain_mut(|(_, child)| match child.try_wait() {
                Ok(Some(_)) => {
                    exited = true;
                    false
                }
                _ => true,
            });
        if exited {
            self.refresh(ctx);
        }
        if !self.game_processes.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        if !self.loaded {
            self.loaded = true;
            theme::apply(
                ctx,
                self.settings_draft.dark_mode,
                self.settings_draft.ui_scale_percent,
            );
            self.refresh(ctx);
        }
        if let Some(rx) = &self.result {
            match rx.try_recv() {
                Ok(result) => {
                    self.result = None;
                    match result {
                        Ok(Work::Launched(id, child)) => {
                            self.status = format!("Opened {id} in its own window.");
                            self.game_processes.push((id, child));
                        }
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
                            self.vulkan = None;
                            if !game.shader_sources.is_empty() && self.vulkan_state.is_some() {
                                match crate::gpu_vulkan::Renderer::new(
                                    self.vulkan_state.as_ref().unwrap(),
                                    &game.shader_sources,
                                ) {
                                    Ok(renderer) => {
                                        self.vulkan = Some(std::sync::Arc::new(renderer))
                                    }
                                    Err(error) => {
                                        self.status = format!("Unable to play: {error:#}");
                                        return;
                                    }
                                }
                            }
                            if !game.shader_sources.is_empty() && self.vulkan_state.is_none() {
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
                            self.selected_game = Some(game.id.clone());
                            self.player = Some(game);
                        }
                        Ok(Work::Package(m)) => {
                            self.preview = None;
                            self.status = format!("Verified {} {}", m.id, m.version);
                            self.selected_game = Some(m.id.clone());
                            self.manifest = Some(m);
                            self.refresh(ctx);
                        }
                        Ok(Work::Update(m, file)) => {
                            self.preview = None;
                            self.status = format!(
                                "Package ready: {} {}. Review and install below.",
                                m.id, m.version
                            );
                            self.selected_game = Some(m.id.clone());
                            self.manifest = Some(m);
                            self.pending = Some(file);
                        }
                        Ok(Work::RomPicked(selection)) => {
                            self.status = match self.apply_rom_pick(selection) {
                                Ok(true) => {
                                    "ROM file remembered. The original stays on your drive.".into()
                                }
                                Ok(false) => "ROM selection cancelled. Previous file kept.".into(),
                                Err(error) => format!("Unable to remember ROM file: {error:#}"),
                            };
                        }
                        Ok(Work::Message(message)) => {
                            self.status = message;
                            self.refresh(ctx);
                        }
                        Err(e) => {
                            self.status = format!("Failed: {e}");
                            self.manifest = None;
                            self.pending = None;
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
        self.handle_drops(ctx);
        if let Some(notice) = self.drop_notice.clone() {
            egui::Window::new("File drop")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(notice);
                    if ui.button("OK").clicked() {
                        self.drop_notice = None;
                    }
                });
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
        if self.player.is_none() {
            self.input_devices.reset();
            self.sprite_textures.finish(0);
        }
        if let Some(game) = self.player.as_mut() {
            let mut image_count = 0;
            let mut failure = None;
            let select_input = ctx.input(|i| {
                u32::from(i.key_pressed(egui::Key::Tab)) << 7
                    | u32::from(i.key_down(egui::Key::Tab)) << 15
            });
            if game.builtin_engine {
                ctx.input_mut(|i| {
                    i.events.retain(|event| {
                        !matches!(
                            event,
                            egui::Event::Key {
                                key: egui::Key::Tab,
                                ..
                            }
                        )
                    })
                });
                if let Some(id) = ctx.memory(|m| m.focused()) {
                    ctx.memory_mut(|m| m.surrender_focus(id));
                }
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
                .show(ctx, |ui| {
                    let input = ctx.input(|i| {
                        (u32::from(i.key_down(egui::Key::ArrowLeft) || i.key_down(egui::Key::A))
                            << 8)
                            | (u32::from(
                                i.key_down(egui::Key::ArrowRight) || i.key_down(egui::Key::D),
                            ) << 9)
                            | (u32::from(
                                i.key_down(egui::Key::ArrowUp) || i.key_down(egui::Key::W),
                            ) << 10)
                            | (u32::from(
                                i.key_down(egui::Key::ArrowDown) || i.key_down(egui::Key::S),
                            ) << 11)
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
                    let input = input
                        | select_input
                        | ctx.input(|i| {
                            let mut buttons = 0u32;
                            for (key, press, held) in [
                                (egui::Key::Z, 4, 12),
                                (egui::Key::X, 5, 13),
                                (egui::Key::Enter, 6, 14),
                                (egui::Key::Tab, 7, 15),
                                (egui::Key::C, 16, 20),
                                (egui::Key::V, 17, 21),
                                (egui::Key::Q, 18, 22),
                                (egui::Key::E, 19, 23),
                            ] {
                                buttons |= u32::from(i.key_pressed(key)) << press;
                                buttons |= u32::from(i.key_down(key)) << held;
                            }
                            buttons
                        });
                    let device_input = self.input_devices.poll(ui);
                    let input = if ctx.input(|i| i.focused) {
                        input | device_input
                    } else {
                        self.pending_input = 0;
                        0
                    };
                    self.pending_input |= input & 0xf00ff;
                    let now = std::time::Instant::now();
                    if self.next_frame.is_none_or(|next| now >= next) {
                        match game.step(self.pending_input | (input & 0xf0ff00)) {
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
                    if game.custom_renderer {
                        let scale = (ui.available_width() / 640.0)
                            .min(ui.available_height() / 480.0)
                            .max(0.0);
                        let rect = egui::Rect::from_center_size(
                            ui.max_rect().center(),
                            egui::vec2(640.0 * scale, 480.0 * scale),
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
                                crate::drawing::Command::Image {
                                    x,
                                    y,
                                    width,
                                    height,
                                    ref image,
                                } => {
                                    let texture =
                                        self.sprite_textures.texture(ctx, image_count, image);
                                    image_count += 1;
                                    painter.image(
                                        texture,
                                        egui::Rect::from_min_size(
                                            rect.min + egui::vec2(x * scale, y * scale),
                                            egui::vec2(width * scale, height * scale),
                                        ),
                                        egui::Rect::from_min_max(
                                            egui::Pos2::ZERO,
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        egui::Color32::WHITE,
                                    );
                                }
                                crate::drawing::Command::ShaderImage { index, ref image } => {
                                    if let Some(renderer) = &self.vulkan {
                                        if let Some(callback) =
                                            renderer.callback(rect, index, Some(image))
                                        {
                                            painter.add(callback);
                                        }
                                    }
                                    if let Some(renderer) = &self.gpu {
                                        let renderer = renderer.clone();
                                        let image = image.clone();
                                        painter.add(egui::PaintCallback {
                                            rect,
                                            callback: std::sync::Arc::new(
                                                eframe::egui_glow::CallbackFn::new(move |_, _| {
                                                    renderer.paint_image(index, Some(&image))
                                                }),
                                            ),
                                        });
                                    }
                                }
                                crate::drawing::Command::Shader { index } => {
                                    if let Some(renderer) = &self.vulkan {
                                        if let Some(callback) = renderer.callback(rect, index, None)
                                        {
                                            painter.add(callback);
                                        }
                                    }
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
                    let size = ui.available_width().min(ui.available_height()).max(0.0);
                    let rect = egui::Rect::from_center_size(
                        ui.max_rect().center(),
                        egui::vec2(size, size),
                    );
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
                    painter.circle_filled(
                        position(game.target()),
                        cell * 0.28,
                        egui::Color32::YELLOW,
                    );
                    painter.circle_filled(
                        position(game.player()),
                        cell * 0.35,
                        egui::Color32::from_rgb(100, 220, 180),
                    );
                });
            self.sprite_textures.finish(image_count);
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
            if failure.is_some() {
                if let Some(game) = &mut self.player {
                    if let Err(e) = game.finish() {
                        self.status = format!("Unable to save play history: {e}");
                    }
                }
                if self.game_window && failure.is_none() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                self.player = None;
                self.loaded = false;
                self.gpu = None;
                self.vulkan = None;
                self.audio_player = None;
            }
            if let Some(e) = failure {
                self.status = e;
            }
            return;
        }
        if self.game_window {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
                .show(ctx, |_| {});
            if self.result.is_none() {
                rfd::MessageDialog::new()
                    .set_title("Unable to play game")
                    .set_description(&self.status)
                    .set_level(rfd::MessageLevel::Error)
                    .show();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
        if self.brand_icon.is_none() {
            let icon = window_icon();
            self.brand_icon = Some(ctx.load_texture(
                "gamepkg-brand",
                egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                ),
                egui::TextureOptions::LINEAR,
            ));
        }
        egui::TopBottomPanel::top("navigation").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if let Some(icon) = &self.brand_icon {
                    ui.image((icon.id(), egui::vec2(38.0, 38.0)));
                }
                ui.label(egui::RichText::new("GamePKG").size(23.0).strong());
                ui.add_space(16.0);
                if ui
                    .selectable_label(!self.show_settings && !self.show_support, "Library")
                    .clicked()
                {
                    self.show_settings = false;
                    self.show_support = false;
                    self.close_game_page();
                    self.preview = None;
                }
                if ui
                    .add_enabled(self.result.is_none(), egui::Button::new("Settings"))
                    .clicked()
                {
                    self.show_settings = true;
                    self.show_support = false;
                    self.preview = None;
                    match crate::settings::load() {
                        Ok(settings) => self.settings_draft = settings,
                        Err(e) => self.status = format!("Unable to load settings: {e:#}"),
                    }
                }
                if ui.selectable_label(self.show_support, "Support").clicked() {
                    self.open_support();
                }
            });
        });
        if self.show_support {
            if self.brand_icon.is_none() {
                let icon = window_icon();
                self.brand_icon = Some(ctx.load_texture(
                    "gamepkg-brand",
                    egui::ColorImage::from_rgba_unmultiplied(
                        [icon.width as usize, icon.height as usize],
                        &icon.rgba,
                    ),
                    egui::TextureOptions::LINEAR,
                ));
            }
            let support = crate::support::details();
            egui::CentralPanel::default().frame(egui::Frame::central_panel(&ctx.style()).inner_margin(20)).show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if let Some(icon) = &self.brand_icon {
                            ui.image((icon.id(), egui::vec2(88.0, 88.0)));
                        }
                        ui.vertical(|ui| {
                            ui.heading("Support GamePKG");
                            ui.label("Help build a safer way to share and play games.");
                        });
                    });
                    ui.add_space(20.0);
                    ui.label("Your support helps maintain the host, improve protections, and make GamePKG easier to use.");
                    ui.add_space(16.0);
                    ui.heading("PayPal");
                    if support.paypal_url.is_empty() {
                        ui.add_enabled(false, egui::Button::new("Support with PayPal"));
                        ui.weak("Details coming soon.");
                    } else {
                        ui.hyperlink_to("Support with PayPal", &support.paypal_url);
                    }
                    ui.add_space(16.0);
                    ui.heading("Crypto");
                    ui.label("Click an address to copy it.");
                    for (name, address) in [("Bitcoin", &support.bitcoin), ("Monero", &support.monero), ("Dogecoin", &support.dogecoin)] {
                        ui.group(|ui| {
                            ui.label(name);
                            if address.is_empty() {
                                ui.add_enabled(false, egui::Button::new("Copy address"));
                                ui.weak("Address coming soon.");
                            } else if ui.button(egui::RichText::new(address).monospace()).on_hover_text("Copy address").clicked() {
                                ctx.copy_text(address.clone());
                                self.support_copied = Some(format!("{name} address copied."));
                            }
                        });
                    }
                    if let Some(message) = &self.support_copied { ui.label(message); }
                    ui.add_space(20.0);
                    ui.heading("Mine with me");
                    ui.label("Join the GamePKG group on Simoleons.");
                    ui.hyperlink_to("Mine with me", &support.mining_url);
                });
            });
            return;
        }
        if self.show_settings {
            egui::CentralPanel::default().frame(egui::Frame::central_panel(&ctx.style()).inner_margin(20)).show(ctx,|ui| {
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
                    ui.horizontal(|ui| { ui.label("Tick deadline (ms)");ui.add(egui::Slider::new(&mut self.settings_draft.tick_deadline_ms,10..=100).step_by(10.0)); });
                    ui.label("Memory uses the lower of your ceiling and the signed game limit. These settings cannot raise a game's hard limit. Changes apply the next time you launch a game.");
                    ui.separator();ui.heading("Builds and appearance");
                    ui.add(egui::Slider::new(&mut self.settings_draft.ui_scale_percent,80..=160).step_by(10.0).text("Interface size (%)"));
                    ui.label("Larger text and controls apply when you save. Display scaling also follows your system settings.");
                    #[cfg(windows)]
                    {
                        ui.heading("Windows compiler");
                        ui.label("WSL 2 distribution name (leave both fields empty to disable)");
                        ui.text_edit_singleline(&mut self.settings_draft.windows_compiler_distribution);
                        ui.label("GamePKG compiler executable inside Linux");
                        ui.text_edit_singleline(&mut self.settings_draft.windows_compiler_executable);
                        ui.label("Use a configured Linux compiler with the same GamePKG version. Games still run in this Windows app after their build is verified.");
                    }
                    #[cfg(target_os = "linux")]
                    {
                        ui.label("C/WASI build tools folder (optional; empty uses system tools)");
                        ui.text_edit_singleline(&mut self.settings_draft.wasi_sysroot_directory);
                        ui.label("Native source ports use Clang 18 and lld-18. The separate Wasm build also needs wasi-libc and libclang-rt-18-dev-wasm32; set this folder only for a separate WASI installation.");
                        if ui.button("Check C/WASI build tools").clicked() {
                            self.status = match crate::c_toolchain::check_draft(&self.settings_draft.wasi_sysroot_directory) {
                                Ok(root) => format!("C/WASI build tools ready: {}. Save settings to use this folder.", root.display()),
                                Err(error) => format!("C/WASI build tools: {error:#}"),
                            };
                        }
                    }
                    egui::ComboBox::from_label("Renderer (restart app after changing)").selected_text(&self.settings_draft.renderer).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.settings_draft.renderer,"vulkan".into(),"Vulkan");
                        ui.selectable_value(&mut self.settings_draft.renderer,"opengl".into(),"OpenGL");
                    });
                    ui.label("Slang 2026.19 installation folder");
                    ui.text_edit_singleline(&mut self.settings_draft.slang_directory);
                    egui::ComboBox::from_label("Shader backend").selected_text(&self.settings_draft.shader_backend).show_ui(ui, |ui| {
                        for backend in ["spirv", "wgsl", "metal", "dxil", "glsl"] { ui.selectable_value(&mut self.settings_draft.shader_backend, backend.to_string(), backend); }
                    });
                    ui.label("Vulkan uses WGSL or SPIR-V for bounded texture fragment passes. OpenGL also supports GLSL. Other backends compile and cache shaders.");
                    if self.settings_draft.shader_backend == "dxil" {
                        ui.label("DXC 1.9.2609 installation folder");
                        ui.text_edit_singleline(&mut self.settings_draft.dxc_directory);
                        ui.label("Choose the pinned DXC installation. GamePKG verifies its compiler libraries before each build.");
                        if std::env::var_os("GAMEPKG_DXC_ROOT").is_some() { ui.label("GAMEPKG_DXC_ROOT overrides this folder for the current launch."); }
                    }
                    ui.separator();
                    ui.heading("Game source policy");
                    ui.label("Game packages cannot use Cargo dependencies. The host supplies the gamepkg API for input, files, drawing, shaders and audio.");
                    ui.separator();
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
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(validation.is_ok(), egui::Button::new("Save settings")).clicked() {
                            let result=crate::settings::path().and_then(|path|self.settings_draft.save(&path));
                            match result {
                                Ok(()) => {
                                    theme::apply(ctx, self.settings_draft.dark_mode, self.settings_draft.ui_scale_percent);
                                    self.applied_settings=Some(self.settings_draft.clone());
                                    self.pending=None;self.pending_store=None;self.manifest=None;self.installed.clear();self.rejected.clear();self.catalog.clear();self.status="Settings saved.".into();self.refresh(ctx);
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
        if self.selected_game.is_some() {
            self.show_game_page(ctx);
            return;
        }
        egui::CentralPanel::default().frame(egui::Frame::central_panel(&ctx.style()).inner_margin(20)).show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("library-page").show(ui, |ui| {
            ui.heading("Your games");
            ui.label("Choose a game to build, play or explore.");
            ui.horizontal_wrapped(|ui| {
                for (label,value) in [("Installed",self.installed.len().to_string()),("Ready to play",self.built.len().to_string()),("Storage used",self.library_bytes.map(crate::storage::bytes).unwrap_or_else(||"—".into()))] {
                    egui::Frame::group(ui.style()).inner_margin(14).corner_radius(10).show(ui,|ui| {
                        ui.vertical(|ui| {ui.label(egui::RichText::new(value).size(24.0).strong().color(ui.visuals().hyperlink_color));ui.label(label);});
                    });
                }
            });
            ui.add_space(12.0);
            let mut inspect = None;
            let mut integrity = None;
            egui::ScrollArea::vertical().id_salt("library-games").max_height((ctx.available_rect().height()*0.45).clamp(160.0,440.0)).show(ui, |ui| {
                inspect = self.library_games(ui);
            });
            for (name, error) in &self.rejected {
                ui.colored_label(egui::Color32::from_rgb(240, 130, 100), format!("Unavailable package: {name}"));
                ui.label(error);
                if ui.add_enabled(self.result.is_none(), egui::Button::new("Restore verified copy / rebuild")).clicked() { if let Some(id)=name.strip_suffix(".gamepkg") { integrity=Some((id.to_owned(),true,None)); } }
                if ui.add_enabled(self.result.is_none() && self.source.trim().ends_with(".gamepkgstore") && std::path::Path::new(self.source.trim()).is_file(),egui::Button::new("Repair assets")).on_hover_text("Use the retained verified package and the original local store bundle entered below.").clicked() {if let Some(id)=name.strip_suffix(".gamepkg") {integrity=Some((id.to_owned(),true,Some(PathBuf::from(self.source.trim()))));}}
            }
            if self.installed.is_empty() && self.catalog.is_empty() && self.rejected.is_empty() { ui.label("No games yet. Import a game list or install a package."); }
            if ui.add_enabled(self.result.is_none(), egui::Button::new("Refresh library")).clicked() { self.refresh(ctx); }
            if let Some((id,fix,store)) = integrity {
                self.begin_work(move || { let root=crate::registry::default_root()?; Ok(Work::Message(if fix {crate::integrity::fix_with_store(&root,&id,store.as_deref())?} else {crate::integrity::check(&root,&id)?})) });
            }
            if let Some(id)=inspect {self.open_game_page(&id);}
            ui.label(format!("Library storage used: {} (file bytes, including shared assets, caches, saves and play history)", self.library_bytes.map(crate::storage::bytes).unwrap_or_else(||"Unavailable".into())));
            ui.separator();
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
                egui::ScrollArea::vertical().id_salt("store-review").max_height(220.0).show(ui, |ui| {
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
            ui.separator(); ui.label(concat!("GamePKG ", env!("CARGO_PKG_VERSION")));
            ui.hyperlink_to("GamePKG • Apache 2.0", "https://gamepkg.com");
            });
        });
    }
}

pub fn window_icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../html/assets/gamepkg-icon.png"))
        .expect("bundled GamePKG icon must be a valid PNG")
        .thumbnail(256, 256)
        .into_rgba8();
    egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

// Bind remembered files to the library, creator, game and ROM requirements.
fn rom_identity(root: &std::path::Path, manifest: &crate::manifest::Manifest) -> Option<String> {
    let root = std::fs::canonicalize(root).ok()?;
    let data = serde_json::to_vec(&(root, crate::rom_selections::identity(manifest).ok()?)).ok()?;
    Some(crate::verify::digest(&data))
}

#[cfg(test)]
mod rom_selection_tests {
    use super::*;
    #[test]
    fn scope_survives_version_updates_but_changes_for_creators_and_libraries() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let mut m: crate::manifest::Manifest =
            toml::from_str(include_str!("../ports/doom/manifest.toml")).unwrap();
        let first = rom_identity(a.path(), &m);
        m.version = "99.0.0".into();
        assert_eq!(first, rom_identity(a.path(), &m));
        assert_ne!(first, rom_identity(b.path(), &m));
        m.creator_key = "changed".into();
        assert_ne!(first, rom_identity(a.path(), &m));
    }
}
