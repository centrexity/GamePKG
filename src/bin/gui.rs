#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use clap::Parser;
#[derive(Parser)]
#[command(version, about = "GamePKG desktop library and game player")]
struct Args {
    #[arg(long, hide = true)]
    check_bundled_audio: bool,
    #[arg(long, hide = true)]
    check_build_identity: bool,
    #[arg(long)]
    library: Option<std::path::PathBuf>,
    #[arg(long, help = "Use a specific GamePKG settings file")]
    config: Option<std::path::PathBuf>,
    #[arg(long)]
    play: Option<String>,
    #[command(flatten)]
    launch: gamepkg::launch_options::LaunchArgs,
    #[arg(long, requires="play", value_parser=["native","wasm"])]
    target: Option<String>,
    #[arg(long,value_parser=["vulkan","opengl"],help="Choose the desktop renderer explicitly")]
    renderer: Option<String>,
    #[arg(long, conflicts_with = "play", help = "Open the project support page")]
    support: bool,
}
/// Prepare the Vulkan instance, adapter and device before eframe starts window
/// setup. Passing them as `WgpuSetup::Existing` leaves eframe with surface and
/// pipeline setup instead of another adapter request and device creation.
fn preinit_vulkan(
    setup: eframe::egui_wgpu::WgpuSetupCreateNew,
) -> Option<eframe::egui_wgpu::WgpuSetupExisting> {
    let instance =
        pollster::block_on(eframe::egui_wgpu::WgpuSetup::CreateNew(setup.clone()).new_instance());
    let adapter = pollster::block_on(instance.request_adapter(
        &eframe::wgpu::RequestAdapterOptions {
            power_preference: setup.power_preference,
            compatible_surface: None,
            force_fallback_adapter: false,
        },
    ))?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &(setup.device_descriptor)(&adapter),
        setup.trace_path.as_deref(),
    ))
    .ok()?;
    Some(eframe::egui_wgpu::WgpuSetupExisting {
        instance,
        adapter,
        device,
        queue,
    })
}

fn main() -> eframe::Result {
    let args = Args::parse();
    if !args.launch.is_empty() && args.play.is_none() {
        use clap::error::ErrorKind;
        clap::Error::raw(
            ErrorKind::MissingRequiredArgument,
            "graphics launch flags require --play <GAME_ID>",
        )
        .exit();
    }
    if args.check_build_identity {
        println!("{}", gamepkg::native::build_identity());
        return Ok(());
    }
    if args.check_bundled_audio {
        let factory = gamepkg::audio_decode::Factory::bundled().map_err(|error| {
            eframe::Error::AppCreation(Box::new(std::io::Error::other(error.to_string())))
        })?;
        println!(
            "{}",
            serde_json::json!({
                "sha256": gamepkg::audio_decode::Factory::bundled_digest().unwrap(),
                "maximum_memory_bytes": factory.maximum_memory_bytes(),
            })
        );
        return Ok(());
    }
    if let Some(config) = args.config {
        std::env::set_var("GAMEPKG_CONFIG", config);
    }
    if let Some(root) = args.library {
        std::env::set_var("GAMEPKG_HOME", root);
    }
    if let Some(renderer) = args.renderer {
        std::env::set_var("GAMEPKG_RENDERER", renderer);
    }
    let settings = gamepkg::settings::load().map_err(|error| {
        eframe::Error::AppCreation(Box::new(std::io::Error::other(error.to_string())))
    })?;
    if let Some(id) = &args.play {
        let launch = || -> anyhow::Result<bool> {
            if args.target.as_deref() == Some("wasm") {
                anyhow::ensure!(
                    args.launch.is_empty(),
                    "Graphics launch flags require native Play"
                );
                return Ok(false);
            }
            let root = gamepkg::registry::default_root()?;
            gamepkg::native::play_services(&root, id, &[], &settings, args.launch)?;
            Ok(true)
        };
        if launch().map_err(|error| {
            eframe::Error::AppCreation(Box::new(std::io::Error::other(format!("{error:#}"))))
        })? {
            return Ok(());
        }
    }
    let mut wgpu_create = eframe::egui_wgpu::WgpuSetupCreateNew::default();
    wgpu_create.instance_descriptor.backends = eframe::wgpu::Backends::VULKAN;
    let vulkan_preinit = if settings.renderer == "opengl" {
        None
    } else {
        let create = wgpu_create.clone();
        Some(std::thread::spawn(move || preinit_vulkan(create)))
    };
    let title = args
        .play
        .clone()
        .unwrap_or_else(|| concat!("GamePKG ", env!("CARGO_PKG_VERSION")).into());
    let window_preferences = args.play.as_ref().and_then(|id| {
        let open = || -> anyhow::Result<gamepkg::window_preferences::Session> {
            let root = gamepkg::registry::default_root()?;
            let manifest = gamepkg::registry::installed_signed_identity(&root, id)?;
            gamepkg::window_preferences::Session::open(&root, &manifest.id, &manifest.creator_key)
        };
        match open() {
            Ok(session) => Some(session),
            Err(error) => {
                eprintln!("Unable to load game window settings; using defaults: {error:#}");
                None
            }
        }
    });
    let preferences = window_preferences
        .as_ref()
        .map(|session| session.preferences)
        .unwrap_or_default();
    let wgpu_setup = vulkan_preinit
        .and_then(|handle| handle.join().ok().flatten())
        .map(Into::into)
        .unwrap_or_else(|| wgpu_create.into());
    eframe::run_native(
        &title,
        eframe::NativeOptions {
            vsync: args.play.is_none() || preferences.vsync,
            // Dithering perturbs colors before blending and adds extra shader
            // work before the first frame. Game pixels use the encoded RGBA
            // contract shared with SDL and the browser.
            dithering: false,
            renderer: if settings.renderer == "opengl" {
                eframe::Renderer::Glow
            } else {
                eframe::Renderer::Wgpu
            },
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                present_mode: if args.play.is_none() || preferences.vsync {
                    eframe::wgpu::PresentMode::Fifo
                } else {
                    eframe::wgpu::PresentMode::AutoNoVsync
                },
                wgpu_setup,
                ..Default::default()
            },
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size(if args.play.is_some() {
                    [preferences.width as f32, preferences.height as f32]
                } else {
                    [1280.0, 820.0]
                })
                .with_fullscreen(args.play.is_some() && preferences.fullscreen)
                .with_maximized(args.play.is_some() && preferences.maximized)
                .with_clamp_size_to_monitor_size(true)
                .with_min_inner_size(if args.play.is_some() {
                    [320.0, 240.0]
                } else {
                    [620.0, 480.0]
                })
                .with_icon(gamepkg::gui::window_icon())
                .with_drag_and_drop(args.play.is_none()),
            ..Default::default()
        },
        Box::new(move |cc| {
            let mut app = gamepkg::gui::App::new_with_target(args.play, cc.gl.clone(), Some(false));
            app.set_native_display(cc);
            app.set_window_preferences(window_preferences);
            app.set_vulkan(cc.wgpu_render_state.clone());
            if args.support {
                app.open_support();
            }
            Ok(Box::new(app))
        }),
    )
}
