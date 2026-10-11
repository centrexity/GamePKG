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
    /// Validate and sign a package directory. This does not compile the game.
    Pack {
        dir: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        key: PathBuf,
    },
    /// Verify using an explicitly trusted Ed25519 public key (hex).
    Verify {
        package: PathBuf,
        #[arg(long)]
        trusted_key: String,
    },
    /// Read an asset from a verified archive without extraction.
    Asset {
        package: PathBuf,
        path: String,
        #[arg(long)]
        trusted_key: String,
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
            let m = gamepkg::verify::verify(
                &mut gamepkg::archive::Package::open(&package)?,
                &key(&trusted_key)?,
            )?;
            println!("Verified {} {}", m.id, m.version);
        }
        Command::Asset {
            package,
            path,
            trusted_key,
            output,
        } => {
            let mut pkg = gamepkg::archive::Package::open(&package)?;
            gamepkg::verify::verify(&mut pkg, &key(&trusted_key)?)?;
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
