#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use clap::Parser;
    use gamepkg::port_host::{
        admission::CheckedSources,
        owner::VerifiedAssets,
        preparation::{self, LocalLayout, Plan, Target},
        process::{NativeLaunch, Process},
    };
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };

    #[derive(Parser)]
    struct Args {
        source: PathBuf,
        library: PathBuf,
        #[arg(long)]
        smoke: bool,
    }
    let args = Args::parse();
    std::fs::create_dir_all(&args.library)?;
    let library = args.library.canonicalize()?;
    let saves = library.join("saves");
    std::fs::create_dir_all(&saves)?;
    let temporary = tempfile::tempdir()?;
    let package_path = if args.source.is_file() {
        args.source
    } else {
        let path = temporary.path().join("game.gamepkg");
        let mut key = [0; 32];
        getrandom::fill(&mut key).map_err(|error| anyhow::anyhow!("{error}"))?;
        gamepkg::pack::pack(&args.source, &path, &key)?;
        path
    };
    let mut package = gamepkg::archive::Package::open(&package_path)?;
    let source = CheckedSources::verify(&mut package)?;
    let assets = VerifiedAssets::stage(&mut package, source.identity())?;
    ensure!(
        source.manifest().required_rom_count() == 0,
        "the 2D tick test does not supply ROMs"
    );
    let plan = Plan::new(
        Target::Linux64,
        "game_tick",
        vec![],
        LocalLayout::None,
        true,
    )?;
    let artifact = preparation::build_in_process(&library, &source, &plan, None)
        .context("build the native game with the GamePKG compatibility layer")?;
    let receipt = artifact.receipt();
    ensure!(
        receipt.game.source_kit == source.identity(),
        "compiled source kit differs"
    );
    let mut process = Process::start_in_process_with_assets(
        &artifact,
        &library,
        Vec::new(),
        source
            .manifest()
            .permissions
            .save
            .then_some((saves.as_path(), 2 * 1024 * 1024)),
        assets.as_ref(),
        NativeLaunch {
            visible: !args.smoke,
            mute_audio: args.smoke,
            graphics: Default::default(),
            saved: None,
            volume: 50,
        },
    )?;
    let start = Instant::now();
    loop {
        if let Some(status) = process.poll()? {
            ensure!(
                status == 0,
                "game exited with status {status}: {}",
                process.diagnostics()
            );
            ensure!(!args.smoke, "game exited before its first frame");
            return Ok(());
        }
        if args.smoke && process.first_frame_presented() {
            println!(
                "PASS: {} presented from its own audited native process",
                source.manifest().id
            );
            process.stop()?;
            return Ok(());
        }
        ensure!(
            process.first_frame_presented() || start.elapsed() < Duration::from_secs(45),
            "initial frame timed out: {}",
            process.diagnostics()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("the game-owned native tick fixture requires Linux x64")
}
