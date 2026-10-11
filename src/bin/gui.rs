use clap::Parser;
#[derive(Parser)]
#[command(about = "GamePKG desktop library and grid-v1 player")]
struct Args {
    #[arg(long)]
    library: Option<std::path::PathBuf>,
    #[arg(long)]
    play: Option<String>,
}
fn main() -> eframe::Result {
    let args = Args::parse();
    if let Some(root) = args.library {
        std::env::set_var("GAMEPKG_HOME", root);
    }
    eframe::run_native(
        "GamePKG",
        eframe::NativeOptions::default(),
        Box::new(move |_| Ok(Box::new(gamepkg::gui::App::new(args.play)))),
    )
}
