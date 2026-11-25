//! Shared desktop implementation: Vulkan presentation, the game's own window,
//! and the existing output-only host audio player. No SDL types in the API.
use super::{media::Frame, *};
use eframe::{egui, egui_wgpu, wgpu};
use std::sync::Weak;
type Rumble = Box<dyn FnMut(u64, u16, u16, u32) -> Result<()>>;

pub struct Desktop {
    state: egui_wgpu::RenderState,
    graphics: Option<super::graphics::Graphics>,
    pending_native: Option<super::graphics::Presented>,
    current_native: Option<(super::graphics::Presented, egui::TextureId)>,
    context: egui::Context,
    renderer: Arc<crate::gpu_vulkan::Renderer>,
    shader_count: u32,
    texture_limit: u32,
    pending: Option<Frame>,
    current: Option<Frame>,
    textures: BTreeMap<(usize, bool), (Weak<Image>, egui::TextureHandle)>,
    straight_cache: crate::gui::linear_sprites::Cache,
    straight_sources: BTreeMap<usize, (Weak<Image>, Arc<crate::drawing::image::Image>)>,
    recorded: crate::audio_effects::Runtime,
    audio: Option<crate::audio::Player>,
    streams: Option<super::audio_streams::Mixer>,
    microphone_allowed: bool,
    capture: Option<(u64, crate::audio::capture::Device)>,
    capture_budget: u64,
    rumble: Option<Rumble>,
    cursor_visible: bool,
}
impl Desktop {
    /// Caller supplies only verified compiled shader sources. Refuses any
    /// non-Vulkan adapter; no software/alternate backend is selected here.
    pub fn new(
        context: egui::Context,
        state: &egui_wgpu::RenderState,
        shaders: &[String],
        audio: Option<crate::audio::Player>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            state.adapter.get_info().backend == wgpu::Backend::Vulkan,
            "port windows require Vulkan"
        );
        anyhow::ensure!(
            state.adapter.get_info().device_type != wgpu::DeviceType::Cpu,
            "port windows require hardware graphics"
        );
        let streams = audio
            .as_ref()
            .map(|output| {
                let layout = if output.port_channels() == 1 {
                    super::audio_streams::Layout::Mono
                } else {
                    super::audio_streams::Layout::Stereo
                };
                let mixer = super::audio_streams::Mixer::new(layout, 2 * 1024 * 1024);
                let map = (0..layout.channels()).collect::<Vec<_>>();
                anyhow::ensure!(
                    output.install_port_mixer(mixer.clone(), &map),
                    "unable to attach bounded port audio mixer"
                );
                Ok(mixer)
            })
            .transpose()?;
        Ok(Self {
            state: state.clone(),
            graphics: None,
            pending_native: None,
            current_native: None,
            renderer: Arc::new(crate::gpu_vulkan::Renderer::new(state, shaders)?),
            shader_count: shaders.len() as u32,
            texture_limit: state.device.limits().max_texture_dimension_2d,
            context,
            pending: None,
            current: None,
            textures: BTreeMap::new(),
            recorded: Default::default(),
            straight_cache: Default::default(),
            straight_sources: BTreeMap::new(),
            audio,
            streams,
            microphone_allowed: false,
            capture: None,
            capture_budget: 2 * 1024 * 1024,
            rumble: None,
            cursor_visible: true,
        })
    }
    /// Host bootstrap only, using verified Slang/WGSL pipeline metadata and a
    /// budget derived from the game's approved memory policy. Install once.
    pub fn install_graphics(
        &mut self,
        pipelines: Vec<super::graphics::Pipeline>,
        budget: u64,
        samples: u32,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(self.graphics.is_none(), "native graphics already installed");
        self.graphics = Some(super::graphics::Graphics::new(
            &self.state,
            pipelines,
            budget,
            samples,
        )?);
        let context = self.context.clone();
        self.graphics
            .as_mut()
            .unwrap()
            .set_wake(move || context.request_repaint());
        Ok(())
    }
    /// Trusted controller provider resolves canonical IDs to its actual device.
    pub fn set_rumble(&mut self, callback: impl FnMut(u64, u16, u16, u32) -> Result<()> + 'static) {
        self.rumble = Some(Box::new(callback));
    }
    /// Bootstrap-only selection of verified speaker mapping and charged queue
    /// budget. In particular a 5.1 layout requires an explicit provider map.
    pub fn configure_audio(
        &mut self,
        layout: super::audio_streams::Layout,
        budget: u64,
        map: &[usize],
    ) -> Result<()> {
        let output = self.audio.as_ref().ok_or(Error::Unsupported)?;
        let mixer = super::audio_streams::Mixer::new(layout, budget);
        if !output.install_port_mixer(mixer.clone(), map) {
            return Err(Error::Unsupported);
        }
        if let Some(previous) = self.streams.replace(mixer) {
            previous.close();
        }
        Ok(())
    }
    /// Explicit USER approval supplied by the launcher, independently of the
    /// game's signed microphone request. False revokes buffered data at once.
    pub fn configure_microphone(&mut self, approved: bool, budget: u64) {
        self.microphone_allowed = approved;
        self.capture_budget = budget;
        if !approved {
            self.capture.take();
        }
    }
    /// Host bootstrap connects completion wakeups to the SAME session clock
    /// used by its private worker. A closed session safely ignores late wakes.
    pub fn connect_clock(&mut self, clock: super::timing::Clock) -> Result<()> {
        let context = self.context.clone();
        self.graphics
            .as_mut()
            .ok_or(Error::Unsupported)?
            .set_wake(move || {
                let _ = clock.wake();
                context.request_repaint();
            });
        Ok(())
    }
    #[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
    pub(super) fn install_recorded_audio(
        &mut self,
        package: &crate::archive::Package,
        manifest: &crate::manifest::Manifest,
    ) -> anyhow::Result<()> {
        let Some(output) = self.audio.as_ref() else {
            return Ok(());
        };
        let (mut recorded, _) = crate::audio_effects::Runtime::new(
            Some(package),
            manifest.limits.effects_memory_bytes,
            manifest.limits.music_memory_bytes,
            manifest.limits.hard_memory_bytes,
            manifest.permissions.audio,
        )?;
        if recorded.enabled() {
            anyhow::ensure!(
                output.original_source(recorded.playback()?),
                "recorded audio output is unavailable"
            );
            recorded.attach(output.effect_publisher());
        }
        self.recorded = recorded;
        Ok(())
    }
    pub fn window_state(&self) -> WindowState {
        self.context.input(|i| {
            let v = i.viewport();
            let size = v
                .inner_rect
                .map(|r| r.size() * i.pixels_per_point)
                .unwrap_or_default();
            WindowState {
                width: size.x.max(0.0) as u32,
                height: size.y.max(0.0) as u32,
                fullscreen: v.fullscreen.unwrap_or(false),
                focused: v.focused.unwrap_or(false),
                closed: v.close_requested(),
            }
        })
    }
    #[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
    pub(super) fn has_pending_frame(&self) -> bool {
        self.pending.is_some() || self.pending_native.is_some()
    }
    /// Paint inside a margin-free game viewport. This adds no menus/controls.
    /// Returns whether a new game frame was consumed in this redraw.
    pub fn paint(&mut self, ui: &mut egui::Ui) -> bool {
        let fresh_frame = self.pending.is_some() || self.pending_native.is_some();
        if !self.cursor_visible {
            self.context.set_cursor_icon(egui::CursorIcon::None);
        }
        if let Some(frame) = self.pending.take() {
            self.current = Some(frame);
            self.clear_native();
        }
        if let Some(frame) = self.pending_native.take() {
            self.current = None;
            self.textures.clear();
            self.clear_native();
            let id = self.state.renderer.write().register_native_texture(
                &self.state.device,
                &frame.view,
                wgpu::FilterMode::Linear,
            );
            self.current_native = Some((frame, id));
        }
        if let Some((_, id)) = &self.current_native {
            ui.painter().image(
                *id,
                ui.max_rect(),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            return fresh_frame;
        }
        let Some(frame) = self.current.as_ref() else {
            return false;
        };
        let scale = self.context.pixels_per_point();
        let origin = ui.max_rect().min;
        let c = frame.clear.to_be_bytes();
        ui.painter().rect_filled(
            ui.max_rect(),
            0.0,
            egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]),
        );
        self.textures
            .retain(|_, (image, _)| image.strong_count() > 0);
        self.straight_sources
            .retain(|_, (image, _)| image.strong_count() > 0);
        let mut straight_count = 0;
        for draw in &frame.draws {
            let [x, y, w, h] = draw.rect;
            let rect = egui::Rect::from_min_size(
                origin + egui::vec2(x, y) / scale,
                egui::vec2(w, h) / scale,
            );
            let painter = match draw.clip {
                Some([x, y, w, h]) => ui.painter().with_clip_rect(egui::Rect::from_min_size(
                    origin + egui::vec2(x, y) / scale,
                    egui::vec2(w, h) / scale,
                )),
                None => ui.painter().clone(),
            };
            if let Some(color) = draw.solid {
                let [r, g, b, a] = color.to_be_bytes();
                let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                if draw.circle {
                    painter.circle_filled(rect.center(), rect.width() / 2.0, color);
                } else {
                    painter.rect_filled(rect, 0.0, color);
                }
                continue;
            }
            let Some(image) = &draw.image else {
                continue;
            };
            if draw.circle {
                let c = &image.rgba;
                painter.circle_filled(
                    rect.center(),
                    rect.width() / 2.0,
                    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]),
                );
            } else if draw.straight {
                let key = Arc::as_ptr(image) as usize;
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    self.straight_sources.entry(key)
                {
                    if let Some(source) =
                        crate::drawing::image::Image::new(image.width, image.height, &image.rgba)
                    {
                        entry.insert((Arc::downgrade(image), Arc::new(source)));
                    }
                }
                if let Some((_, source)) = self.straight_sources.get(&key) {
                    if let Ok(callback) = self.straight_cache.callback(
                        straight_count,
                        (source, true),
                        [x, y, w, h],
                        (ui.max_rect(), scale),
                        Some(&self.state),
                        None,
                    ) {
                        painter.add(egui::Shape::Callback(callback));
                        straight_count += 1;
                    }
                }
            } else if let Some(shader) = draw.shader {
                if let Some(callback) = self.renderer.callback_rgba_filtered(
                    rect,
                    shader as usize,
                    image.width,
                    image.height,
                    &image.rgba,
                    draw.linear,
                ) {
                    painter.add(egui::Shape::Callback(callback));
                }
            } else {
                let key = (Arc::as_ptr(image) as usize, draw.linear);
                let options = if draw.linear {
                    egui::TextureOptions::LINEAR
                } else {
                    egui::TextureOptions::NEAREST
                };
                let texture = self.textures.entry(key).or_insert_with(|| {
                    (
                        Arc::downgrade(image),
                        self.context.load_texture(
                            "GamePKG port image",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [image.width as usize, image.height as usize],
                                &image.rgba,
                            ),
                            options,
                        ),
                    )
                });
                painter.image(
                    texture.1.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
        self.straight_cache.finish(straight_count);
        fresh_frame
    }
    fn clear_native(&mut self) {
        if let Some((_, id)) = self.current_native.take() {
            self.state.renderer.write().free_texture(&id);
        }
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        self.clear_native();
    }
}
impl Backend for Desktop {
    fn tick_begin(&mut self) -> Result<()> {
        self.recorded.begin().map_err(|_| Error::Failed)
    }
    fn tick_finish(&mut self) -> Result<()> {
        self.recorded.finish(true).map_err(|_| Error::Failed)
    }
    fn tick_audio(&mut self, operation: u32, name: &str, value: u32) -> bool {
        self.recorded.call(operation, name, value)
    }
    fn tick_homepage(&mut self, url: String) {
        self.context.open_url(egui::OpenUrl::new_tab(url));
    }
    fn revoke_capture(&mut self) {
        self.microphone_allowed = false;
        self.capture.take();
    }
    fn capture(&mut self, operation: u32, bytes: &[u8], capacity: usize) -> Result<Vec<u8>> {
        if !self.microphone_allowed {
            return Err(Error::Denied);
        }
        let mut r = super::protocol::Request(bytes);
        if operation == 140 {
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            let format = r.u32()?;
            let rate = r.u32()?;
            let layout = r.u32()?;
            let frames = r.u32()? as usize;
            r.end()?;
            if format != 0 || rate != 48000 || layout > 1 {
                return Err(Error::Unsupported);
            }
            if self.capture.is_some() {
                return Err(Error::Busy);
            }
            let id = identifier()?;
            let capture =
                crate::audio::capture::Device::new(layout as u16 + 1, frames, self.capture_budget)?;
            self.capture = Some((id, capture));
            return Ok(id.to_le_bytes().to_vec());
        }
        let id = r.u64()?;
        let (_, capture) = self
            .capture
            .as_ref()
            .filter(|(current, _)| *current == id)
            .ok_or(Error::Denied)?;
        if operation == 143 {
            r.end()?;
            self.capture.take();
            return Ok(Vec::new());
        }
        capture.ready()?;
        match operation {
            141 => {
                let frames = r.u32()? as usize;
                r.end()?;
                capture.queue.read(frames, capacity)
            }
            142 => {
                r.end()?;
                if capacity < 32 {
                    return Err(Error::Invalid);
                }
                Ok(capture
                    .queue
                    .status()?
                    .into_iter()
                    .flat_map(u64::to_le_bytes)
                    .collect())
            }
            _ => Err(Error::Unsupported),
        }
    }
    fn audio_streams(&self) -> Option<super::audio_streams::Mixer> {
        self.streams.clone()
    }
    fn graphics(&mut self, operation: u32, request: &[u8], capacity: usize) -> Result<Vec<u8>> {
        if matches!(operation, 105 | 109)
            && (self.pending.is_some() || self.pending_native.is_some())
        {
            return Err(Error::Busy);
        }
        let graphics = self.graphics.as_mut().ok_or(Error::Unsupported)?;
        if matches!(operation, 105 | 109) {
            self.pending_native = Some(if operation == 109 {
                graphics.present_target(request)?
            } else {
                graphics.render(request)?
            });
            self.context.request_repaint();
            Ok(Vec::new())
        } else {
            graphics.call(operation, request, capacity)
        }
    }
    fn texture_limit(&self) -> u32 {
        self.texture_limit
    }
    fn window(&mut self, command: WindowCommand) -> Result<()> {
        use egui::ViewportCommand as V;
        let cmd = match command {
            WindowCommand::Fullscreen(b) => V::Fullscreen(b),
            WindowCommand::CursorVisible(b) => {
                self.cursor_visible = b;
                V::CursorVisible(b)
            }
            WindowCommand::CursorLocked(b) => V::CursorGrab(if b {
                egui::CursorGrab::Locked
            } else {
                egui::CursorGrab::None
            }),
            WindowCommand::Resize(w, h) => {
                V::InnerSize(egui::vec2(w as f32, h as f32) / self.context.pixels_per_point())
            }
            WindowCommand::Close => V::Close,
        };
        self.context.send_viewport_cmd(cmd);
        Ok(())
    }
    fn shader_count(&self) -> u32 {
        self.shader_count
    }
    fn present(&mut self, frame: Frame) -> Result<()> {
        if self.pending.is_some() || self.pending_native.is_some() {
            return Err(Error::Busy);
        }
        self.pending = Some(frame);
        self.context.request_repaint();
        Ok(())
    }
    fn audio(&mut self, samples: &[i16], channels: u32) -> Result<()> {
        let player = self.audio.as_ref().ok_or(Error::Unsupported)?;
        if player.error().is_some() {
            return Err(Error::Failed);
        }
        if player.submit_channels(samples, channels) {
            Ok(())
        } else {
            Err(Error::Busy)
        }
    }
    fn rumble(&mut self, device: u64, low: u16, high: u16, ms: u32) -> Result<()> {
        self.rumble.as_mut().ok_or(Error::Unsupported)?(device, low, high, ms)
    }
}
