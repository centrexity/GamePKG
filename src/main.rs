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
    /// Play an installed grid-v1 game in the terminal, or run N headless frames.
    Run {
        id: String,
        #[arg(long)]
        frames: Option<u32>,
        /// Read-only ROM file explicitly supplied for this session.
        #[arg(long)]
        rom: Option<PathBuf>,
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
    Build { id: String },
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
        Command::Run { id, frames, rom } => {
            let mut game = gamepkg::runtime::Game::load_with_rom(
                &gamepkg::registry::default_root()?,
                &id,
                rom.as_deref(),
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
            } else {
                if game.custom_renderer {
                    anyhow::bail!("this game draws graphics; open it in gamepkg-gui --play {id}, or use --frames for a headless run");
                }
                use std::io::{self, Write};
                game.step(0)?;
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
                }
            }
        }
        Command::ImportList { source } => {
            let downloaded;
            let path = if source.contains("://") {
                downloaded = gamepkg::registry::download(&source)?;
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
        Command::Build { id } => {
            gamepkg::build::compile(&gamepkg::registry::default_root()?, &id)?;
            println!("Built {id}");
        }
        Command::CheckUpdates { id } => {
            match gamepkg::registry::check_update(&gamepkg::registry::default_root()?, &id)? {
                Some((m, _)) => println!("Update available: {} {}", m.id, m.version),
                None => println!("{id} is up to date"),
            }
        }
        Command::List => {
            let root = gamepkg::registry::default_root()?;
            let installed = gamepkg::registry::list(&root)?;
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
        } => {
            gamepkg::pack::pack(&dir, &output, &key(&fs::read_to_string(path)?)?)?;
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
