use clap::Parser;
#[derive(Parser)]
#[command(version, about = "GamePKG desktop library and game player")]
struct Args {
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
    if let Some(root) = args.library {
        std::env::set_var("GAMEPKG_HOME", root);
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
                    [960.0, 720.0]
                } else {
                    [1280.0, 820.0]
                })
                .with_min_inner_size(if args.play.is_some() {
                    [320.0, 240.0]
                } else {
                    [620.0, 480.0]
                })
                .with_icon(gamepkg::gui::window_icon())
                .with_drag_and_drop(true),
            ..Default::default()
        },
        Box::new(move |cc| {
            let mut app = gamepkg::gui::App::new_with_target(
                args.play,
                cc.gl.clone(),
                args.target.as_deref().map(|t| t == "native"),
            );
            app.set_vulkan(cc.wgpu_render_state.clone());
            if args.support {
                app.open_support();
            }
            Ok(Box::new(app))
        }),
    )
}
