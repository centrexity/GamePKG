use clap::Parser;
#[derive(Parser)]
#[command(version, about = "GamePKG desktop library and game player")]
struct Args {
    #[arg(long, hide = true)]
    check_bundled_audio: bool,
    #[arg(long)]
    library: Option<std::path::PathBuf>,
    #[arg(long)]
    play: Option<String>,
    #[arg(long, requires="play", value_parser=["native","wasm"])]
    target: Option<String>,
    #[arg(long,value_parser=["vulkan","opengl"],help="Choose the desktop renderer explicitly")]
    renderer: Option<String>,
    #[arg(long, conflicts_with = "play", help = "Open the project support page")]
    support: bool,
}
fn main() -> eframe::Result {
    let args = Args::parse();
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
    let mut wgpu_options = eframe::egui_wgpu::WgpuSetupCreateNew::default();
    wgpu_options.instance_descriptor.backends = eframe::wgpu::Backends::VULKAN;
    let title = args.play.clone().unwrap_or_else(|| "GamePKG".into());
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
            renderer: if settings.renderer == "opengl" {
                eframe::Renderer::Glow
            } else {
                eframe::Renderer::Wgpu
            },
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
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
