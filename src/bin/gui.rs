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
    if let Some(id) = args.play.as_deref() {
        if let Ok(root) = gamepkg::registry::default_root() {
            if gamepkg::registry::installed_manifest(&root, id)
                .ok()
                .is_some_and(|manifest| {
                    manifest
                        .builtin_engine
                        .as_deref()
                        .and_then(gamepkg::external_engine::get)
                        .is_some()
                })
            {
                if args.target.as_deref() == Some("wasm") {
                    return Err(eframe::Error::AppCreation(Box::new(std::io::Error::other(
                        "this external engine has no Wasm build. Play it with the native target.",
                    ))));
                }
                if !args.launch.is_empty() {
                    return Err(eframe::Error::AppCreation(Box::new(std::io::Error::other(
                        "Graphics launch flags currently require the reviewed native Takedawn engine",
                    ))));
                }
                gamepkg::external_engine::launch(&root, id).map_err(|error| {
                    eframe::Error::AppCreation(Box::new(std::io::Error::other(error.to_string())))
                })?;
                return Ok(());
            }
        }
    }
    if let Some(renderer) = args.renderer {
        std::env::set_var("GAMEPKG_RENDERER", renderer);
    }
    let settings = gamepkg::settings::load().map_err(|error| {
        eframe::Error::AppCreation(Box::new(std::io::Error::other(error.to_string())))
    })?;
    #[cfg(any(target_os = "linux", windows))]
    if let Some(id) = &args.play {
        let launch = || -> anyhow::Result<bool> {
            let root = gamepkg::registry::default_root()?;
            let manifest = gamepkg::registry::installed_manifest(&root, id)?;
            if manifest.builtin_engine.as_deref() != Some(gamepkg::takedawn::PROFILE) {
                anyhow::ensure!(
                    args.launch.is_empty(),
                    "Graphics launch flags currently require the reviewed native Takedawn engine"
                );
                return Ok(false);
            }
            anyhow::ensure!(
                args.target.as_deref() != Some("wasm"),
                "Takedawn requires native Play"
            );
            gamepkg::takedawn::play_with_launch(&root, id, &[], &settings, None, args.launch)?;
            Ok(true)
        };
        if launch().map_err(|error| {
            eframe::Error::AppCreation(Box::new(std::io::Error::other(format!("{error:#}"))))
        })? {
            return Ok(());
        }
    }
    let mut wgpu_options = eframe::egui_wgpu::WgpuSetupCreateNew::default();
    wgpu_options.instance_descriptor.backends = eframe::wgpu::Backends::VULKAN;
    let title = args
        .play
        .clone()
        .unwrap_or_else(|| concat!("GamePKG ", env!("CARGO_PKG_VERSION")).into());
    let window_preferences = args.play.as_ref().and_then(|id| {
        let open = || -> anyhow::Result<gamepkg::window_preferences::Session> {
            let root = gamepkg::registry::default_root()?;
            let manifest = gamepkg::registry::installed_manifest(&root, id)?;
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
    eframe::run_native(
        &title,
        eframe::NativeOptions {
            vsync: args.play.is_none() || preferences.vsync,
            // UI dithering perturbs colors before blending. Game pixels use
            // the encoded RGBA contract shared with SDL and the browser.
            dithering: args.play.is_none(),
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
                wgpu_setup: wgpu_options.into(),
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
            let mut app = gamepkg::gui::App::new_with_target(
                args.play,
                cc.gl.clone(),
                args.target.as_deref().map(|t| t == "native"),
            );
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
