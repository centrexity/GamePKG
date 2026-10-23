//! Linux playback for the pinned OpenCE engine.
//!
//! The package contains no engine source and no Halo maps. Build clones the
//! pinned commit into the library, and Play runs that 32-bit binary in its
//! own window. Bubblewrap removes its network when the tool is installed.
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(target_os = "linux")]
pub mod build;

pub const PROFILE: &str = "opence-ce95f7de";
pub const COMMIT: &str = "ce95f7deed5bbf726e50a9d5fe5227437194cf3b";
pub const CLONE_URL: &str = "https://github.com/OpenCommunityEdition/OpenCE.git";
pub const LICENSE_SHA256: &str = "0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0";
pub const MEMORY: u64 = 2 * 1024 * 1024 * 1024;
pub const HARD_MEMORY_LIMIT: u64 = 3 * 1024 * 1024 * 1024;
pub const LAUNCHER: &str =
    "pub fn game_tick(input: u32, state: u64) -> u64 { gamepkg::opence_tick(input, state) }\n";
const BINARY_LIMIT: u64 = 64 * 1024 * 1024;
const SDL_LIMIT: u64 = 16 * 1024 * 1024;

pub(crate) fn validate_recipe(package: &mut crate::archive::Package) -> Result<()> {
    ensure!(
        package.read("source/src/lib.rs")? == LAUNCHER.as_bytes(),
        "OpenCE accepts only its reviewed engine launcher"
    );
    ensure!(
        !package
            .entries
            .keys()
            .any(|name| name.starts_with("assets/") || name.starts_with("shaders/")),
        "OpenCE cannot ship package media or shaders in place of your game data"
    );
    for name in package.entries.keys() {
        let lower = name.to_ascii_lowercase();
        ensure!(
            !lower.ends_with(".iso")
                && !lower.ends_with(".xiso")
                && !lower.ends_with(".map")
                && !lower.contains("/maps/")
                && !lower.contains("/custom_maps/"),
            "OpenCE packages cannot contain disc images or map caches"
        );
        if name.starts_with("source/") {
            ensure!(
                matches!(
                    name.as_str(),
                    "source/Cargo.toml" | "source/Cargo.lock" | "source/src/lib.rs"
                ),
                "OpenCE cannot add package-supplied engine or Rust source"
            );
        }
    }
    let cargo: toml::Value =
        toml::from_str(std::str::from_utf8(&package.read("source/Cargo.toml")?)?)?;
    crate::dependencies::validate(&cargo, &package.read("source/Cargo.lock")?)
}

pub fn binary_path(root: &Path, package_digest: &str) -> PathBuf {
    root.join("builds")
        .join(package_digest)
        .join(PROFILE)
        .join("halo")
}

pub fn cache_ready(binary: &Path) -> Result<bool> {
    if !binary.is_file() {
        return Ok(false);
    }
    let bytes = crate::files::read_regular(binary, BINARY_LIMIT, false)?;
    if validate_i686(&bytes).is_err() || crate::integrity::verify_build(binary, &bytes).is_err() {
        return Ok(false);
    }
    let sdl = binary.with_file_name("libSDL3.so.0");
    let sdl_bytes = match crate::files::read_regular(&sdl, SDL_LIMIT, false) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(false),
    };
    let receipt = sdl.with_file_name("libSDL3.so.0.sha256");
    let expected = match std::fs::read(&receipt) {
        Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
        Err(_) => return Ok(false),
    };
    Ok(expected == crate::verify::digest(&sdl_bytes))
}

pub fn validate_i686(bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() > 64 && bytes.starts_with(b"\x7fELF"),
        "OpenCE compiler did not produce an ELF binary"
    );
    ensure!(bytes[4] == 1, "OpenCE engine must be a 32-bit ELF");
    ensure!(
        bytes[5] == 1,
        "OpenCE engine has an incompatible byte order"
    );
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    ensure!(
        machine == 3,
        "OpenCE engine must be Intel 80386, found machine {machine}"
    );
    Ok(())
}

#[derive(Debug)]
pub enum GameData {
    Maps(PathBuf),
    Disc(PathBuf, PathBuf),
}

pub fn classify(path: &Path) -> Result<GameData> {
    let path = fs::canonicalize(path).context("choose the Halo data folder")?;
    ensure!(path.is_dir(), "Halo data must be a folder");
    let maps = path.join("maps");
    if maps
        .symlink_metadata()
        .is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink())
    {
        let ui = maps.join("ui.map");
        let meta = ui
            .symlink_metadata()
            .context("the maps folder needs ui.map from your Xbox disc")?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "ui.map must be a regular file from your disc"
        );
        ensure!(
            (1..=BINARY_LIMIT).contains(&meta.len()),
            "ui.map size is not plausible for an Xbox maps folder"
        );
        return Ok(GameData::Maps(path));
    }
    let mut discs = Vec::new();
    for entry in fs::read_dir(&path).context("read the Halo data folder")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !(name.ends_with(".iso") || name.ends_with(".xiso")) {
            continue;
        }
        let meta = fs::symlink_metadata(entry.path())?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "disc image must be a regular file, not a link"
        );
        ensure!(
            (1024..8 * 1024 * 1024 * 1024).contains(&meta.len()),
            "disc image size is not plausible"
        );
        discs.push(entry.path());
    }
    ensure!(
        discs.len() == 1,
        "choose a folder that contains maps/ui.map from your disc, or exactly one .iso or .xiso file"
    );
    Ok(GameData::Disc(path, discs.remove(0)))
}

pub fn launch(root: &Path, id: &str) -> Result<()> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, id);
        anyhow::bail!("OpenCE currently runs on Linux");
    }
    #[cfg(target_os = "linux")]
    build::launch(root, id)
}

pub(crate) fn alsa_available() -> bool {
    [
        "/usr/lib/i386-linux-gnu/libasound.so.2",
        "/lib/i386-linux-gnu/libasound.so.2",
        "/usr/lib32/libasound.so.2",
    ]
    .iter()
    .any(|path| Path::new(path).exists())
}

pub(crate) fn command(binary: &Path, runtime: &Path, isolate_network: bool) -> Command {
    if isolate_network && Command::new("bwrap").arg("--version").output().is_ok() {
        let mut command = Command::new("bwrap");
        command
            .arg("--unshare-net")
            .arg("--die-with-parent")
            .arg("--ro-bind")
            .arg("/")
            .arg("/")
            .arg("--dev-bind")
            .arg("/dev")
            .arg("/dev")
            .arg("--proc")
            .arg("/proc")
            .arg("--bind")
            .arg(runtime)
            .arg(runtime);
        if Path::new("/dev/shm").is_dir() {
            command.arg("--bind").arg("/dev/shm").arg("/dev/shm");
        }
        if Path::new("/tmp/.X11-unix").is_dir() {
            command
                .arg("--bind")
                .arg("/tmp/.X11-unix")
                .arg("/tmp/.X11-unix");
        }
        if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            if Path::new(&dir).is_dir() {
                command.arg("--bind").arg(&dir).arg(&dir);
            }
        }
        command.arg("--chdir").arg(runtime).arg("--").arg(binary);
        return command;
    }
    if isolate_network {
        eprintln!("OpenCE network isolation needs bubblewrap; the engine process keeps this user's network.");
    }
    let mut command = Command::new(binary);
    command.current_dir(runtime);
    command
}
