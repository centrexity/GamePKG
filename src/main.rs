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
    /// Open a local browser metadata library using Hindsight Database/IndexedDB.
    WebLibrary {
        #[arg(long)]
        indexeddb_package: PathBuf,
        #[arg(long, default_value_t = 39173)]
        port: u16,
    },
    /// Verify installed signed contents, shared assets and compiled game checksums.
    IntegrityCheck { id: String },
    /// Rebuild compiled files from verified source; never alter signatures or player data.
    IntegrityFix { id: String },
    /// Manage host approval of checksum-pinned bundled guest crates.
    Allowlist {
        #[command(subcommand)]
        command: AllowlistCommand,
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
    /// Play an installed grid-v1 game in the terminal, or run N headless frames.
    Run {
        id: String,
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
    /// Compile an installed game to Wasm under Linux bubblewrap isolation.
    Build {
        id: String,
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
#[derive(Subcommand)]
enum AllowlistCommand {
    /// Show the bundled catalog and current approvals.
    List,
    /// Approve a crate from the bundled pinned catalog.
    Add { name: String },
    /// Revoke a crate approval for package verification and future builds.
    Remove { name: String },
}
fn key(s: &str) -> Result<[u8; 32]> {
    hex::decode(s.trim())?
        .try_into()
        .map_err(|_| anyhow::anyhow!("key must contain 32 bytes"))
}
fn main() -> Result<()> {
    match Cli::parse().command {
        Command::WebLibrary {
            indexeddb_package,
            port,
        } => gamepkg::web_library::serve(
            &gamepkg::registry::default_root()?,
            &indexeddb_package,
            port,
        )?,
        Command::IntegrityCheck { id } => println!(
            "{}",
            gamepkg::integrity::check(&gamepkg::registry::default_root()?, &id)?
        ),
        Command::IntegrityFix { id } => println!(
            "{}",
            gamepkg::integrity::fix(&gamepkg::registry::default_root()?, &id)?
        ),
        Command::Allowlist { command } => match command {
            AllowlistCommand::List => {
                let settings = gamepkg::settings::load()?;
                println!(
                    "{} {} — {} — SHA-256 {}",
                    gamepkg::allowlist::NAME,
                    gamepkg::allowlist::VERSION,
                    if settings
                        .approved_crates
                        .iter()
                        .any(|name| name == gamepkg::allowlist::NAME)
                    {
                        "approved"
                    } else {
                        "not approved"
                    },
                    gamepkg::allowlist::CHECKSUM
                );
            }
            AllowlistCommand::Add { name } => {
                gamepkg::allowlist::set_approval(&name, true)?;
                println!("Approved {name} from the pinned catalog");
            }
            AllowlistCommand::Remove { name } => {
                gamepkg::allowlist::set_approval(&name, false)?;
                println!("Revoked {name}; applies to subsequent verification and builds");
            }
        },

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
        Command::Keygen { output } => {
            let public = gamepkg::creator::generate(&output)?;
            println!("Created private signing key: {}", output.display());
            println!("Creator public key: {public}");
        }
        Command::Run { id, frames, rom } => {
            let mut game = gamepkg::runtime::Game::load_with_roms(
                &gamepkg::registry::default_root()?,
                &id,
                &rom,
            )?;
            if let Some(count) = frames {
                if count > 10000 {
                    anyhow::bail!("headless runs are limited to 10000 frames");
                }
                for _ in 0..count {
                    game.step(0)?;
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
                    audio.submit(game.audio_frame());
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
                        audio.submit(game.audio_frame());
                    }
                }
            }
            game.finish()?;
        }
        Command::ImportList { source } => {
            let downloaded;
            let path = if source.contains("://") {
                downloaded = gamepkg::registry::download_catalog(&source)?;
                downloaded.path()
            } else {
                std::path::Path::new(&source)
            };
            let count = gamepkg::catalog::import(&gamepkg::registry::default_root()?, path)?;
            println!("Added {count} catalog games; no games downloaded");
        }
        Command::Download { id, trust_creator } => {
            let root = gamepkg::registry::default_root()?;
            let (_, file) = gamepkg::catalog::fetch(&root, &id)?;
            let m = gamepkg::registry::install(&root, file.path(), trust_creator)?;
            println!("Installed {} {}", m.id, m.version);
        }
        Command::Build { id, output, key } => {
            if let (Some(output), Some(key)) = (output, key) {
                gamepkg::build::package(
                    std::path::Path::new(&id),
                    &output,
                    &gamepkg::creator::read_seed(&key)?,
                )?;
                println!("Built and verified source package: {}", output.display());
            } else {
                gamepkg::build::compile(&gamepkg::registry::default_root()?, &id)?;
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
                }
            }
            for m in installed {
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
