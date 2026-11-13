//! Native Vulkan integration check. Synthetic power messages affect this
//! process only; the test never changes monitor power or lid settings.
//! Requires an active Windows desktop and an available Vulkan GPU.
#![cfg_attr(windows, windows_subsystem = "windows")]
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use windows_sys::{
    core::GUID,
    Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            SendMessageW, PBT_APMRESUMEAUTOMATIC, PBT_POWERSETTINGCHANGE, WM_DISPLAYCHANGE,
            WM_POWERBROADCAST,
        },
    },
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId, WindowLevel},
};
fn log(s: &str) {
    let p = std::env::var_os("GAMEPKG_RENDER_PROBE_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| "power-cycle.log".into());
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(f, "{s}");
    }
}
const LID: GUID = GUID::from_u128(0xba3e0f4d_b817_4094_a2d1_d56379e6a0f3);
const DISPLAY: GUID = GUID::from_u128(0x6fe69556_704a_47a0_8f24_c28d936fda47);
#[repr(C)]
struct Payload {
    guid: GUID,
    len: u32,
    value: u32,
}
#[derive(Clone, Copy)]
enum Action {
    Lid(u32),
    Display(u32),
    Malformed,
    Resume,
    Topology,
}
const STEPS: &[(&str, Action, u32)] = &[
    ("lid closes", Action::Lid(0), 0),
    ("lid opens", Action::Lid(1), 1),
    ("display off", Action::Display(0), 0),
    ("display on", Action::Display(1), 1),
    ("display dims", Action::Display(2), 0),
    ("invalid lid value", Action::Lid(3), 0),
    ("malformed payload", Action::Malformed, 0),
    ("both close: lid", Action::Lid(0), 0),
    ("both close: display", Action::Display(0), 0),
    ("lid alone returns", Action::Lid(1), 0),
    ("display also returns", Action::Display(1), 1),
    ("system resumes", Action::Resume, 1),
    ("display topology changes", Action::Topology, 1),
    ("duplicate display on", Action::Display(1), 0),
];
struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    start: Instant,
    last: Instant,
    frames: u32,
    resizes: u32,
    step: usize,
    baseline: u32,
    changed: Instant,
    started: bool,
    finished: bool,
    pixels: u32,
}
#[derive(Default)]
struct App {
    state: Option<State>,
    failures: u32,
}
fn action(window: &Window, a: Action) {
    let RawWindowHandle::Win32(h) = window.window_handle().unwrap().as_raw() else {
        panic!("Windows required")
    };
    let hwnd = h.hwnd.get() as HWND;
    unsafe {
        match a {
            Action::Lid(value) | Action::Display(value) => {
                let guid = if matches!(a, Action::Lid(_)) {
                    LID
                } else {
                    DISPLAY
                };
                let p = Payload {
                    guid,
                    len: 4,
                    value,
                };
                SendMessageW(
                    hwnd,
                    WM_POWERBROADCAST,
                    PBT_POWERSETTINGCHANGE as usize,
                    &p as *const _ as isize,
                );
            }
            Action::Malformed => {
                let p = Payload {
                    guid: LID,
                    len: 0,
                    value: 0,
                };
                SendMessageW(
                    hwnd,
                    WM_POWERBROADCAST,
                    PBT_POWERSETTINGCHANGE as usize,
                    &p as *const _ as isize,
                );
            }
            Action::Resume => {
                SendMessageW(hwnd, WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC as usize, 0);
            }
            Action::Topology => {
                SendMessageW(hwnd, WM_DISPLAYCHANGE, 32, 0);
            }
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        let window = Arc::new(
            e.create_window(
                Window::default_attributes()
                    .with_title("GamePKG Vulkan display recovery test")
                    .with_inner_size(winit::dpi::LogicalSize::new(900.0, 600.0))
                    .with_active(false)
                    .with_window_level(WindowLevel::AlwaysOnBottom),
            )
            .unwrap(),
        );
        self.state = Some(pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN,
                ..Default::default()
            });
            let surface = instance.create_surface(window.clone()).unwrap();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                })
                .await
                .unwrap();
            log(&format!("ADAPTER {:?}", adapter.get_info()));
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .unwrap();
            let caps = surface.get_capabilities(&adapter);
            assert!(caps.usages.contains(wgpu::TextureUsages::COPY_SRC));
            let size = window.inner_size();
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                format: wgpu::TextureFormat::Bgra8Unorm,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: vec![],
            };
            assert!(caps.formats.contains(&config.format));
            surface.configure(&device, &config);
            let now = Instant::now();
            State {
                window,
                surface,
                device,
                queue,
                config,
                start: now,
                last: now - Duration::from_secs(1),
                frames: 0,
                resizes: 0,
                step: 0,
                baseline: 0,
                changed: now,
                started: false,
                finished: false,
                pixels: 0,
            }
        }));
    }
    fn window_event(&mut self, e: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(s) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => {
                self.failures += 1;
                e.exit();
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    s.resizes += 1;
                    s.config.width = size.width;
                    s.config.height = size.height;
                    s.surface.configure(&s.device, &s.config);
                    log(&format!(
                        "RESIZE count{} phase{} size{}x{}",
                        s.resizes, s.step, size.width, size.height
                    ));
                }
            }
            WindowEvent::RedrawRequested => {
                let frame = s.surface.get_current_texture().unwrap();
                let view = frame.texture.create_view(&Default::default());
                let mut enc = s.device.create_command_encoder(&Default::default());
                {
                    let _p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.05,
                                    g: 0.25,
                                    b: 0.8,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..Default::default()
                    });
                }
                let capture = if s.started && s.changed.elapsed() > Duration::from_millis(250) {
                    let row = (s.config.width * 4).div_ceil(256) * 256;
                    let b = s.device.create_buffer(&wgpu::BufferDescriptor {
                        label: None,
                        size: u64::from(row) * u64::from(s.config.height),
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    });
                    enc.copy_texture_to_buffer(
                        frame.texture.as_image_copy(),
                        wgpu::TexelCopyBufferInfo {
                            buffer: &b,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(row),
                                rows_per_image: None,
                            },
                        },
                        wgpu::Extent3d {
                            width: s.config.width,
                            height: s.config.height,
                            depth_or_array_layers: 1,
                        },
                    );
                    Some((b, row))
                } else {
                    None
                };
                s.queue.submit([enc.finish()]);
                if let Some((b, row)) = capture {
                    let (tx, rx) = std::sync::mpsc::channel();
                    b.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                        let _ = tx.send(r);
                    });
                    s.device.poll(wgpu::Maintain::Wait);
                    rx.recv().unwrap().unwrap();
                    {
                        let d = b.slice(..).get_mapped_range();
                        let at = (s.config.height / 2 * row + s.config.width / 2 * 4) as usize;
                        let p = &d[at..at + 4];
                        if p != [204, 64, 13, 255] {
                            self.failures += 1;
                            log(&format!("BAD PIXEL {p:?}"));
                        }
                        s.pixels += 1;
                    }
                    b.unmap();
                }
                s.window.pre_present_notify();
                frame.present();
                s.frames += 1;
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, e: &ActiveEventLoop) {
        let Some(s) = self.state.as_mut() else { return };
        let now = Instant::now();
        if !s.started && s.start.elapsed() > Duration::from_secs(2) {
            s.started = true;
            s.baseline = s.resizes;
            s.changed = now;
            log(&format!("ACTION {}", STEPS[0].0));
            action(&s.window, STEPS[0].1);
        } else if s.started && !s.finished && s.changed.elapsed() > Duration::from_millis(700) {
            let (name, _, expected) = STEPS[s.step];
            let actual = s.resizes - s.baseline;
            log(&format!(
                "RESULT {name}: resizes{actual} expected{expected}"
            ));
            if actual != expected {
                self.failures += 1;
            }
            s.step += 1;
            s.baseline = s.resizes;
            s.changed = now;
            if s.step == STEPS.len() {
                s.finished = true;
                log(&format!(
                    "COMPLETE steps{} pixels{} frames{} failures{}",
                    s.step, s.pixels, s.frames, self.failures
                ));
                e.exit();
            } else {
                log(&format!("ACTION {}", STEPS[s.step].0));
                action(&s.window, STEPS[s.step].1);
            }
        }
        if s.last.elapsed() > Duration::from_millis(80) {
            s.window.request_redraw();
            s.last = now;
        }
        e.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(20)));
    }
}
fn main() {
    std::panic::set_hook(Box::new(|p| log(&format!("PANIC {p}"))));
    log("START native Vulkan notification cycle; synthetic messages affect this test process only");
    let mut app = App::default();
    EventLoop::new().unwrap().run_app(&mut app).unwrap();
    let covered = app
        .state
        .as_ref()
        .is_some_and(|s| s.finished && s.pixels >= STEPS.len() as u32);
    if !covered || app.failures != 0 {
        log("FAIL");
        std::process::exit(1);
    }
    log("PASS");
}
