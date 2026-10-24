use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};
#[derive(Parser)]
#[command(
    version,
    about = "Signed source-first game packages (foundation preview)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Install the standalone Windows tools used by the native engine.
    #[cfg(windows)]
    SetupNativeTools,
    /// Verify the supported Xbox Burnout 3 disc directly from an ISO; extract nothing.
    CheckXboxDisc {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Read-only check of literal startup-script includes in selected original game data.
    CheckGameData {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Maintainer packaging check; never reads a runtime decoder path.
    #[command(hide = true)]
    CheckBundledAudio,
    /// Remember a display preference for the pinned Zelda engine.
    ConfigureDisplay {
        id: String,
        #[arg(long, value_parser=["default", "original", "wide", "widest"])]
        preset: String,
        #[arg(long)]
        extended_height: bool,
        #[arg(long)]
        extra_sprites: bool,
        #[arg(long)]
        dim_flashes: bool,
        #[arg(long)]
        skip_intro: bool,
        /// Enable an original Zelda enhancement by its documented name; repeat for more.
        #[arg(long)]
        enhancement: Vec<String>,
    },
    /// Remember the next multiplayer server or hosting address for a supported signed game.
    ConfigureNetwork {
        id: String,
        #[arg(long)]
        server: Option<String>,
        #[arg(long)]
        master: Option<String>,
        #[arg(long)]
        listen: Option<String>,
        #[arg(long, hide = true)]
        allow_local: bool,
        #[arg(long,conflicts_with_all=["server","master","listen","allow_local"])]
        clear: bool,
    },
    /// List saved demos and movies for an installed game.
    Recordings {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Import a selected demo or AVI without replacing existing saved files. Close the game first.
    RecordingImport {
        id: String,
        source: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Copy one saved recording to a new file; existing files are never replaced.
    RecordingExport {
        id: String,
        name: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Internal verified compiler response; requires an installed signed package.
    #[command(hide = true)]
    AcceptBuildWorker { id: String, exchange: PathBuf },
    #[command(hide = true)]
    PrepareBuildWorker { id: String, output: PathBuf },
    /// Internal isolated compiler exchange; not an installable game format.
    #[command(hide = true)]
    BuildWorker {
        store: PathBuf,
        output: PathBuf,
        #[arg(long, default_value = "wgsl")]
        shader_backend: String,
        #[arg(long)]
        no_optimize: bool,
        #[arg(long)]
        copy_output: bool,
    },
    /// Export a built game as a self-contained browser player.
    WebExport {
        id: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Open the local GamePKG browser library.
    WebLibrary {
        #[arg(long, default_value_t = 39173)]
        port: u16,
    },
    /// Verify installed signed contents, shared assets and compiled game checksums.
    IntegrityCheck { id: String },
    /// Rebuild compiled files from verified source; never alter signatures or player data.
    IntegrityFix {
        id: String,
        #[arg(long,value_parser=["native","wasm"],default_value="native")]
        target: String,
        /// Restore missing or damaged shared assets from the original verified store bundle.
        #[arg(long)]
        store: Option<PathBuf>,
    },
    /// Bundle signed packages and their shared blobs for portable distribution.
    StoreExport {
        #[arg(required = true)]
        packages: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Verify a complete store bundle and list its creator identities.
    StoreInspect { source: String },
    /// Import a verified store bundle from a file, HTTPS, or Tor URL.
    StoreImport {
        source: String,
        #[arg(long)]
        trust_creators: bool,
    },
    /// Add one local asset blob to the shared store and print its hash and size.
    PoolPut { file: PathBuf },
    /// Add verified package assets to the shared content-addressed store.
    PoolImport { package: PathBuf },
    /// Verify a shared blob and copy it to a new output file.
    PoolRead {
        sha256: String,
        #[arg(long)]
        size: u64,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Validate and decode a package preview without running its game.
    Preview {
        package: PathBuf,
        #[arg(long)]
        theora: bool,
    },
    /// Create a private signing key and print its public creator key.
    Keygen {
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Play an installed game, or run N headless frames.
    Run {
        id: String,
        /// Explicit runtime choice; otherwise use the installed native build.
        #[arg(long,value_parser=["native","wasm"])]
        target: Option<String>,
        /// Restore a Zelda snapshot slot before a headless run.
        #[arg(long,requires="frames",value_parser=clap::value_parser!(u8).range(1..=20))]
        load_slot: Option<u8>,
        /// Replace a Zelda snapshot slot after a successful headless run.
        #[arg(long,requires="frames",value_parser=clap::value_parser!(u8).range(1..=20))]
        save_slot: Option<u8>,
        /// Remember an original installation folder for this signed game.
        #[arg(long)]
        game_data: Option<PathBuf>,
        #[arg(long)]
        frames: Option<u32>,
        /// Read-only ROM file; repeat in manifest order for games requiring multiple files.
        #[arg(long)]
        rom: Vec<PathBuf>,
    },
    /// Import a .gamepkglist without downloading games.
    ImportList { source: String },
    /// Download and install one catalog game (first install needs approval).
    Download {
        id: String,
        #[arg(long)]
        trust_creator: bool,
    },
    /// Compile an installed game for this computer (other targets are explicit).
    Build {
        id: String,
        #[arg(long, value_parser = ["native", "wasm"], default_value="native")]
        target: String,
        /// Build a source directory into a signed package after isolated compilation succeeds.
        #[arg(short, long, requires = "key")]
        output: Option<PathBuf>,
        #[arg(long, requires = "output")]
        key: Option<PathBuf>,
    },
    /// Inspect a signed update without installing it.
    CheckUpdates { id: String },
    /// List installed packages.
    List,
    /// Install a local file or download an HTTPS or Tor onion URL.
    Install {
        source: String,
        #[arg(long)]
        trust_creator: bool,
    },
    /// Download and install a newer package from its signed update URL.
    Update { id: String },
    /// Validate and sign a package directory. This does not compile the game.
    Pack {
        dir: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        key: PathBuf,
        /// Keep assets in the shared store and sign their references in the manifest.
        #[arg(long)]
        shared_assets: bool,
    },
    /// Verify the embedded creator signature; optionally require a known public key.
    Verify {
        package: PathBuf,
        #[arg(long)]
        trusted_key: Option<String>,
    },
    /// Read an asset from a verified archive without extraction.
    Asset {
        package: PathBuf,
        path: String,
        #[arg(long)]
        trusted_key: Option<String>,
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn key(s: &str) -> Result<[u8; 32]> {
    hex::decode(s.trim())?
        .try_into()
        .map_err(|_| anyhow::anyhow!("key must contain 32 bytes"))
}
fn main() -> Result<()> {
    match Cli::parse().command {
        #[cfg(windows)]
        Command::SetupNativeTools => {
            let sdk =
                gamepkg::takedawn::prepare_windows_tools(&gamepkg::registry::default_root()?)?;
            println!(
                "Verified native Windows build tools ready: {}",
                sdk.display()
            );
        }
        Command::CheckXboxDisc { path, json } => {
            let report = gamepkg::xbox_disc::Disc::open(&path)?.verify_takedawn()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Usable supported disc: {} — {}", report.game, report.build);
                println!(
                    "Verified {} files ({} bytes) directly in the image; nothing extracted.",
                    report.verified_files, report.verified_bytes
                );
                println!("Select this ISO on the game's page, build the native game, then Play. The ISO stays in place.");
            }
        }
        Command::CheckBundledAudio => {
            let factory = gamepkg::audio_decode::Factory::bundled()?;
            println!(
                "{}",
                serde_json::json!({
                    "sha256": gamepkg::audio_decode::Factory::bundled_digest().unwrap(),
                    "maximum_memory_bytes": factory.maximum_memory_bytes(),
                })
            );
        }
        Command::PrepareBuildWorker { id, output } => {
            gamepkg::build_worker::prepare(&gamepkg::registry::default_root()?, &id, &output)?;
        }
        Command::AcceptBuildWorker { id, exchange } => {
            gamepkg::build_worker::accept(&gamepkg::registry::default_root()?, &id, &exchange)?;
        }
        Command::BuildWorker {
            store,
            output,
            shader_backend,
            no_optimize,
            copy_output,
        } => {
            let compile = if copy_output {
                gamepkg::build_worker::compile_transport
            } else {
                gamepkg::build_worker::compile
            };
            compile(&store, &output, &shader_backend, !no_optimize)?;
        }
        Command::WebExport { id, output } => {
            gamepkg::web_export::export(&gamepkg::registry::default_root()?, &id, &output)?;
            println!("Web player exported to {}", output.display());
        }
        Command::WebLibrary { port } => {
            gamepkg::web_library::serve(&gamepkg::registry::default_root()?, port)?
        }
        Command::IntegrityCheck { id } => println!(
            "{}",
            gamepkg::integrity::check(&gamepkg::registry::default_root()?, &id)?
        ),
        Command::IntegrityFix { id, store, target } => println!(
            "{}",
            gamepkg::integrity::fix_target(
                &gamepkg::registry::default_root()?,
                &id,
                store.as_deref(),
                target == "native"
            )?
        ),
        Command::StoreExport { packages, output } => {
            gamepkg::store::export(&gamepkg::registry::default_root()?, &packages, &output)?;
            println!("Created {}", output.display());
        }
        Command::StoreInspect { source } => {
            let downloaded;
            let path = if source.contains("://") {
                downloaded = gamepkg::registry::download_store(&source)?;
                downloaded.path()
            } else {
                std::path::Path::new(&source)
            };
            let bundle = gamepkg::store::Bundle::open(path)?;
            for game in bundle.games() {
                println!(
                    "{} {} — creator {}",
                    game.id, game.version, game.creator_key
                );
            }
            println!(
                "Review creator identities, then use store-import --trust-creators to install."
            );
        }
        Command::StoreImport {
            source,
            trust_creators,
        } => {
            let downloaded;
            let path = if source.contains("://") {
                downloaded = gamepkg::registry::download_store(&source)?;
                downloaded.path()
            } else {
                std::path::Path::new(&source)
            };
            let bundle = gamepkg::store::Bundle::open(path)?;
            let count = bundle.install(&gamepkg::registry::default_root()?, trust_creators)?;
            println!("Installed {count} store games");
        }
        Command::PoolPut { file } => {
            use std::io::Read;
            let input = fs::File::open(file)?;
            if !input.metadata()?.is_file() {
                anyhow::bail!("asset must be a regular file");
            }
            let mut bytes = Vec::new();
            input
                .take(gamepkg::archive::MAX_FILE + 1)
                .read_to_end(&mut bytes)?;
            let reference =
                gamepkg::pool::Pool::open(&gamepkg::registry::default_root()?)?.put(&bytes)?;
            println!("{} {}", reference.sha256, reference.size);
        }
        Command::PoolImport { package } => {
            let pool = gamepkg::pool::Pool::open(&gamepkg::registry::default_root()?)?;
            let references = pool.import_package(&package)?;
            for (path, reference) in references {
                println!("{} {} {}", reference.sha256, reference.size, path);
            }
        }
        Command::PoolRead {
            sha256,
            size,
            output,
        } => {
            let pool = gamepkg::pool::Pool::open(&gamepkg::registry::default_root()?)?;
            let bytes = pool.read(&gamepkg::pool::Reference { sha256, size })?;
            use std::io::Write;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?
                .write_all(&bytes)?;
        }
        Command::Preview { package, theora } => {
            let mut pkg = gamepkg::archive::Package::open(&package)?;
            gamepkg::verify::verify_embedded(&mut pkg)?;
            let name = if !theora && pkg.entries.contains_key("preview.av1.webm") {
                "preview.av1.webm"
            } else {
                "preview.ogv"
            };
            let frames = gamepkg::previews::decode(name, &pkg.read(name)?)?;
            println!("Decoded {} preview frames (320×240, 5 fps)", frames.len());
        }
        Command::Recordings { id, json } => {
            let entries =
                gamepkg::cube2::recordings::list(&gamepkg::registry::default_root()?, &id)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else if entries.is_empty() {
                println!("No saved recordings.");
            } else {
                for entry in entries {
                    println!("{}  {} bytes  {}", entry.kind, entry.bytes, entry.name);
                }
            }
        }
        Command::RecordingImport { id, source, name } => {
            let entry = gamepkg::cube2::recordings::import(
                &gamepkg::registry::default_root()?,
                &id,
                &source,
                name.as_deref(),
            )?;
            println!("Imported {} ({} bytes).", entry.name, entry.bytes);
        }
        Command::RecordingExport { id, name, output } => {
            let bytes = gamepkg::cube2::recordings::export(
                &gamepkg::registry::default_root()?,
                &id,
                &name,
                &output,
            )?;
            println!("Exported {bytes} bytes to {}", output.display());
        }
        Command::CheckGameData { path, json } => {
            #[cfg(target_os = "linux")]
            {
                let report = gamepkg::cube2::content::audit(&path)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!(
                        "Checked {} scripts for line-leading literal includes.",
                        report.checked_scripts
                    );
                    for issue in &report.missing_or_unreadable {
                        println!(
                            "Missing/unreadable: {} (from {}): {}",
                            issue.reference, issue.script, issue.reason
                        );
                    }
                    for issue in &report.upstream_missing_references {
                        println!("Known upstream stale include: {} (from {}). Original texture browser is built into the engine.", issue.reference,issue.script);
                    }
                    for issue in &report.unresolved {
                        println!("Unresolved: {}: {}", issue.script, issue.reason);
                    }
                    println!("Dynamic commands, conditional execution, complete media and matching asset versions are not verified by this check.");
                }
                anyhow::ensure!(
                    report.missing_or_unreadable.is_empty(),
                    "selected game data has missing or unreadable referenced scripts"
                );
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (path, json);
                anyhow::bail!("original game-data auditing currently requires Linux");
            }
        }
        Command::ConfigureDisplay {
            id,
            preset,
            extended_height,
            extra_sprites,
            dim_flashes,
            skip_intro,
            enhancement,
        } => {
            anyhow::ensure!(
                preset != "default"
                    || (!extended_height
                        && !extra_sprites
                        && !dim_flashes
                        && !skip_intro
                        && enhancement.is_empty()),
                "package default cannot override display options"
            );
            let mut video = match preset.as_str() {
                "default" => None,
                "original" => Some(gamepkg::manifest::ZeldaVideo {
                    extra_width: 0,
                    extend_height: extended_height,
                    no_sprite_limits: false,
                    dim_flashes: false,
                    skip_intro_on_keypress: false,
                    enhancements: Default::default(),
                }),
                "wide" => Some(gamepkg::video_selections::wide(extended_height)),
                "widest" => Some(gamepkg::manifest::ZeldaVideo {
                    extra_width: 96,
                    extend_height: extended_height,
                    no_sprite_limits: false,
                    dim_flashes: false,
                    skip_intro_on_keypress: false,
                    enhancements: Default::default(),
                }),
                _ => unreachable!(),
            };
            if let Some(video) = video.as_mut() {
                video.no_sprite_limits = extra_sprites;
                video.dim_flashes = dim_flashes;
                video.skip_intro_on_keypress = skip_intro;
                for name in &enhancement {
                    video.enhancements.set(name, true)?;
                }
            }
            gamepkg::video_selections::remember_for_game(
                &gamepkg::registry::default_root()?,
                &id,
                video,
            )?;
            println!("Display settings saved for the next launch.");
        }
        Command::ConfigureNetwork {
            id,
            server,
            master,
            listen,
            allow_local,
            clear,
        } => {
            anyhow::ensure!(
                clear || server.is_some() || master.is_some() || listen.is_some(),
                "provide --server, --master, --listen, or --clear"
            );
            let selection = gamepkg::server_selections::Selection {
                server: server.unwrap_or_default(),
                master: master.unwrap_or_default(),
                listen: listen.unwrap_or_default(),
                allow_local,
            };
            gamepkg::server_selections::remember_for_game(
                &gamepkg::registry::default_root()?,
                &id,
                &selection,
            )?;
            println!("Multiplayer launch settings saved. Blank addresses clear the launch commands; the package networking permission still applies.");
        }
        Command::Keygen { output } => {
            let public = gamepkg::creator::generate(&output)?;
            println!("Created private signing key: {}", output.display());
            println!("Creator public key: {public}");
        }
        Command::Run {
            id,
            target,
            load_slot,
            save_slot,
            frames,
            rom,
            game_data,
        } => {
            if let Some(path) = game_data {
                let root = gamepkg::registry::default_root()?;
                gamepkg::game_data::remember_for_game(&root, &id, &path)?;
            }
            let root = gamepkg::registry::default_root()?;
            #[cfg(any(target_os = "linux", windows))]
            if gamepkg::registry::installed_manifest(&root, &id)?
                .builtin_engine
                .as_deref()
                == Some(gamepkg::takedawn::PROFILE)
            {
                anyhow::ensure!(
                    target.as_deref() != Some("wasm"),
                    "Takedawn requires native Run"
                );
                anyhow::ensure!(
                    load_slot.is_none() && save_slot.is_none(),
                    "Takedawn uses its original profile saves"
                );
                gamepkg::takedawn::play(&root, &id, &rom, &gamepkg::settings::load()?, frames)?;
                if let Some(count) = frames {
                    println!("Ran {count} native frames.");
                }
                return Ok(());
            }
            let mut game = gamepkg::runtime::Game::load_target(
                &root,
                &id,
                &rom,
                target.as_deref().map(|target| target == "native"),
            )?;
            if load_slot.is_some() || save_slot.is_some() {
                anyhow::ensure!(
                    game.supports_zelda_snapshot_slots(),
                    "this game does not support Zelda snapshot slots"
                );
            }
            if let Some(slot) = load_slot {
                game.load_zelda_slot(&root, slot - 1)?;
            }
            if let Some(count) = frames {
                if count > 10000 {
                    anyhow::bail!("headless runs are limited to 10000 frames");
                }
                for _ in 0..count {
                    game.step(0)?;
                    if game.exited() {
                        break;
                    }
                }
                if let Some(slot) = save_slot {
                    game.save_zelda_slot(&root, slot - 1)?;
                }
                if game.custom_renderer {
                    println!(
                        "Ran {count} frames; draw commands: {}",
                        game.frame().commands.len()
                    );
                } else {
                    println!("Ran {count} frames; score: {}", game.score());
                }
                if game.audio_allowed {
                    println!(
                        "Audio samples: {}; peak: {}",
                        game.audio_frame().len(),
                        game.audio_frame()
                            .iter()
                            .map(|&s| (s as i32).abs())
                            .max()
                            .unwrap_or(0)
                    );
                }
            } else {
                if game.custom_renderer {
                    anyhow::bail!("this game draws graphics; open it in gamepkg-gui --play {id}, or use --frames for a headless run");
                }
                #[cfg(feature = "audio")]
                let audio = {
                    let settings = gamepkg::settings::load()?;
                    if game.audio_allowed && !settings.mute_audio && settings.audio_volume > 0 {
                        match gamepkg::audio::Player::new(settings.audio_volume) {
                            Ok(player) => Some(player),
                            Err(error) => {
                                eprintln!("Sound unavailable: {error:#}");
                                None
                            }
                        }
                    } else {
                        None
                    }
                };
                use std::io::{self, Write};
                game.step(0)?;
                #[cfg(feature = "audio")]
                if let Some(audio) = &audio {
                    for chunk in game
                        .audio_frame()
                        .chunks(1600 * game.audio_channels() as usize)
                    {
                        audio.submit_channels(chunk, game.audio_channels());
                    }
                }
                loop {
                    let (x, y) = game.player();
                    let (tx, ty) = game.target();
                    println!("\n{} — Score {}", game.id, game.score());
                    for row in 0..16 {
                        for col in 0..16 {
                            print!(
                                "{}",
                                if (col, row) == (x, y) {
                                    '@'
                                } else if (col, row) == (tx, ty) {
                                    '*'
                                } else {
                                    '.'
                                }
                            );
                        }
                        println!();
                    }
                    print!("W/A/S/D then Enter to move, Q to quit: ");
                    io::stdout().flush()?;
                    let mut line = String::new();
                    if io::stdin().read_line(&mut line)? == 0 {
                        break;
                    }
                    let input = match line.trim().to_ascii_lowercase().as_str() {
                        "a" => 1,
                        "d" => 2,
                        "w" => 4,
                        "s" => 8,
                        "q" => break,
                        _ => 0,
                    };
                    game.step(input)?;
                    #[cfg(feature = "audio")]
                    if let Some(audio) = &audio {
                        for chunk in game
                            .audio_frame()
                            .chunks(1600 * game.audio_channels() as usize)
                        {
                            audio.submit_channels(chunk, game.audio_channels());
                        }
                    }
                }
            }
            game.finish()?;
        }
        Command::ImportList { source } => {
            let count = gamepkg::catalog::sources::import_source(
                &gamepkg::registry::default_root()?,
                &source,
            )?;
            println!("Added {count} catalog games; no games downloaded");
        }
        Command::Download { id, trust_creator } => {
            let root = gamepkg::registry::default_root()?;
            let (_, file) = gamepkg::catalog::fetch(&root, &id)?;
            let m = gamepkg::registry::install(&root, file.path(), trust_creator)?;
            println!("Installed {} {}", m.id, m.version);
        }
        Command::Build {
            id,
            output,
            key,
            target,
        } => {
            if let (Some(output), Some(key)) = (output, key) {
                gamepkg::build::package_target(
                    std::path::Path::new(&id),
                    &output,
                    &gamepkg::creator::read_seed(&key)?,
                    target == "native",
                )?;
                println!("Built and verified source package: {}", output.display());
            } else {
                if target == "native" {
                    gamepkg::native::compile(&gamepkg::registry::default_root()?, &id)?;
                } else {
                    gamepkg::build::compile(&gamepkg::registry::default_root()?, &id)?;
                }
                println!("Built {id}");
            }
        }
        Command::CheckUpdates { id } => {
            match gamepkg::registry::check_update(&gamepkg::registry::default_root()?, &id)? {
                Some((m, _)) => println!("Update available: {} {}", m.id, m.version),
                None => println!("{id} is up to date"),
            }
        }
        Command::List => {
            let root = gamepkg::registry::default_root()?;
            let report = gamepkg::registry::inspect_library(&root)?;
            for (name, error) in report.rejected {
                println!("{name} — Unavailable: {error}");
            }
            let installed = report.games;
            for entry in gamepkg::catalog::entries(&root)? {
                if !installed.iter().any(|m| m.id == entry.id) {
                    println!(
                        "{} {} — Not downloaded (catalog metadata, unverified)",
                        entry.id,
                        entry.version.as_deref().unwrap_or("unknown")
                    );
                    if let Some(description) = &entry.description {
                        println!("Description: {description}");
                    }
                }
            }
            for m in installed {
                if m.title.is_some() {
                    println!("Title: {}", m.display_title());
                }
                if let Some(description) = &m.description {
                    println!("Description: {description}");
                }
                if let Some(author) = &m.author {
                    println!("Author: {author}");
                }
                if let Some(homepage) = &m.homepage {
                    println!("Homepage: {homepage}");
                }
                if !m.genres.is_empty() {
                    println!("Genres: {}", m.genres.join(", "));
                }

                println!(
                    "Build state: {}",
                    match (
                        gamepkg::native::is_built(&root, &m.id),
                        gamepkg::build::is_built(&root, &m.id)
                    ) {
                        (Ok(true), Ok(true)) => "Built (native and Wasm)",
                        (Ok(true), _) => "Built (native)",
                        (_, Ok(true)) => "Built (Wasm)",
                        (Ok(false), Ok(false)) => "Not built",
                        _ => "Unavailable",
                    }
                );
                let statistics = (|| -> Result<()> {
                    let progress = gamepkg::progress::load(&root, &m.id, &m.creator_key)?;
                    let usage = gamepkg::storage::usage(&root, &m)?;
                    println!(
                    "Played: {}; achievements: {}/{}; package: {}; build: {}; expected storage: {}",
                    gamepkg::progress::duration(progress.played_ms),
                    m.achievements
                        .iter()
                        .filter(|a| progress.unlocked.contains_key(&a.id))
                        .count(),
                    m.achievements.len(),
                    gamepkg::storage::bytes(usage.package_bytes),
                    gamepkg::storage::bytes(usage.built_bytes),
                    usage
                        .estimated_bytes
                        .map(gamepkg::storage::bytes)
                        .unwrap_or_else(|| "unknown until built".into())
                );
                    println!(
                        "Storage used: {}; {}",
                        gamepkg::storage::bytes(usage.used_bytes()),
                        usage.breakdown()
                    );
                    for a in &m.achievements {
                        println!(
                            "  [{}] {} — {}",
                            if progress.unlocked.contains_key(&a.id) {
                                "Unlocked"
                            } else {
                                "Locked"
                            },
                            a.title,
                            a.description
                        );
                    }
                    Ok(())
                })();
                if let Err(error) = statistics {
                    println!("Play history/storage unavailable: {error:#}");
                }
                println!(
                    "{} {} (ROM: {})",
                    m.id,
                    m.version,
                    if m.requires_rom {
                        "required from user"
                    } else {
                        "not required"
                    }
                );
            }
        }
        Command::Install {
            source,
            trust_creator,
        } => {
            let downloaded;
            let path = if source.contains("://") {
                downloaded = gamepkg::registry::download(&source)?;
                downloaded.path()
            } else {
                std::path::Path::new(&source)
            };
            let m = gamepkg::registry::install(
                &gamepkg::registry::default_root()?,
                path,
                trust_creator,
            )?;
            println!("Installed {} {}", m.id, m.version);
        }
        Command::Update { id } => {
            let m = gamepkg::registry::update(&gamepkg::registry::default_root()?, &id)?;
            println!("Updated {} {}", m.id, m.version);
        }
        Command::Pack {
            dir,
            output,
            key: path,
            shared_assets,
        } => {
            let seed = gamepkg::creator::read_seed(&path)?;
            if shared_assets {
                gamepkg::pack::pack_shared(
                    &dir,
                    &output,
                    &seed,
                    &gamepkg::registry::default_root()?,
                )?;
            } else {
                gamepkg::pack::pack(&dir, &output, &seed)?;
            }
            println!("Created {}", output.display());
        }
        Command::Verify {
            package,
            trusted_key,
        } => {
            let mut pkg = gamepkg::archive::Package::open(&package)?;
            let m = match trusted_key {
                Some(k) => gamepkg::verify::verify(&mut pkg, &key(&k)?)?,
                None => gamepkg::verify::verify_embedded(&mut pkg)?,
            };
            println!("Verified {} {}", m.id, m.version);
        }
        Command::Asset {
            package,
            path,
            trusted_key,
            output,
        } => {
            let mut pkg = gamepkg::archive::Package::open(&package)?;
            match trusted_key {
                Some(k) => {
                    gamepkg::verify::verify(&mut pkg, &key(&k)?)?;
                }
                None => {
                    gamepkg::verify::verify_embedded(&mut pkg)?;
                }
            }
            if pkg.has_shared_assets() {
                pkg.resolve_shared_assets(&gamepkg::registry::default_root()?)?;
            }
            use std::io::Write;
            let data = pkg.asset(&path)?;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?
                .write_all(&data)?;
        }
    }
    Ok(())
}
