//! Reviewed desktop engines that build and run outside the Wasm guest.
//!
//! A package names one of these profiles. The commands, data layouts, image
//! formats, and sandbox inputs live here, so a package cannot supply its own
//! compiler command or weaken the check that its game data stayed out of git.
use crate::manifest::DataLayout;
use anyhow::{bail, ensure, Context, Result};
use std::path::Path;

pub mod build;

const BINARY_LIMIT: u64 = 64 * 1024 * 1024;

/// One way a player's folder can satisfy an engine. The package must repeat it.
pub struct LayoutSpec {
    pub placement: &'static str,
    pub directories: &'static [&'static str],
    pub paths: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub min_bytes: Option<u64>,
    pub max_bytes: Option<u64>,
}

impl LayoutSpec {
    fn to_manifest(&self) -> DataLayout {
        DataLayout {
            placement: self.placement.into(),
            directories: self.directories.iter().map(|item| (*item).into()).collect(),
            paths: self.paths.iter().map(|item| (*item).into()).collect(),
            extensions: self.extensions.iter().map(|item| (*item).into()).collect(),
            min_bytes: self.min_bytes,
            max_bytes: self.max_bytes,
        }
    }
}

/// A file that must sit beside the engine executable.
#[derive(Debug)]
pub struct Companion {
    pub relative: &'static str,
    pub limit: u64,
}

/// What to do when a host library the engine wants is missing.
#[derive(Debug)]
pub struct AudioFallback {
    pub libraries: &'static [&'static str],
    pub variable: &'static str,
    pub value: &'static str,
    pub warning: &'static str,
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
    pub id: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    /// Upstream reason this target cannot be built. `None` means the build is supported.
    pub blocker: Option<&'static str>,
    pub tools: &'static [&'static str],
    pub configure: &'static [&'static str],
    pub build_program: &'static str,
    pub build_args: &'static [&'static str],
    pub binary: &'static str,
    pub companions: &'static [Companion],
    pub image: Image,
    pub audio: Option<AudioFallback>,
}

/// Environment the engine reads. Names come from the upstream port, values do not.
pub struct LaunchSpec {
    /// Set only when the matched layout links directories into a private data root.
    pub data_root: Option<&'static str>,
    pub save_root: Option<&'static str>,
    pub fixed: &'static [(&'static str, &'static str)],
    pub passthrough: &'static [&'static str],
}

pub struct Engine {
    pub id: &'static str,
    pub clone_url: &'static str,
    pub commit: &'static str,
    pub license_sha256: &'static str,
    pub license_marker: &'static str,
    pub memory: u64,
    pub hard_memory: u64,
    pub network_allowed: bool,
    pub launcher: &'static str,
    pub required_paths: &'static [&'static str],
    pub forbidden_directories: &'static [&'static str],
    pub forbidden_suffixes: &'static [&'static str],
    /// Directories the engine may write beside its executable. Cleared when the
    /// selected game data changes, so one disc's extraction cannot leak into the next.
    pub cleanup: &'static [&'static str],
    pub layout_specs: &'static [LayoutSpec],
    pub targets: &'static [Target],
    pub launch: LaunchSpec,
}

impl Engine {
    pub fn layouts(&self) -> Vec<DataLayout> {
        self.layout_specs
            .iter()
            .map(LayoutSpec::to_manifest)
            .collect()
    }
}

const LINUX_X64_BLOCKED: &str = "OpenCE's Linux build is 32-bit only. port/linux/include/halo_linux_prefix.h rejects every other ABI with `#error the Linux port targets 32-bit x86: game data structures assume 32-bit pointers`, and tools/linux_build.py hardcodes `--target=i686-linux-gnu -m32`. Cache files and saved games embed 32-bit pointers. There is no x86_64 or x32 target. The Android ILP32 guest is arm64_32, not 64-bit x86.";

const WINDOWS_X64_BLOCKED: &str = "OpenCE's Windows build is 32-bit only. port/windows/include/halo_windows_prefix.h rejects other targets with `#error the Windows port targets 32-bit x86: game data structures assume 32-bit pointers`. Clang `--target=x86_64-pc-windows-msvc` hits that error before any SDK header. Win64 is LLP64, so pointers would not match the Xbox map format. The upstream Windows target is `i686-pc-windows-msvc`.";

static ENGINES: &[Engine] = &[Engine {
    id: "opence-ce95f7de",
    clone_url: "https://github.com/OpenCommunityEdition/OpenCE.git",
    commit: "ce95f7deed5bbf726e50a9d5fe5227437194cf3b",
    license_sha256: "0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0",
    license_marker: "GNU AFFERO GENERAL PUBLIC LICENSE",
    memory: 2 * 1024 * 1024 * 1024,
    hard_memory: 3 * 1024 * 1024 * 1024,
    network_allowed: false,
    launcher:
        "pub fn game_tick(input: u32, state: u64) -> u64 { gamepkg::opence_tick(input, state) }\n",
    required_paths: &["configure.py", "source", "port/linux"],
    forbidden_directories: &["maps", "custom_maps"],
    forbidden_suffixes: &[".iso", ".xiso", ".map", ".wad", ".sfc", ".smc", ".rom"],
    cleanup: &["maps", "custom_maps", "content"],
    layout_specs: &[
        LayoutSpec {
            placement: "link",
            directories: &["maps"],
            paths: &["maps/ui.map"],
            extensions: &[],
            min_bytes: Some(1),
            max_bytes: Some(64 * 1024 * 1024),
        },
        LayoutSpec {
            placement: "beside",
            directories: &[],
            paths: &[],
            extensions: &["iso", "xiso"],
            min_bytes: Some(1024),
            max_bytes: Some(8 * 1024 * 1024 * 1024),
        },
    ],
    targets: &[
        Target {
            id: "linux-x86_64",
            os: "linux",
            arch: "x86_64",
            blocker: Some(LINUX_X64_BLOCKED),
            tools: &[],
            configure: &[],
            build_program: "",
            build_args: &[],
            binary: "",
            companions: &[],
            image: Image::Elf {
                class: 2,
                machine: 62,
            },
            audio: None,
        },
        Target {
            id: "linux-i686",
            os: "linux",
            arch: "i686",
            blocker: None,
            tools: &["python3", "ninja", "clang", "cmake", "pkg-config"],
            configure: &[
                "python3",
                "configure.py",
                "--release",
                "--portable",
                "--pgo=off",
                "--lto=off",
            ],
            build_program: "ninja",
            build_args: &["linux"],
            binary: "build/linux/halo",
            companions: &[Companion {
                relative: "build/linux/libSDL3.so.0",
                limit: 32 * 1024 * 1024,
            }],
            image: Image::Elf {
                class: 1,
                machine: 3,
            },
            audio: Some(AudioFallback {
                libraries: &[
                    "/usr/lib/i386-linux-gnu/libasound.so.2",
                    "/lib/i386-linux-gnu/libasound.so.2",
                    "/usr/lib32/libasound.so.2",
                ],
                variable: "SDL_AUDIODRIVER",
                value: "dummy",
                warning: "32-bit ALSA was not found. The engine will start without sound.",
            }),
        },
        Target {
            id: "windows-x64",
            os: "windows",
            arch: "x86_64",
            blocker: Some(WINDOWS_X64_BLOCKED),
            tools: &[],
            configure: &[],
            build_program: "",
            build_args: &[],
            binary: "",
            companions: &[],
            image: Image::Pe {
                machine: 0x8664,
                magic: 0x20b,
            },
            audio: None,
        },
        Target {
            id: "windows-i686",
            os: "windows",
            arch: "i686",
            blocker: None,
            tools: &["python3", "ninja", "clang", "cmake", "pkg-config"],
            // `--portable` selects `-march=x86-64`. Clang 18 ignores that flag on
            // `i686-pc-windows-msvc`, SSE2 stays off, and mbedtls then fails to
            // compile. The default `-march=native` enables SSE2 on this triple.
            configure: &[
                "python3",
                "configure.py",
                "--release",
                "--pgo=off",
                "--lto=off",
                "--windows-cross",
                "--windows-vctools-dir",
                "{vctools}",
                "--windows-sdk-dir",
                "{winsdk}",
            ],
            build_program: "ninja",
            build_args: &["windows"],
            binary: "build/windows/halo.exe",
            companions: &[Companion {
                relative: "build/windows/SDL3.dll",
                limit: 32 * 1024 * 1024,
            }],
            image: Image::Pe {
                machine: 0x14c,
                magic: 0x10b,
            },
            audio: None,
        },
    ],
    launch: LaunchSpec {
        data_root: Some("HALO_DATA_ROOT"),
        save_root: Some("HALO_SAVE_ROOT"),
        fixed: &[("HALO_UPDATE_AUTO", "false")],
        passthrough: &[
            "SDL_VIDEODRIVER",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XAUTHORITY",
            "XDG_RUNTIME_DIR",
            "LANG",
            "LC_ALL",
            "HALO_HIDDEN_WINDOW",
            "HALO_NULL_RENDERER",
            "HALO_EXIT_AFTER",
        ],
    },
}];

pub fn get(id: &str) -> Option<&'static Engine> {
    ENGINES.iter().find(|engine| engine.id == id)
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
                        .map(|target| target.id)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
        if let Some(blocker) = target.blocker {
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
                .any(|suffix| lower.ends_with(suffix))
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
    let base = root.join("builds").join(digest).join(engine.id);
    let id = std::fs::read_to_string(base.join("current")).unwrap_or_default();
    let id = id.trim();
    if let Some(target) = engine.targets.iter().find(|target| target.id == id) {
        let name = Path::new(target.binary)
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("engine"));
        return base.join(target.id).join(name);
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
    for companion in target.companions {
        let name = Path::new(companion.relative)
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

    fn engine() -> &'static Engine {
        get("opence-ce95f7de").unwrap()
    }

    #[test]
    fn blocked_abis_report_the_upstream_pointer_limit() {
        let engine = engine();
        for id in ["linux-x86_64", "windows-x64"] {
            let error = select(engine, Some(id)).unwrap_err().to_string();
            assert!(error.contains("32-bit pointers"), "{id}: {error}");
        }
        assert!(select(engine, Some("linux-i686"))
            .unwrap()
            .blocker
            .is_none());
        assert!(select(engine, Some("windows-i686"))
            .unwrap()
            .blocker
            .is_none());
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
