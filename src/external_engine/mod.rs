//! Reviewed desktop engines that build and run outside the Wasm guest.
//!
//! A package names one of these profiles. The commands, data layouts, image
//! formats, and sandbox inputs live here, so a package cannot supply its own
//! compiler command or weaken the check that its game data stayed out of git.
use crate::manifest::DataLayout;
use anyhow::{bail, ensure, Context, Result};
use std::path::Path;
use std::sync::OnceLock;

pub mod build;
mod catalog;

const BINARY_LIMIT: u64 = 64 * 1024 * 1024;

/// A file that must sit beside the engine executable.
#[derive(Debug)]
pub struct Companion {
    pub relative: String,
    pub limit: u64,
}

/// What to do when a host library the engine wants is missing.
#[derive(Debug)]
pub struct AudioFallback {
    pub libraries: Vec<String>,
    pub variable: String,
    pub value: String,
    pub warning: String,
}

/// Executable image this target is allowed to cache.
#[derive(Clone, Copy, Debug)]
pub enum Image {
    Elf { class: u8, machine: u16 },
    Pe { machine: u16, magic: u16 },
}

/// One upstream build of an external engine.
#[derive(Debug)]
pub struct Target {
    pub id: String,
    pub os: String,
    pub arch: String,
    /// Recorded reason this target cannot be built. `None` means the build is supported.
    pub blocker: Option<String>,
    /// Review note. The host does not branch on it.
    pub note: Option<String>,
    pub tools: Vec<String>,
    pub configure: Vec<String>,
    pub build_program: String,
    pub build_args: Vec<String>,
    pub binary: String,
    pub companions: Vec<Companion>,
    pub image: Image,
    pub audio: Option<AudioFallback>,
}

/// Environment the engine reads. Names come from the engine record.
pub struct LaunchSpec {
    /// Set only when the matched layout links directories into a private data root.
    pub data_root: Option<String>,
    pub save_root: Option<String>,
    pub fixed: Vec<(String, String)>,
    pub passthrough: Vec<String>,
}

pub struct Engine {
    pub id: String,
    pub clone_url: String,
    pub commit: String,
    pub license_sha256: String,
    pub license_marker: String,
    pub memory: u64,
    pub hard_memory: u64,
    pub network_allowed: bool,
    pub launcher: String,
    pub required_paths: Vec<String>,
    pub forbidden_directories: Vec<String>,
    pub forbidden_suffixes: Vec<String>,
    /// Directories the engine may write beside its executable. Cleared when the
    /// selected game data changes, so one extraction cannot leak into the next.
    pub cleanup: Vec<String>,
    /// Patch name and body, applied after the pinned commit is checked out.
    pub patches: Vec<(String, String)>,
    pub layouts: Vec<DataLayout>,
    pub targets: Vec<Target>,
    pub launch: LaunchSpec,
}

impl Engine {
    pub fn layouts(&self) -> Vec<DataLayout> {
        self.layouts.clone()
    }
}

include!(concat!(env!("OUT_DIR"), "/external_engine_records.rs"));

fn catalog() -> &'static [Engine] {
    static CATALOG: OnceLock<Vec<Engine>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        catalog::load(RECORDS)
            .unwrap_or_else(|error| panic!("reviewed external-engine catalog is invalid: {error}"))
    })
}

pub fn get(id: &str) -> Option<&'static Engine> {
    catalog().iter().find(|engine| engine.id == id)
}

/// Linux can run its own native workers. Another host can run an external
/// engine only when that engine has a ready target for the host operating system.
pub fn runs_on_host(profile: Option<&str>) -> bool {
    if cfg!(target_os = "linux") {
        return true;
    }
    profile.and_then(get).is_some_and(|engine| {
        engine
            .targets
            .iter()
            .any(|target| target.blocker.is_none() && target.os == std::env::consts::OS)
    })
}

pub fn select<'a>(engine: &'a Engine, requested: Option<&str>) -> Result<&'a Target> {
    if let Some(id) = requested {
        let target = engine
            .targets
            .iter()
            .find(|target| target.id == id)
            .with_context(|| {
                format!(
                    "unknown engine target {id}. Known targets: {}",
                    engine
                        .targets
                        .iter()
                        .map(|target| target.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
        if let Some(blocker) = &target.blocker {
            bail!("{blocker}");
        }
        return Ok(target);
    }
    let host_arch = match std::env::consts::ARCH {
        "x86" => "i686",
        other => other,
    };
    if let Some(target) = engine.targets.iter().find(|target| {
        target.blocker.is_none() && target.os == std::env::consts::OS && target.arch == host_arch
    }) {
        return Ok(target);
    }
    engine
        .targets
        .iter()
        .find(|target| target.blocker.is_none() && target.os == std::env::consts::OS)
        .with_context(|| format!("no ready engine target for {}", std::env::consts::OS))
}

pub fn validate_image(image: &Image, bytes: &[u8]) -> Result<()> {
    match *image {
        Image::Elf { class, machine } => {
            ensure!(
                bytes.len() > 64 && bytes.starts_with(b"\x7fELF"),
                "compiler did not produce an ELF binary"
            );
            ensure!(bytes[4] == class, "executable has the wrong ELF class");
            ensure!(bytes[5] == 1, "executable has an incompatible byte order");
            let found = u16::from_le_bytes([bytes[18], bytes[19]]);
            ensure!(
                found == machine,
                "executable is machine {found}, expected {machine}"
            );
        }
        Image::Pe { machine, magic } => {
            ensure!(
                bytes.len() > 0x40 && bytes.starts_with(b"MZ"),
                "compiler did not produce a Windows executable"
            );
            let offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
            ensure!(
                bytes.len() > offset.saturating_add(26)
                    && bytes.get(offset..offset + 4) == Some(b"PE\0\0"),
                "executable is not a PE image"
            );
            let found = u16::from_le_bytes([bytes[offset + 4], bytes[offset + 5]]);
            ensure!(
                found == machine,
                "executable is machine {found:#x}, expected {machine:#x}"
            );
            let found_magic = u16::from_le_bytes([bytes[offset + 24], bytes[offset + 25]]);
            ensure!(
                found_magic == magic,
                "executable is PE magic {found_magic:#x}, expected {magic:#x}"
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_recipe(
    engine: &Engine,
    package: &mut crate::archive::Package,
) -> Result<()> {
    ensure!(
        package.read("source/src/lib.rs")? == engine.launcher.as_bytes(),
        "this engine accepts only its reviewed launcher"
    );
    ensure!(
        !package
            .entries
            .keys()
            .any(|name| name.starts_with("assets/") || name.starts_with("shaders/")),
        "this engine cannot ship package media or shaders in place of game data"
    );
    for name in package.entries.keys() {
        let lower = name.to_ascii_lowercase();
        ensure!(
            !engine
                .forbidden_suffixes
                .iter()
                .any(|suffix| lower.ends_with(suffix.as_str()))
                && !engine
                    .forbidden_directories
                    .iter()
                    .any(|dir| contains_directory(&lower, dir)),
            "this package cannot contain the engine's game data"
        );
        if name.starts_with("source/") {
            ensure!(
                matches!(
                    name.as_str(),
                    "source/Cargo.toml" | "source/Cargo.lock" | "source/src/lib.rs"
                ),
                "this engine cannot add package-supplied source"
            );
        }
    }
    let cargo: toml::Value =
        toml::from_str(std::str::from_utf8(&package.read("source/Cargo.toml")?)?)?;
    crate::dependencies::validate(&cargo, &package.read("source/Cargo.lock")?)
}

fn contains_directory(name: &str, directory: &str) -> bool {
    name == directory
        || name.starts_with(&format!("{directory}/"))
        || name.contains(&format!("/{directory}/"))
}

pub fn cached_binary(root: &Path, digest: &str, engine: &Engine) -> std::path::PathBuf {
    let base = root.join("builds").join(digest).join(&engine.id);
    let id = std::fs::read_to_string(base.join("current")).unwrap_or_default();
    let id = id.trim();
    if let Some(target) = engine.targets.iter().find(|target| target.id == id) {
        let name = Path::new(&target.binary)
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("engine"));
        return base.join(&target.id).join(name);
    }
    base.join("not-built")
}

pub fn cache_ready(binary: &Path, engine: &Engine) -> Result<bool> {
    let Some(target_id) = binary
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
    else {
        return Ok(false);
    };
    let Some(target) = engine.targets.iter().find(|target| target.id == target_id) else {
        return Ok(false);
    };
    if target.blocker.is_some() || !binary.is_file() {
        return Ok(false);
    }
    let bytes = match crate::files::read_regular(binary, BINARY_LIMIT, false) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(false),
    };
    if validate_image(&target.image, &bytes).is_err()
        || crate::integrity::verify_build(binary, &bytes).is_err()
    {
        return Ok(false);
    }
    for companion in &target.companions {
        let name = Path::new(&companion.relative)
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("companion"));
        let path = binary.with_file_name(name);
        let bytes = match crate::files::read_regular(&path, companion.limit, false) {
            Ok(bytes) => bytes,
            Err(_) => return Ok(false),
        };
        let receipt = path.with_file_name(format!("{}.sha256", name.to_string_lossy()));
        let expected = match std::fs::read(&receipt) {
            Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
            Err(_) => return Ok(false),
        };
        if expected != crate::verify::digest(&bytes) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn launch(root: &Path, id: &str) -> Result<()> {
    build::launch(root, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_targets_return_their_recorded_reason() {
        assert!(!catalog().is_empty());
        for engine in catalog() {
            for target in &engine.targets {
                match &target.blocker {
                    Some(blocker) => {
                        let error = select(engine, Some(&target.id)).unwrap_err().to_string();
                        assert_eq!(error, *blocker, "{}", target.id);
                    }
                    None => {
                        let selected = select(engine, Some(&target.id)).unwrap();
                        assert!(selected.blocker.is_none());
                        assert!(!selected.configure.is_empty());
                    }
                }
            }
        }
    }

    #[test]
    fn image_checks_accept_only_the_declared_format() {
        let mut elf32 = vec![0u8; 128];
        elf32[..4].copy_from_slice(b"\x7fELF");
        elf32[4] = 1;
        elf32[5] = 1;
        elf32[18..20].copy_from_slice(&3u16.to_le_bytes());
        validate_image(
            &Image::Elf {
                class: 1,
                machine: 3,
            },
            &elf32,
        )
        .unwrap();
        let mut elf64 = elf32.clone();
        elf64[4] = 2;
        elf64[18..20].copy_from_slice(&62u16.to_le_bytes());
        assert!(validate_image(
            &Image::Elf {
                class: 1,
                machine: 3
            },
            &elf64
        )
        .is_err());

        let mut pe32 = vec![0u8; 0x120];
        pe32[..2].copy_from_slice(b"MZ");
        pe32[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        pe32[0x80..0x84].copy_from_slice(b"PE\0\0");
        pe32[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        pe32[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        validate_image(
            &Image::Pe {
                machine: 0x14c,
                magic: 0x10b,
            },
            &pe32,
        )
        .unwrap();
        let mut pe64 = pe32.clone();
        pe64[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
        pe64[0x98..0x9a].copy_from_slice(&0x20bu16.to_le_bytes());
        assert!(validate_image(
            &Image::Pe {
                machine: 0x14c,
                magic: 0x10b,
            },
            &pe64,
        )
        .is_err());
    }
}
