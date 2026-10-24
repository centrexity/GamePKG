use eframe::egui;
mod add_page;
mod app_updates;
mod artwork;
mod cube2_input;
mod display;
mod doom_input;
mod drops;
mod game_colors;
mod game_page;
mod input;
mod library_view;
mod linear_sprites;
mod multiplayer;
mod processes;
mod recordings;
mod rom_picker;
mod snapshot_slots;
mod source_input;
mod sprites;
mod theme;
mod user_files;
mod window_control;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc,
};
enum Work {
    DataPicked(PathBuf, crate::manifest::Manifest, Option<PathBuf>),
    Launched(
        String,
        std::process::Child,
        Option<(String, crate::server_selections::Selection)>,
    ),
    NetworkSaved(String, crate::server_selections::Selection),
    VideoSaved(String, Option<crate::manifest::ZeldaVideo>),
    Recordings(recordings::Loaded),
    RecordingExported(String),
    Store(crate::store::Bundle),
    Preview(Vec<Vec<u8>>),
    Game(Box<crate::runtime::Game>),
    Package(crate::manifest::Manifest),
    Update(crate::manifest::Manifest, tempfile::NamedTempFile),
    RomPicked(rom_picker::Selection),
    Message(String),
    NativeBuilt(String),
    AppRelease(Option<crate::app_updates::Release>),
    AppInstalled(PathBuf),
}
#[derive(Default)]
pub struct App {
    gl: Option<std::sync::Arc<eframe::glow::Context>>,
    gpu: Option<std::sync::Arc<crate::gpu::Renderer>>,
    vulkan_state: Option<eframe::egui_wgpu::RenderState>,
    vulkan: Option<std::sync::Arc<crate::gpu_vulkan::Renderer>>,
    sprite_textures: sprites::Cache,
    linear_sprites: linear_sprites::Cache,
    applied_settings: Option<crate::settings::Settings>,
    updates: crate::updates::Monitor,
    source: String,
    drop_notice: Option<String>,
    selected_game: Option<String>,
    steam_custom_root: Option<PathBuf>,
    rom_paths: HashMap<String, Vec<String>>,
    data_paths: HashMap<String, String>,
    network_drafts: HashMap<String, multiplayer::Draft>,
    video_drafts: HashMap<String, display::Draft>,
    recordings: Option<recordings::Loaded>,
    rom_path: String,
    rom_game: Option<String>,
    rom_identity: Option<String>,
    additional_rom_paths: Vec<String>,
    library_filter: library_view::Filter,
    library_search: String,
    library_pagination: library_view::Pagination,
    show_add: bool,
    show_settings: bool,
    focus_memory_settings: bool,
    focus_tick_settings: bool,
    app_release: Option<crate::app_updates::Release>,
    app_installed: Option<PathBuf>,
    startup_app_check: Option<mpsc::Receiver<Result<Option<crate::app_updates::Release>, String>>>,
    app_update_notice: bool,
    app_update_error: Option<String>,
    show_support: bool,
    support_copied: Option<String>,
    brand_icon: Option<egui::TextureHandle>,
    settings_draft: crate::settings::Settings,
    player: Option<Box<crate::runtime::Game>>,
    user_files: user_files::Picker,
    game_window: bool,
    game_window_closing: bool,
    window_preferences: Option<crate::window_preferences::Session>,
    startup_vsync: Option<bool>,
    window_preferences_active: bool,
    pending_window_maximize: bool,
    cube_mouse_capture: cube2_input::MouseCapture,
    confine_mouse: bool,
    source_input: source_input::Capture,
    source_window: window_control::Controller,
    cube_desktop: cube2_input::Desktop,
    doom_desktop: doom_input::Desktop,
    snapshot_desktop: snapshot_slots::Desktop,
    game_processes: Vec<(String, std::process::Child)>,
    next_frame: Option<std::time::Instant>,
    pending_input: u32,
    input_devices: input::Devices,
    keyboard: input::Keyboard,
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
    catalog_sources: Vec<crate::catalog::sources::List>,
    remove_list: Option<String>,
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
    pub fn set_window_preferences(&mut self, session: Option<crate::window_preferences::Session>) {
        self.startup_vsync = session
            .as_ref()
            .filter(|_| self.game_window)
            .map(|session| session.preferences.vsync);
        self.pending_window_maximize = session
            .as_ref()
            .is_some_and(|session| session.preferences.maximized);
        self.window_preferences = session;
    }
    fn observe_window_preferences(&mut self, ctx: &egui::Context, restoring_maximize: bool) {
        let requested = self
            .player
            .as_ref()
            .map(|game| game.window_state())
            .unwrap_or_default();
        let settled = !self.source_window.resize_pending(ctx, requested);
        if self.window_preferences_active {
            if let (Some(vsync), Some(session)) =
                (requested.vsync, self.window_preferences.as_mut())
            {
                session.preferences.vsync = vsync;
            }
        }
        let confirmation = if settled {
            self.source_window.take_confirmation()
        } else {
            None
        };
        let actual_fullscreen = ctx.input(|input| input.viewport().fullscreen);
        if let (Some((normal, maximized)), Some(session)) = (
            confirmation.or_else(|| self.source_window.preview_preferences()),
            self.window_preferences.as_mut(),
        ) {
            let native_scale = ctx.pixels_per_point() / ctx.zoom_factor();
            let width = (normal[0] as f32 / native_scale).round();
            let height = (normal[1] as f32 / native_scale).round();
            if (320.0..=10000.0).contains(&width) && (240.0..=10000.0).contains(&height) {
                session.preferences.width = width as u32;
                session.preferences.height = height as u32;
                session.preferences.maximized = maximized;
                if let Some(fullscreen) = actual_fullscreen {
                    session.preferences.fullscreen = fullscreen;
                }
            }
        }
        if self.window_preferences_active
            && settled
            && !self.game_window_closing
            && requested.resize_origin.is_none()
        {
            if let Some(session) = &mut self.window_preferences {
                let zoom = ctx.zoom_factor();
                ctx.input(|input| {
                    let viewport = input.viewport();
                    session.preferences.observe(
                        if restoring_maximize {
                            None
                        } else {
                            viewport
                                .inner_rect
                                .map(|rect| [rect.width() * zoom, rect.height() * zoom])
                        },
                        viewport.fullscreen,
                        if restoring_maximize {
                            None
                        } else {
                            viewport.maximized
                        },
                        viewport.minimized.unwrap_or(false),
                    );
                });
            }
        }
    }
    fn persist_window_preferences(&mut self) {
        if self.window_preferences_active {
            if let Some(session) = &mut self.window_preferences {
                if let Err(error) = session.persist() {
                    eprintln!("Unable to save game window settings: {error:#}");
                }
            }
        }
    }
    pub fn set_vulkan(&mut self, state: Option<eframe::egui_wgpu::RenderState>) {
        self.vulkan_state = state;
    }
    pub fn open_support(&mut self) {
        self.show_add = false;
        self.show_support = true;
        self.show_settings = false;
        self.preview = None;
        self.support_copied = None;
    }
    pub fn set_native_display(&mut self, cc: &eframe::CreationContext<'_>) {
        use eframe::egui_wgpu::wgpu::rwh::{HasDisplayHandle, RawDisplayHandle};
        self.confine_mouse = cc.display_handle().is_ok_and(|handle| {
            matches!(
                handle.as_raw(),
                RawDisplayHandle::Xlib(_) | RawDisplayHandle::Xcb(_) | RawDisplayHandle::Windows(_)
            )
        });
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
        app.start_app_update_check();
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
        self.recordings = None;
        match crate::registry::default_root().and_then(|root| crate::catalog::sources::lists(&root))
        {
            Ok(lists) => self.catalog_sources = lists,
            Err(error) => {
                self.catalog_sources.clear();
                self.status = format!("Unable to read saved lists: {error:#}");
            }
        }
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
                let scopes: HashSet<String> = games.iter().filter_map(multiplayer::scope).collect();
                self.network_drafts.retain(|key, _| scopes.contains(key));
                let video_scopes: HashSet<String> = games
                    .iter()
                    .filter_map(|m| crate::video_selections::identity(m).ok())
                    .collect();
                self.video_drafts
                    .retain(|key, _| video_scopes.contains(key));
                self.icons.clear();
                self.package_notices.clear();
                self.built.clear();
                self.wasm_built.clear();
                if let Ok(root) = crate::registry::default_root() {
                    self.rom_paths.clear();
                    self.data_paths.clear();
                    self.history.clear();
                    self.usage.clear();
                    self.library_bytes = crate::storage::total(&root).ok();
                    for m in &games {
                        if let Ok(scope) = crate::video_selections::identity(m) {
                            match crate::video_selections::load(&root, m) {
                                Ok(selection) => self
                                    .video_drafts
                                    .entry(scope)
                                    .or_default()
                                    .loaded(selection),
                                Err(error) => {
                                    self.status =
                                        format!("Unable to read display settings: {error:#}")
                                }
                            }
                        }

                        if let Some(scope) = multiplayer::scope(m) {
                            match crate::server_selections::load(&root, m) {
                                Ok(selection) => self
                                    .network_drafts
                                    .entry(scope)
                                    .or_default()
                                    .loaded(selection),
                                Err(error) => {
                                    self.status =
                                        format!("Unable to read multiplayer settings: {error:#}")
                                }
                            }
                        }
                        match crate::progress::load(&root, &m.id, &m.creator_key) {
                            Ok(record) => {
                                self.history.insert(m.id.clone(), record);
                            }
                            Err(e) => self.status = format!("Unable to read play history: {e}"),
                        }
                        if let Ok(usage) = crate::storage::usage(&root, m) {
                            self.usage.insert(m.id.clone(), usage);
                        }
                        if let Ok(Some(path)) = crate::game_data::load(&root, m) {
                            self.data_paths
                                .insert(m.id.clone(), path.to_string_lossy().into_owned());
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
        self.persist_window_preferences();
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        // Some window managers ignore maximization during window creation.
        // Restore once after mapping; subsequent window controls belong to the user.
        let restoring_maximize = self.pending_window_maximize
            && ctx.input(|input| {
                input.focused
                    && input.viewport().inner_rect.is_some()
                    && input.viewport().fullscreen == Some(false)
            });
        if restoring_maximize {
            self.pending_window_maximize = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        self.user_files.tick(self.player.as_deref_mut(), ctx);
        let file_dialog_open = self.user_files.is_open();
        egui_winit::gamepkg_input::enable(
            ctx.viewport_id(),
            !self.game_window_closing
                && !file_dialog_open
                && self.player.as_ref().is_some_and(|game| {
                    game.doom_input_allowed()
                        || game.is_cube2()
                        || game.input_events_allowed()
                        || game.supports_zelda_snapshot_slots()
                }),
            ctx.input(|i| i.focused) && !file_dialog_open,
        );
        egui_winit::gamepkg_input::enable_surface(
            ctx.viewport_id(),
            self.player
                .as_ref()
                .is_some_and(|game| game.input_events_allowed()),
        );
        // Closing a viewport can schedule one more update. Do not reopen the
        // loading/error UI after an orderly in-game Quit has saved the session.
        if self.game_window_closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let mut stopped = processes::poll(&mut self.game_processes);
        if stopped.exited {
            let previous_status = self.status.clone();
            self.refresh(ctx);
            if !stopped.errors.is_empty() && self.status != previous_status {
                stopped.errors.push(self.status.clone());
            }
        }
        if !stopped.errors.is_empty() {
            self.status = stopped.errors.join("\n");
        }
        if !self.game_processes.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        if !self.loaded {
            self.loaded = true;
            if !self.game_window {
                theme::install_click_cursor(ctx);
            }
            theme::apply(
                ctx,
                self.settings_draft.dark_mode,
                self.settings_draft.ui_scale_percent,
            );
            self.refresh(ctx);
        }
        self.poll_app_update_check(ctx);
        if let Some(rx) = &self.result {
            match rx.try_recv() {
                Ok(result) => {
                    self.result = None;
                    match result {
                        Ok(Work::NativeBuilt(id)) => {
                            self.refresh(ctx);
                            self.status = match self.installed.iter().find(|m| m.id == id) {
                                Some(manifest) => {
                                    let blockers = self.play_blockers(manifest);
                                    if blockers.is_empty() { "Native build complete. Ready to play.".into() }
                                    else { format!("Native build complete. Before playing: {}", blockers.join(" ")) }
                                }
                                None => "Build finished, but the game is no longer available in this library.".into(),
                            };
                        }
                        Ok(Work::AppRelease(release)) => {
                            self.status = if let Some(r) = &release {
                                format!("GamePKG {} is available.", r.version)
                            } else {
                                "GamePKG is up to date.".into()
                            };
                            self.app_release = release;
                        }
                        Ok(Work::AppInstalled(path)) => {
                            self.status = format!(
                                "Update installed at {}. Ready to restart.",
                                path.parent().unwrap_or(&path).display()
                            );
                            self.app_installed = Some(path);
                        }
                        Ok(Work::Launched(id, child, network)) => {
                            if let Some((scope, selection)) = network {
                                self.network_drafts.insert(scope, selection.into());
                            }
                            self.status = format!("Opened {id} in its own window.");
                            self.game_processes.push((id, child));
                        }
                        Ok(Work::Recordings(loaded)) => self.loaded_recordings(loaded),
                        Ok(Work::RecordingExported(message)) => self.status = message,
                        Ok(Work::VideoSaved(scope, selection)) => {
                            self.video_drafts.insert(scope, selection.into());
                            self.status = "Display settings saved for the next launch.".into();
                            self.refresh(ctx);
                        }
                        Ok(Work::NetworkSaved(scope, selection)) => {
                            self.network_drafts.insert(scope, selection.into());
                            self.status = "Multiplayer settings saved for the next launch.".into();
                            self.refresh(ctx);
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
                        Ok(Work::Game(mut game)) => {
                            self.cube_mouse_capture = Default::default();
                            self.cube_desktop = Default::default();
                            self.preview = None;
                            self.gpu = None;
                            self.vulkan = None;
                            if let Some(state) = self
                                .vulkan_state
                                .as_ref()
                                .filter(|_| !game.shader_sources.is_empty())
                            {
                                match crate::gpu_vulkan::Renderer::new(state, &game.shader_sources)
                                {
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
                            let recorded_audio = game.has_recorded_audio();
                            if game.audio_allowed {
                                match crate::settings::load() {
                                    Ok(settings)
                                        if recorded_audio
                                            || (!settings.mute_audio
                                                && settings.audio_volume > 0) =>
                                    {
                                        let volume = if settings.mute_audio {
                                            0
                                        } else {
                                            settings.audio_volume
                                        };
                                        match crate::audio::Player::new(volume) {
                                            Ok(player) => self.audio_player = Some(player),
                                            Err(error) if volume > 0 => {
                                                self.audio_error =
                                                    format!("Sound unavailable: {error:#}")
                                            }
                                            Err(_) => {}
                                        }
                                    }
                                    Ok(_) => {}
                                    Err(error) => {
                                        self.audio_error = format!("Sound disabled: {error:#}")
                                    }
                                }
                            }
                            if let Err(error) = game.background_player() {
                                self.status = format!("Unable to play: {error:#}");
                                return;
                            }
                            game.cube2_audio_sink(
                                self.audio_player.as_ref().map(crate::audio::Player::sink),
                            );
                            if let Some(audio) = &self.audio_player {
                                match game.effect_playback() {
                                    Ok(Some(playback)) => {
                                        if !audio.original_source(Some(playback)) {
                                            self.status =
                                                "Unable to attach recorded-audio output".into();
                                            self.audio_player = None;
                                            return;
                                        }
                                        game.attach_effect_output(audio.effect_publisher());
                                    }
                                    Ok(None) => {}
                                    Err(error) => {
                                        self.status =
                                            format!("Unable to prepare recorded audio: {error:#}");
                                        self.audio_player = None;
                                        return;
                                    }
                                }
                            }
                            self.selected_game = Some(game.id.clone());
                            self.source_window.release(ctx);
                            self.window_preferences_active = self.game_window;
                            game.enable_user_files();
                            self.player = Some(game);
                        }
                        Ok(Work::Package(m)) => {
                            self.show_add = false;
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
                        Ok(Work::DataPicked(root, requested, path)) => {
                            self.status = (|| -> anyhow::Result<String> {
                                let Some(path) = path else {
                                    return Ok("Folder selection cancelled.".into());
                                };
                                anyhow::ensure!(
                                    std::fs::canonicalize(crate::registry::default_root()?)?
                                        == root,
                                    "The library changed. Choose the folder again."
                                );
                                let _change = crate::registry::LibraryChange::acquire(&root)?;
                                let current =
                                    crate::registry::installed_manifest(&root, &requested.id)?;
                                anyhow::ensure!(
                                    crate::game_data::identity(&current)?
                                        == crate::game_data::identity(&requested)?,
                                    "The game-data request changed. Choose the folder again."
                                );
                                crate::game_data::remember(&root, &current, &path)?;
                                self.data_paths
                                    .insert(current.id, path.to_string_lossy().into_owned());
                                Ok(
                                    "Game-data folder remembered. Original files stay read-only."
                                        .into(),
                                )
                            })()
                            .unwrap_or_else(|error| {
                                format!("Unable to select game data: {error:#}")
                            });
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
            self.keyboard.reset();
            self.source_input = Default::default();
            self.doom_desktop = Default::default();
            self.snapshot_desktop = Default::default();
            self.cube_desktop = Default::default();
            self.sprite_textures.finish(0);
            self.linear_sprites.clear();
        }
        if let Some(game) = self.player.as_mut() {
            let mut image_count = 0;
            let mut linear_count = 0;
            let mut failure = None;
            if game.supports_zelda_snapshot_slots() && !file_dialog_open {
                let events = egui_winit::gamepkg_input::take(ctx.viewport_id());
                if let Some(action) = ctx.input(|input| {
                    self.snapshot_desktop.request(
                        &events,
                        input,
                        self.applied_settings
                            .as_ref()
                            .unwrap_or(&self.settings_draft),
                    )
                }) {
                    let result = crate::registry::default_root().and_then(|root| match action {
                        snapshot_slots::Action::Save(slot) => game.save_zelda_slot(&root, slot),
                        snapshot_slots::Action::Load(slot) => game.load_zelda_slot(&root, slot),
                    });
                    match result {
                        Ok(()) => {
                            if matches!(action, snapshot_slots::Action::Load(_)) {
                                self.pending_input = 0;
                                self.next_frame = None;
                                if let Some(audio) = &self.audio_player {
                                    audio.clear();
                                }
                            }
                        }
                        Err(error) => {
                            rfd::MessageDialog::new()
                                .set_title("Snapshot slot")
                                .set_description(format!("{error:#}"))
                                .set_level(rfd::MessageLevel::Error)
                                .show();
                        }
                    }
                }
            }
            let select_input = ctx.input(|i| {
                u32::from(input::key_edge(i, egui::Key::Tab)) << 7
                    | u32::from(i.key_down(egui::Key::Tab)) << 15
            });
            if game.builtin_engine && !game.is_cube2() {
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
                    // Cube 2 receives physical keys once through its native queue.
                    // Only controller/touch devices use the portable action mapping.
                    let input = if game.is_cube2() {
                        0
                    } else {
                        select_input | ctx.input(|i| self.keyboard.poll(i))
                    };
                    let device_input = self.input_devices.poll(ui, game.is_cube2() || game.doom_input_allowed());
                    let input = if ctx.input(|i| i.focused) && !file_dialog_open {
                        input | device_input
                    } else {
                        self.pending_input = 0;
                        0
                    };
                    if game.is_cube2() {
                        let (width, height) = cube2_input::resolution(
                            ui.available_size(),
                            ctx.pixels_per_point(),
                            self.applied_settings
                                .as_ref()
                                .filter(|s| s.render_limit_enabled)
                                .map(|s| (s.render_max_width, s.render_max_height)),
                        );
                        game.cube2_resize(width, height);
                        let (dx, dy, focused) = ctx.input(|i| {
                            let motion = i
                                .events
                                .iter()
                                .filter_map(|e| {
                                    if let egui::Event::MouseMoved(delta) = e {
                                        Some(*delta)
                                    } else {
                                        None
                                    }
                                })
                                .fold(egui::Vec2::ZERO, |a, b| a + b);
                            (motion.x.round() as i32, motion.y.round() as i32, i.focused)
                        });
                        let (captured, recaptured) = self.cube_mouse_capture.update(
                            focused && !file_dialog_open,
                            ctx.input(cube2_input::release_requested),
                            ctx.input(|i| i.pointer.primary_clicked()),
                        );
                        cube2_input::MouseCapture::apply(ctx, captured, self.confine_mouse);
                        let (look_x, look_y) = self.input_devices.look_delta();
                        let native_events = egui_winit::gamepkg_input::take(ctx.viewport_id());
                        if let Err(error) = self.cube_desktop.forward(
                            game,
                            &native_events,
                            captured,
                            recaptured,
                            ctx.pixels_per_point(),
                        ) {
                            failure = Some(format!("Game stopped: {error:#}"));
                        }
                        if captured {
                            game.cube2_desktop_input(
                                0,
                                if recaptured {
                                    0
                                } else {
                                    dx.saturating_add(look_x)
                                },
                                if recaptured {
                                    0
                                } else {
                                    dy.saturating_add(look_y)
                                },
                            );
                        } else {
                            game.cube2_clear_input();
                        }
                    }
                    if game.doom_input_allowed() {
                        let focused = ctx.input(|i| i.focused);
                        let (captured, recaptured) = self.cube_mouse_capture.update(
                            focused && !file_dialog_open,
                            ctx.input(cube2_input::release_requested),
                            ctx.input(|i| i.pointer.primary_clicked()),
                        );
                        cube2_input::MouseCapture::apply(ctx, captured, self.confine_mouse);
                        let events = egui_winit::gamepkg_input::take(ctx.viewport_id());
                        let (look_x, look_y) = self.input_devices.look_delta();
                        if let Err(error) = self
                            .doom_desktop
                            .forward(game, &events, captured, recaptured)
                        {
                            failure = Some(format!("Game stopped: {error:#}"));
                        }
                        if captured {
                            if let Err(error) = self.doom_desktop.controller_look(
                                game,
                                if recaptured { 0 } else { look_x },
                                if recaptured { 0 } else { look_y },
                            ) {
                                failure = Some(format!("Game stopped: {error:#}"));
                            }
                        }
                    }
                    let input = if game.doom_engine || game.is_cube2() {
                        if self.cube_mouse_capture.captured() {
                            device_input
                        } else {
                            0
                        }
                    } else {
                        input
                    };
                    if game.input_events_allowed() {
                        let scale = (ui.available_width() / 640.0)
                            .min(ui.available_height() / 480.0)
                            .max(0.0);
                        let rect = egui::Rect::from_center_size(
                            ui.max_rect().center(),
                            egui::vec2(640.0 * scale, 480.0 * scale),
                        );
                        let events = egui_winit::gamepkg_input::take(ctx.viewport_id());
                        let release = ctx.input(cube2_input::release_requested);
                        self.source_input.confine(self.source_window.confined());
                        let captured = if release {
                            self.source_input = Default::default();
                            vec![crate::input_events::Event::RESET]
                        } else {
                            self.source_input.translate(
                                &events,
                                rect,
                                ctx.pixels_per_point(),
                                ctx.input(|i| i.focused) && !file_dialog_open,
                            )
                        };
                        for event in captured {
                            if let Err(error) = game.input_event(event) {
                                failure = Some(format!("Game input failed: {error:#}"));
                                break;
                            }
                        }
                    }
                    game.observe_fullscreen(ctx.input(|i| i.viewport().fullscreen));
                    game.observe_window_vsync(self.startup_vsync);
                    let physical_scale = ctx.pixels_per_point();
                    game.observe_window_size(if self.game_window {
                        ctx.input(|input| {
                            let viewport = input.viewport();
                            if viewport.minimized == Some(true) {
                                return None;
                            }
                            viewport.inner_rect.and_then(|rect| {
                                let size = rect.size() * physical_scale;
                                if size.x.is_finite() && size.y.is_finite()
                                    && (1.0..=65535.0).contains(&size.x)
                                    && (1.0..=65535.0).contains(&size.y)
                                {
                                    Some([size.x.round() as u32, size.y.round() as u32])
                                } else { None }
                            })
                        })
                    } else { None });
                    self.pending_input |= input & 0xf00ff;
                    let now = std::time::Instant::now();
                    if self.next_frame.is_none_or(|next| now >= next) {
                        match game.step(self.pending_input | (input & 0xf0ff00)) {
                            Err(e) => failure = Some(format!("Game stopped: {e:#}")),
                            Ok(_) => {
                                if let Some(audio) = &self.audio_player {
                                    for chunk in game
                                        .audio_frame()
                                        .chunks(1600 * game.audio_channels() as usize)
                                    {
                                        audio.submit_channels(chunk, game.audio_channels());
                                    }
                                }
                            }
                        }
                        self.pending_input = 0;
                        self.next_frame = Some(now + std::time::Duration::from_millis(16));
                    }
                    if let Some(url) = game.take_homepage_url() {
                        if !file_dialog_open && ctx.input(|input| input.focused) {
                            ctx.open_url(egui::OpenUrl::new_tab(url));
                        }
                    }
                    if let Some(session)=&self.window_preferences {
                        let native_scale=ctx.pixels_per_point()/ctx.zoom_factor();
                        self.source_window.normal_size([
                            (session.preferences.width as f32*native_scale).round() as u32,
                            (session.preferences.height as f32*native_scale).round() as u32,
                        ]);
                    }
                    if !game.builtin_engine {
                        self.source_window
                            .apply(ctx, game.window_state(), file_dialog_open);
                    } else {
                        self.source_window.apply_display(
                            ctx,
                            game.window_state(),
                            file_dialog_open,
                        );
                    }
                    if let Some(error) = self.audio_player.as_ref().and_then(|a| a.error()) {
                        self.audio_error = format!("Sound stopped: {error}");
                        self.audio_player = None;
                        game.cube2_audio_sink(None);
                    }
                    if game.custom_renderer {
                        let scale = (ui.available_width() / 640.0)
                            .min(ui.available_height() / 480.0)
                            .max(0.0);
                        let display_size = if let Some((width, height)) = game.cube2_frame_size() {
                            let native = egui::vec2(width as f32, height as f32);
                            native
                                * (ui.available_width() / native.x)
                                    .min(ui.available_height() / native.y)
                        } else {
                            egui::vec2(640.0 * scale, 480.0 * scale)
                        };
                        let rect =
                            egui::Rect::from_center_size(ui.max_rect().center(), display_size);
                        let image_scale = egui::vec2(rect.width() / 640.0, rect.height() / 480.0);
                        let painter = ui.painter_at(rect);
                        let color = game_colors::packed;
                        painter.rect_filled(
                            rect,
                            0.0,
                            color(game.frame().background.unwrap_or(0x10171dff)),
                        );
                        let mut drawing_clip = rect;
                        for command in &game.frame().commands {
                            if let crate::drawing::Command::Clip { rect: clip } = command {
                                drawing_clip = clip
                                    .map(|[x, y, w, h]| {
                                        egui::Rect::from_min_size(
                                            rect.min
                                                + egui::vec2(x * image_scale.x, y * image_scale.y),
                                            egui::vec2(w * image_scale.x, h * image_scale.y),
                                        )
                                    })
                                    .unwrap_or(rect)
                                    .intersect(rect);
                                continue;
                            }
                            if !drawing_clip.is_positive() {
                                continue;
                            }
                            let painter = painter.with_clip_rect(drawing_clip);
                            match *command {
                                crate::drawing::Command::Clip { .. } => {
                                    unreachable!("clip handled before drawing")
                                }
                                crate::drawing::Command::Image {
                                    x,
                                    y,
                                    width,
                                    height,
                                    ref image,
                                }
                                | crate::drawing::Command::LinearImage {
                                    x,
                                    y,
                                    width,
                                    height,
                                    ref image,
                                }
                                | crate::drawing::Command::StraightLinearImage {x,y,width,height,ref image} => {
                                    if matches!(
                                        command,
                                        crate::drawing::Command::LinearImage { .. } | crate::drawing::Command::StraightLinearImage {..}
                                    ) {
                                        let bounds=[x,y,width,height];
                                        match self.linear_sprites.callback(
                                            linear_count,
                                            (image,matches!(command,crate::drawing::Command::StraightLinearImage{..})),
                                            bounds,
                                            (rect,ctx.pixels_per_point()),
                                            self.vulkan_state.as_ref(),
                                            self.gl.as_ref(),
                                        ) {
                                            Ok(callback) => painter.add(callback),
                                            Err(error) => {
                                                failure = Some(format!(
                                                    "Unable to draw game sprite: {error:#}"
                                                ));
                                                return;
                                            }
                                        };
                                        linear_count += 1;
                                        continue;
                                    }
                                    let texture = self.sprite_textures.texture(
                                        ctx,
                                        image_count,
                                        image,
                                        matches!(
                                            command,
                                            crate::drawing::Command::LinearImage { .. } | crate::drawing::Command::StraightLinearImage {..}
                                        ),
                                    );
                                    image_count += 1;
                                    let image_rect = egui::Rect::from_min_size(
                                        rect.min + egui::vec2(x * image_scale.x, y * image_scale.y),
                                        egui::vec2(width * image_scale.x, height * image_scale.y),
                                    );
                                    let image_rect = if image.size() == [1, 1] {
                                        sprites::single_texel_bounds(
                                            image_rect,
                                            ctx.pixels_per_point(),
                                        )
                                    } else {
                                        image_rect
                                    };
                                    painter.image(
                                        texture,
                                        image_rect,
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
                                    let covered = linear_sprites::solid_rectangle(
                                        [x, y, width, height], rect, ctx.pixels_per_point(),
                                    );
                                    if covered.is_positive() {
                                        painter.rect_filled(covered, 0.0, color(rgba));
                                    }
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
            self.linear_sprites.finish(linear_count);
            self.observe_window_preferences(ctx, restoring_maximize);
            if self.player.as_ref().is_some_and(|game| game.is_cube2()) {
                ctx.request_repaint();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
            let exited = self.player.as_ref().is_some_and(|game| game.exited());
            if failure.is_some() || exited {
                // Release capture before the next frame can open a native error
                // dialog; blocking the event loop while grabbed traps the cursor.
                self.cube_mouse_capture.release(ctx, self.confine_mouse);
                self.source_window.release(ctx);
                if let Some(game) = &mut self.player {
                    if let Err(e) = game.finish() {
                        record_shutdown_error(&mut failure, &format!("{e:#}"));
                    }
                }
                if self.game_window && failure.is_none() {
                    self.persist_window_preferences();
                    self.game_window_closing = true;
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
                    .selectable_label(
                        !self.show_add && !self.show_settings && !self.show_support,
                        "Library",
                    )
                    .clicked()
                {
                    self.show_add = false;
                    self.show_settings = false;
                    self.show_support = false;
                    self.close_game_page();
                    self.preview = None;
                }
                if ui
                    .add_enabled(
                        self.result.is_none(),
                        egui::SelectableLabel::new(self.show_settings, "Settings"),
                    )
                    .clicked()
                {
                    self.show_add = false;
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
            ui.add_space(8.0);
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
                    self.app_update_controls(ui, ctx);
                    snapshot_slots::settings(ui, &mut self.settings_draft);
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
                    let memory_control = ui.horizontal(|ui| { ui.label("Memory ceiling (MiB)");ui.add(egui::DragValue::new(&mut self.settings_draft.memory_ceiling_mib).range(1..=4096)) }).inner;
                    if self.focus_memory_settings { memory_control.scroll_to_me(Some(egui::Align::Center));self.focus_memory_settings=false; }
                    let tick_control = ui.horizontal(|ui| { ui.label("Tick deadline (ms)");ui.add(egui::Slider::new(&mut self.settings_draft.tick_deadline_ms,10..=1000).step_by(10.0)) }).inner;
                    if self.focus_tick_settings { tick_control.scroll_to_me(Some(egui::Align::Center));self.focus_tick_settings=false; }
                    ui.checkbox(&mut self.settings_draft.render_limit_enabled,"Limit game render resolution");
                    ui.add_enabled_ui(self.settings_draft.render_limit_enabled,|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Maximum width");ui.add(egui::DragValue::new(&mut self.settings_draft.render_max_width).range(320..=10000));
                            ui.label("Maximum height");ui.add(egui::DragValue::new(&mut self.settings_draft.render_max_height).range(200..=10000));
                        });
                    });
                    ui.label("Off uses the game window's native resolution. An enabled limit preserves the window's aspect ratio. Changes apply next launch.");
                    ui.label("Memory uses the lower of your ceiling and the signed game limit. These settings cannot raise a game's hard limit. Changes apply the next time you launch a game.");
                    ui.separator();ui.heading("Builds and appearance");
                    ui.add(egui::Slider::new(&mut self.settings_draft.ui_scale_percent,80..=160).step_by(10.0).text("Interface size (%)"));
                    ui.label("Larger text and controls apply when you save. Display scaling also follows your system settings.");
                    #[cfg(windows)]
                    {
                        ui.heading("Native build tools");
                        ui.label("Download and verify the tools for native engine builds. These tools do not use Python or WSL.");
                        ui.label("About 485 MB to download; about 2.4 GB of disk space including cached downloads.");
                        if ui.add_enabled(self.result.is_none(), egui::Button::new("Prepare native build tools")).clicked() {
                            let root = crate::registry::default_root();
                            self.status = "Preparing native build tools…".into();
                            self.begin_work(move || {
                                crate::takedawn::prepare_windows_tools(&root?)?;
                                Ok(Work::Message("Native build tools are ready.".into()))
                            });
                        }
                        ui.add_space(12.0);
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
                        ui.label("Native tools folder (optional; empty uses installed libraries)");
                        ui.text_edit_singleline(&mut self.settings_draft.native_sysroot_directory);
                        ui.label("Use this for a separate native SDL2/Vulkan/glslang development installation.");
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
                    ui.label("High optimization for this computer: level 3, native CPU tuning, and Rust link-time optimization.");
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
                                    if !self.settings_draft.check_app_updates_on_startup { self.startup_app_check=None; self.app_update_notice=false; }
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
        if self.show_add {
            self.show_add_page(ctx);
            return;
        }
        if self.selected_game.is_some() {
            self.show_game_page(ctx);
            return;
        }
        egui::CentralPanel::default().frame(egui::Frame::central_panel(&ctx.style()).inner_margin(20)).show(ctx, |ui| {
            let scroll = egui::ScrollArea::vertical().id_salt("library-page");
            let scroll = if std::mem::take(&mut self.library_pagination.scroll_to_top) { scroll.vertical_scroll_offset(0.0) } else { scroll };
            scroll.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Your games");
                if ui.add_enabled(self.result.is_none(), egui::Button::new("Add")).clicked() { self.open_add_page(); }
                for (label,value) in [("Installed",self.installed.len().to_string()),("Ready to play",self.installed.iter().filter(|m|self.ready_to_play(m)).count().to_string()),("Storage used",self.library_bytes.map(crate::storage::bytes).unwrap_or_else(||"—".into()))] {
                    let width = ui.fonts(|fonts| {
                        fonts.layout_no_wrap(label.to_owned(), egui::TextStyle::Body.resolve(ui.style()), ui.visuals().text_color()).size().x
                            + fonts.layout_no_wrap(value.clone(), egui::FontId::proportional(18.0), ui.visuals().hyperlink_color).size().x
                    }) + ui.spacing().item_spacing.x + 24.0;
                    if ui.available_size_before_wrap().x < width { ui.end_row(); }
                    egui::Frame::group(ui.style()).inner_margin(egui::Margin::symmetric(10,6)).corner_radius(10).show(ui,|ui| {
                        ui.horizontal(|ui| {ui.label(label);ui.label(egui::RichText::new(value).size(18.0).strong().color(ui.visuals().hyperlink_color));});
                    });
                }
            });
            ui.label("Choose a game to build, play or explore.");
            ui.add_space(12.0);
            self.library_filters(ui);
            let inspect = self.library_games(ui);
            let mut integrity = None;
            for (name, error) in &self.rejected {
                ui.colored_label(egui::Color32::from_rgb(240, 130, 100), format!("Unavailable package: {name}"));
                ui.label(error);
                if ui.add_enabled(self.result.is_none(), egui::Button::new("Restore verified copy / rebuild")).clicked() { if let Some(id)=name.strip_suffix(".gamepkg") { integrity=Some((id.to_owned(),true,None)); } }
                if ui.add_enabled(self.result.is_none() && self.source.trim().ends_with(".gamepkgstore") && std::path::Path::new(self.source.trim()).is_file(),egui::Button::new("Repair assets")).on_hover_text("Use the retained verified package and the original local store bundle entered on the Add page.").clicked() {if let Some(id)=name.strip_suffix(".gamepkg") {integrity=Some((id.to_owned(),true,Some(PathBuf::from(self.source.trim()))));}}
            }
            if self.installed.is_empty() && self.catalog.is_empty() && self.rejected.is_empty() { ui.label("No games yet. Import a game list or install a package."); }
            ui.label(&self.status);
            if ui.add_enabled(self.result.is_none(), egui::Button::new("Refresh library")).on_hover_text("Reload local games and update all saved list URLs without downloading game packages.").clicked() { self.begin_work(|| Ok(Work::Message(crate::catalog::sources::refresh(&crate::registry::default_root()?)?))); }
            if let Some((id,fix,store)) = integrity {
                self.begin_work(move || { let root=crate::registry::default_root()?; Ok(Work::Message(if fix {crate::integrity::fix_with_store(&root,&id,store.as_deref())?} else {crate::integrity::check(&root,&id)?})) });
            }
            if let Some(id)=inspect {self.open_game_page(&id);}
            ui.label(format!("Library storage used: {} (file bytes, including shared assets, caches, saves and play history)", self.library_bytes.map(crate::storage::bytes).unwrap_or_else(||"Unavailable".into())));
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

fn record_shutdown_error(failure: &mut Option<String>, detail: &str) {
    match failure {
        Some(primary) if primary.contains(detail) => {}
        Some(primary) => {
            primary.push_str("\nDuring shutdown: ");
            primary.push_str(detail);
        }
        None => *failure = Some(format!("Unable to finish the game session: {detail}")),
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

#[cfg(test)]
mod shutdown_error_tests {
    use super::record_shutdown_error;
    #[test]
    fn rendering_failure_is_preserved_and_not_mislabeled_as_history() {
        let detail = "The renderer exceeded your 100 ms tick deadline.";
        let original = format!("Game stopped: {detail}");
        let mut failure = Some(original.clone());
        record_shutdown_error(&mut failure, detail);
        assert_eq!(failure.as_deref(), Some(original.as_str()));
        record_shutdown_error(&mut failure, "Unable to save play history: disk full");
        assert_eq!(
            failure.unwrap(),
            format!("{original}\nDuring shutdown: Unable to save play history: disk full")
        );
    }
    #[test]
    fn shutdown_failure_alone_does_not_claim_history_failed() {
        let mut failure = None;
        record_shutdown_error(&mut failure, "renderer stopped");
        assert_eq!(
            failure.as_deref(),
            Some("Unable to finish the game session: renderer stopped")
        );
    }
}
