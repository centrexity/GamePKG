//! Game-only window and platform providers for the native executable.
//! The pipe-based driver is retained for historical fixtures.
use super::{
    broker::{self, LocalConnection, PendingCalls},
    desktop::Desktop,
    graphics::Pipeline,
    owner::{Grants, Owner},
    preparation::Artifact,
    Backend, Error, Frame, Result as ServiceResult, Session, WindowCommand, WindowState,
};
use crate::manifest::Manifest;
use anyhow::{ensure, Context, Result};
use eframe::{egui, egui_wgpu, wgpu};
use egui_winit::winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{CursorGrabMode, Fullscreen, Window, WindowId},
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct Options {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub visible: bool,
    pub process_memory: u64,
    pub graphics_memory: u64,
    pub samples: u32,
    pub mute_audio: bool,
    pub volume: u8,
    /// User approval, independent of the signed microphone request.
    pub microphone_approved: bool,
    /// Already resolved immutable preferences for the native game.
    pub launch: crate::launch_options::LaunchArgs,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            title: "GamePKG game".into(),
            width: 1280,
            height: 720,
            fullscreen: false,
            visible: true,
            process_memory: 128 * 1024 * 1024,
            graphics_memory: 128 * 1024 * 1024,
            samples: 1,
            mute_audio: false,
            volume: 50,
            microphone_approved: false,
            launch: Default::default(),
        }
    }
}
struct WindowBackend {
    desktop: Desktop,
    window: Arc<Window>,
    closed: bool,
}
impl WindowBackend {
    fn observed(&self) -> WindowState {
        let size = self.window.inner_size();
        WindowState {
            width: size.width,
            height: size.height,
            fullscreen: self.window.fullscreen().is_some(),
            focused: self.window.has_focus(),
            closed: self.closed,
        }
    }
}
impl Backend for WindowBackend {
    fn window(&mut self, command: WindowCommand) -> ServiceResult<()> {
        match command {
            WindowCommand::Fullscreen(value) => self.window.set_fullscreen(
                value.then(|| Fullscreen::Borderless(self.window.current_monitor())),
            ),
            WindowCommand::CursorVisible(value) => {
                self.window.set_cursor_visible(value);
                self.desktop.window(command)?;
            }
            WindowCommand::CursorLocked(value) => {
                let result = if value {
                    self.window
                        .set_cursor_grab(CursorGrabMode::Locked)
                        .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined))
                } else {
                    self.window.set_cursor_grab(CursorGrabMode::None)
                };
                result.map_err(|_| Error::Unsupported)?;
            }
            WindowCommand::Resize(w, h) => {
                if w == 0 || h == 0 || w > 16384 || h > 16384 {
                    return Err(Error::Invalid);
                }
                let _ = self
                    .window
                    .request_inner_size(egui_winit::winit::dpi::PhysicalSize::new(w, h));
            }
            WindowCommand::Close => self.closed = true,
        }
        Ok(())
    }
    fn tick_begin(&mut self) -> ServiceResult<()> {
        self.desktop.tick_begin()
    }
    fn tick_finish(&mut self) -> ServiceResult<()> {
        self.desktop.tick_finish()
    }
    fn tick_audio(&mut self, operation: u32, name: &str, value: u32) -> bool {
        self.desktop.tick_audio(operation, name, value)
    }
    fn tick_homepage(&mut self, url: String) {
        self.desktop.tick_homepage(url);
    }
    fn present(&mut self, frame: Frame) -> ServiceResult<()> {
        self.desktop.present(frame)
    }
    fn audio(&mut self, data: &[i16], channels: u32) -> ServiceResult<()> {
        self.desktop.audio(data, channels)
    }
    fn rumble(&mut self, id: u64, low: u16, high: u16, ms: u32) -> ServiceResult<()> {
        self.desktop.rumble(id, low, high, ms)
    }
    fn graphics(&mut self, op: u32, bytes: &[u8], cap: usize) -> ServiceResult<Vec<u8>> {
        self.desktop.graphics(op, bytes, cap)
    }
    fn capture(&mut self, op: u32, bytes: &[u8], cap: usize) -> ServiceResult<Vec<u8>> {
        self.desktop.capture(op, bytes, cap)
    }
    fn revoke_capture(&mut self) {
        self.desktop.revoke_capture();
    }
    fn audio_streams(&self) -> Option<super::audio_streams::Mixer> {
        self.desktop.audio_streams()
    }
    fn texture_limit(&self) -> u32 {
        self.desktop.texture_limit()
    }
    fn shader_count(&self) -> u32 {
        self.desktop.shader_count()
    }
}

fn connect_window(host: &mut Session<WindowBackend>) -> Result<()> {
    let clock = host.timing();
    let backend = host.backend();
    backend
        .desktop
        .connect_clock(clock)
        .map_err(|e| anyhow::anyhow!("window wake: {e:?}"))?;
    let observed = backend.observed();
    host.observe_window(observed);
    Ok(())
}

type LocalEntry = Box<dyn FnOnce(LocalConnection) -> Result<()> + Send>;
type FirstFrame = Box<dyn FnOnce() -> Result<()>>;

enum Launch {
    Pipe(Box<Artifact>),
    Local(Box<Manifest>, Option<LocalEntry>),
}
impl Launch {
    fn manifest(&self) -> &Manifest {
        match self {
            Self::Pipe(artifact) => artifact.manifest(),
            Self::Local(manifest, _) => manifest,
        }
    }
}

enum Driver {
    Pipe(Box<Owner<WindowBackend>>),
    Local {
        host: Box<Session<WindowBackend>>,
        pending: PendingCalls,
        worker: Option<JoinHandle<Result<()>>>,
    },
}
impl Driver {
    fn host(&mut self) -> &mut Session<WindowBackend> {
        match self {
            Self::Pipe(owner) => &mut owner.host,
            Self::Local { host, .. } => host,
        }
    }

    fn update(&mut self) -> Result<bool> {
        match self {
            Self::Pipe(owner) => {
                owner.dispatch(32);
                match owner.poll()? {
                    None => Ok(false),
                    Some(0) => Ok(true),
                    Some(code) => {
                        anyhow::bail!("game process exited {code}: {}", owner.diagnostics())
                    }
                }
            }
            Self::Local {
                host,
                pending,
                worker,
            } => {
                pending.dispatch(host, 32);
                if !worker.as_ref().is_some_and(|job| job.is_finished()) {
                    return Ok(false);
                }
                worker
                    .take()
                    .unwrap()
                    .join()
                    .map_err(|_| anyhow::anyhow!("native game worker panicked"))??;
                Ok(true)
            }
        }
    }

    fn stop(self) {
        match self {
            Self::Pipe(mut owner) => {
                let _ = owner.stop();
            }
            Self::Local { host, pending, .. } => {
                host.timing().close();
                // Release queued callers without joining a game worker on the
                // event thread. The dedicated executable exits after run_local.
                drop(pending);
            }
        }
    }
}

struct Running {
    driver: Driver,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    state: egui_wgpu::RenderState,
    config: wgpu::SurfaceConfiguration,
    context: egui::Context,
    gui: egui_winit::State,
    input: super::winit_input::Provider,
    controllers: Option<Rc<RefCell<super::controllers::Provider>>>,
    #[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
    picker: Option<super::tick_picker::Picker>,
}
#[derive(Debug, Default)]
pub struct Report {
    pub presented_frames: u64,
    pub adapter: String,
}
struct App {
    report: Report,
    launch: Launch,
    grants: Option<Grants>,
    pipelines: Option<Vec<Pipeline>>,
    options: Options,
    first_frame: Option<FirstFrame>,
    proxy: EventLoopProxy<()>,
    running: Option<Running>,
    error: Option<anyhow::Error>,
    next_input: Instant,
}
impl App {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Running> {
        let options = &self.options;
        let attributes = Window::default_attributes()
            .with_title(format!(
                "{} — GamePKG {}",
                options.title,
                env!("CARGO_PKG_VERSION")
            ))
            .with_visible(options.visible)
            .with_inner_size(egui_winit::winit::dpi::PhysicalSize::new(
                options.width,
                options.height,
            ))
            .with_fullscreen(options.fullscreen.then_some(Fullscreen::Borderless(None)));
        let window = Arc::new(event_loop.create_window(attributes)?);
        window.set_ime_allowed(true);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .context("hardware Vulkan adapter unavailable")?;
        ensure!(
            adapter.get_info().backend == wgpu::Backend::Vulkan
                && adapter.get_info().device_type != wgpu::DeviceType::Cpu,
            "hardware Vulkan required"
        );
        self.report.adapter = adapter.get_info().name;
        let features = adapter.features()
            & (wgpu::Features::DEPTH32FLOAT_STENCIL8
                | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER);
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("GamePKG shared port device"),
                required_features: features,
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
            None,
        ))?;
        let mut config = surface
            .get_default_config(&adapter, options.width, options.height)
            .context("Vulkan presentation unavailable")?;
        // Existing rendering work is retained. Presentation synchronization does
        // not alter game timers or impose an instruction/frame watchdog.
        config.present_mode = wgpu::PresentMode::Fifo;
        surface.configure(&device, &config);
        let renderer = egui_wgpu::Renderer::new(&device, config.format, None, 1, false);
        let state = egui_wgpu::RenderState {
            adapter,
            available_adapters: Vec::new(),
            device,
            queue,
            target_format: config.format,
            renderer: Arc::new(egui::mutex::RwLock::new(renderer)),
        };
        let context = egui::Context::default();
        context.set_pixels_per_point(1.0);
        let proxy = self.proxy.clone();
        context.set_request_repaint_callback(move |_| {
            let _ = proxy.send_event(());
        });
        let gui = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(1.0),
            None,
            Some(state.device.limits().max_texture_dimension_2d as usize),
        );
        let audio = if self.launch.manifest().permissions.audio && !options.mute_audio {
            Some(crate::audio::Player::new(options.volume)?)
        } else {
            None
        };
        let mut desktop = Desktop::new(context.clone(), &state, &[], audio)?;
        if let Some(package) = self
            .grants
            .as_ref()
            .and_then(|grants| grants.recorded_audio.as_ref())
        {
            desktop.install_recorded_audio(package, self.launch.manifest())?;
        }
        desktop.configure_microphone(options.microphone_approved, 2 * 1024 * 1024);
        desktop.install_graphics(
            self.pipelines.take().unwrap(),
            options.graphics_memory,
            options.samples,
        )?;
        let controllers = super::controllers::Provider::new()
            .ok()
            .map(|p| Rc::new(RefCell::new(p)));
        if let Some(provider) = &controllers {
            let provider = provider.clone();
            desktop.set_rumble(move |id, low, high, ms| {
                provider.borrow_mut().rumble(id, low, high, ms)
            });
        }
        let backend = WindowBackend {
            desktop,
            window: window.clone(),
            closed: false,
        };
        let proxy = self.proxy.clone();
        let wake = move || {
            let _ = proxy.send_event(());
        };
        #[allow(unused_mut)]
        let mut grants = self
            .grants
            .take()
            .context("window session already started")?;
        #[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
        let picker = grants
            .picker
            .take()
            .map(|socket| super::tick_picker::Picker::new(socket, self.launch.manifest()));
        let mut driver = match &mut self.launch {
            Launch::Pipe(artifact) => Driver::Pipe(Box::new(Owner::start(
                artifact,
                backend,
                grants,
                options.process_memory,
                wake,
            )?)),
            Launch::Local(manifest, entry) => {
                let entry = entry.take().context("local entry already started")?;
                let mut host = super::owner::session(manifest, backend, grants)?;
                host.launch_options = Some(options.launch.service_bytes(None)?);
                connect_window(&mut host)?;
                let (connection, pending) =
                    broker::channel_with_services(host.worker_services(), wake);
                let connection = connection.into_local_worker()?;
                let worker = thread::Builder::new()
                    .name("gamepkg-game".into())
                    .spawn(move || entry(connection))?;
                Driver::Local {
                    host: Box::new(host),
                    pending,
                    worker: Some(worker),
                }
            }
        };
        if let Driver::Pipe(owner) = &mut driver {
            connect_window(&mut owner.host)?;
        }
        #[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
        if picker.is_some() {
            if let Some(tick) = driver.host().tick.as_mut() {
                tick.files.enable();
            }
        }
        Ok(Running {
            driver,
            window,
            surface,
            state,
            config,
            context,
            gui,
            input: Default::default(),
            controllers,
            #[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
            picker,
        })
    }
    fn stop(&mut self, event_loop: &ActiveEventLoop, error: Option<anyhow::Error>) {
        if let Some(mut running) = self.running.take() {
            running
                .input
                .disconnect(running.driver.host().input_backend());
            if let Some(provider) = &running.controllers {
                provider
                    .borrow_mut()
                    .disconnect(running.driver.host().input_backend());
            }
            running.driver.host().revoke_microphone();
            running.driver.stop();
        }
        if self.error.is_none() {
            self.error = error;
        }
        event_loop.exit();
    }
    fn service(&mut self, event_loop: &ActiveEventLoop) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        let observed = running.driver.host().backend().observed();
        running.driver.host().observe_window(observed);
        if let Some(provider) = &running.controllers {
            provider
                .borrow_mut()
                .poll(running.driver.host().input_backend());
        }
        running.state.device.poll(wgpu::Maintain::Poll);
        if observed.closed {
            self.stop(event_loop, None);
            return;
        }
        #[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
        if let Some(picker) = running.picker.as_mut() {
            if let Some(tick) = running.driver.host().tick.as_mut() {
                let proxy = self.proxy.clone();
                if picker.tick(&mut tick.files, move || {
                    let _ = proxy.send_event(());
                }) {
                    let _ = running
                        .driver
                        .host()
                        .backend()
                        .window(WindowCommand::CursorLocked(false));
                    let _ = running
                        .driver
                        .host()
                        .backend()
                        .window(WindowCommand::CursorVisible(true));
                }
            }
        }
        let finished = running.driver.update();
        if let Some(tick) = running.driver.host().tick.as_mut() {
            for index in tick.take_achievements() {
                // The isolated game worker has no descriptor for this stream.
                println!("GamePKG achievement {index}");
            }
        }
        match finished {
            Ok(false) => {}
            Ok(true) => self.stop(event_loop, None),
            Err(error) => self.stop(event_loop, Some(error)),
        }
    }
    fn draw(running: &mut Running) -> Result<bool> {
        let size = running.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(false);
        }
        let mode = match running.driver.host().tick.as_ref().map(|tick| tick.vsync) {
            Some(false) => wgpu::PresentMode::AutoNoVsync,
            _ => wgpu::PresentMode::Fifo,
        };
        if running.config.width != size.width
            || running.config.height != size.height
            || running.config.present_mode != mode
        {
            running.config.present_mode = mode;
            running.config.width = size.width;
            running.config.height = size.height;
            running
                .surface
                .configure(&running.state.device, &running.config);
        }
        let frame = match running.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                running
                    .surface
                    .configure(&running.state.device, &running.config);
                running.window.request_redraw();
                return Ok(false);
            }
            Err(wgpu::SurfaceError::Timeout) => {
                running.window.request_redraw();
                return Ok(false);
            }
            Err(e) => return Err(e.into()),
        };
        let input = running.gui.take_egui_input(&running.window);
        let mut fresh_frame = false;
        let output = running.context.run(input, |context| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(context, |ui| {
                    fresh_frame |= running.driver.host().backend().desktop.paint(ui);
                });
        });
        running
            .gui
            .handle_platform_output(&running.window, output.platform_output);
        let primitives = running
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: output.pixels_per_point,
        };
        let mut renderer = running.state.renderer.write();
        for (id, delta) in output.textures_delta.set {
            renderer.update_texture(&running.state.device, &running.state.queue, id, &delta);
        }
        let mut encoder =
            running
                .state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("GamePKG game-only presentation"),
                });
        let commands = renderer.update_buffers(
            &running.state.device,
            &running.state.queue,
            &mut encoder,
            &primitives,
            &screen,
        );
        let view = frame.texture.create_view(&Default::default());
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GamePKG game-only surface"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            renderer.render(&mut pass.forget_lifetime(), &primitives, &screen);
        }
        running.state.queue.submit(
            commands
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        frame.present();
        for id in output.textures_delta.free {
            renderer.free_texture(&id);
        }
        Ok(fresh_frame)
    }

    fn present(&mut self, event_loop: &ActiveEventLoop) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        match Self::draw(running) {
            Ok(true) => {
                self.report.presented_frames += 1;
                if let Some(notify) = self.first_frame.take() {
                    if let Err(error) = notify() {
                        self.stop(event_loop, Some(error));
                    }
                }
            }
            Ok(false) => {}
            Err(error) => self.stop(event_loop, Some(error)),
        }
    }

    fn present_hidden_frame(&mut self, event_loop: &ActiveEventLoop) {
        // Windows does not deliver paint messages to hidden windows. Paint
        // pending game frames on the same owner thread using service wakeups.
        if !self.options.visible
            && self
                .running
                .as_mut()
                .is_some_and(|running| running.driver.host().backend().desktop.has_pending_frame())
        {
            self.present(event_loop);
        }
    }
}

/// Service wakes are not game frames. Only poll-driven legacy games need a
/// redraw for every wake; service-runtime games paint when a frame is pending.
fn wake_requests_redraw(manifest: &Manifest, pending_frame: bool) -> bool {
    let poll_redraw = !manifest.native_tick && !manifest.uses_service_runtime();
    poll_redraw || pending_frame
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_none() {
            match self.start(event_loop) {
                Ok(r) => {
                    r.window.request_redraw();
                    self.running = Some(r);
                }
                Err(e) => self.stop(event_loop, Some(e)),
            }
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, _: ()) {
        self.service(event_loop);
        if !self.options.visible {
            self.present_hidden_frame(event_loop);
            return;
        }
        if let Some(running) = self.running.as_mut() {
            let pending_frame = running.driver.host().backend().desktop.has_pending_frame();
            if wake_requests_redraw(self.launch.manifest(), pending_frame) {
                running.window.request_redraw();
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(r) = self.running.as_mut() else {
            return;
        };
        if r.window.id() != id {
            return;
        }
        let observed = r.driver.host().backend().observed();
        if let Some(command) = r
            .input
            .window(r.driver.host().input_backend(), &event, observed)
        {
            if let Err(e) = r.driver.host().backend().window(command) {
                self.stop(event_loop, Some(anyhow::anyhow!("window command: {e:?}")));
                return;
            }
        }
        let _ = r.gui.on_window_event(&r.window, &event);
        match event {
            WindowEvent::CloseRequested => {
                self.stop(event_loop, None);
                return;
            }
            WindowEvent::RedrawRequested => self.present(event_loop),
            WindowEvent::Resized(_)
            | WindowEvent::Focused(_)
            | WindowEvent::ScaleFactorChanged { .. } => r.window.request_redraw(),
            _ => {}
        }
        self.service(event_loop);
    }
    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let Some(r) = self.running.as_mut() {
            r.input.device(
                r.driver.host().input_backend(),
                &event,
                r.window.has_focus(),
            );
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if Instant::now() >= self.next_input {
            self.service(event_loop);
            self.present_hidden_frame(event_loop);
            self.next_input = Instant::now() + Duration::from_millis(4);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_input));
    }
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.stop(
            event_loop,
            Some(anyhow::anyhow!("native window suspended; session revoked")),
        );
    }
}
/// Historical pipe fixture. Pipeline order/sampling must exactly match the
/// audited build receipt. These inputs are host-prepared, never game RPC data.
pub fn run(
    artifact: Artifact,
    pipelines: Vec<Pipeline>,
    grants: Grants,
    options: Options,
) -> Result<Report> {
    artifact.executable()?;
    let hashes = artifact.receipt().game.shaders.clone();
    run_window(
        Launch::Pipe(Box::new(artifact)),
        pipelines,
        &hashes,
        grants,
        options,
        None,
    )
}

/// Main-thread entry for the compatibility layer inside a dedicated game
/// executable. The trusted bootstrap supplies the authenticated manifest and
/// the pipeline hashes from the build receipt. It binds the client and applies
/// the game worker's confinement before invoking a linked entry point. After
/// this returns the bootstrap must exit the process, including on window close.
pub fn run_local(
    manifest: Manifest,
    pipelines: Vec<Pipeline>,
    shader_hashes: &[String],
    grants: Grants,
    options: Options,
    on_first_frame: impl FnOnce() -> Result<()> + 'static,
    entry: impl FnOnce(LocalConnection) -> Result<()> + Send + 'static,
) -> Result<Report> {
    run_window(
        Launch::Local(Box::new(manifest), Some(Box::new(entry))),
        pipelines,
        shader_hashes,
        grants,
        options,
        Some(Box::new(on_first_frame)),
    )
}

fn run_window(
    launch: Launch,
    pipelines: Vec<Pipeline>,
    shader_hashes: &[String],
    grants: Grants,
    options: Options,
    first_frame: Option<FirstFrame>,
) -> Result<Report> {
    ensure!(
        (1..=16384).contains(&options.width)
            && (1..=16384).contains(&options.height)
            && options.volume <= 100,
        "invalid game window options"
    );
    let charged = options
        .process_memory
        .checked_add(options.graphics_memory)
        .and_then(|n| n.checked_add(grants.volatile_cache_bytes.unwrap_or(0)))
        .and_then(|n| n.checked_add(216 * 1024 * 1024))
        .and_then(|n| n.checked_add(super::diagnostics::RESERVED_BYTES))
        .context("memory budget overflow")?;
    ensure!(
        charged <= launch.manifest().limits.hard_memory_bytes,
        "process, graphics and media budgets exceed signed hard limit"
    );
    let hashes = pipelines
        .iter()
        .map(|p| p.identity_with_samples(options.samples))
        .collect::<Vec<_>>();
    ensure!(
        hashes == shader_hashes,
        "pipeline identities differ from audited build"
    );
    let event_loop = EventLoop::<()>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut app = App {
        report: Report::default(),
        launch,
        grants: Some(grants),
        pipelines: Some(pipelines),
        options,
        first_frame,
        proxy,
        running: None,
        error: None,
        next_input: Instant::now(),
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        Err(error)
    } else {
        Ok(app.report)
    }
}

#[cfg(test)]
mod wake_redraw_tests {
    use super::wake_requests_redraw;
    use crate::manifest::Manifest;

    fn manifest(prefix: &str) -> Manifest {
        let base = include_str!("../../examples/minimal/manifest.toml");
        Manifest::parse(format!("{prefix}{base}").as_bytes()).unwrap()
    }

    #[test]
    fn service_wakes_without_a_pending_frame_do_not_request_redraw() {
        let manifest = manifest("native_services = true\n");
        assert!(manifest.uses_service_runtime());
        assert!(!manifest.native_tick);
        for _ in 0..8 {
            assert!(!wake_requests_redraw(&manifest, false));
        }
    }

    #[test]
    fn one_service_wake_with_a_pending_frame_requests_one_redraw() {
        let manifest = manifest("native_services = true\n");
        assert!(wake_requests_redraw(&manifest, true));
    }

    #[test]
    fn legacy_poll_driven_wakes_keep_requesting_redraws() {
        let legacy = manifest("");
        assert!(!legacy.uses_service_runtime());
        assert!(!legacy.native_tick);
        for _ in 0..8 {
            assert!(wake_requests_redraw(&legacy, false));
            assert!(wake_requests_redraw(&legacy, true));
        }
        let tick = manifest("native_tick = true\n");
        assert!(tick.native_tick);
        assert!(!wake_requests_redraw(&tick, false));
        assert!(wake_requests_redraw(&tick, true));
    }
}
