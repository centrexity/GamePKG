//! Host-reviewed native Takedawn recipe. Packages select this fixed source kit;
//! they cannot supply replacement engine code, build scripts or dependencies.
use anyhow::{ensure, Result};
use std::path::{Path, PathBuf};
pub const PROFILE: &str = "takedawn-5a00fc10-rust-v1";
pub const MEMORY: u64 = 4 * 1024 * 1024 * 1024;
pub const KIT_SHA256: &str = "a40e9656476633125d2fd50cc6a71134a453f8942634e34ebef848a215b98a5a";
pub const KIT_ASSET: &str = "assets/takedawn-source-kit.tar.gz";
pub const WINDOWS_KIT_SHA256: &str =
    "18afa3a1f2a0e9ddad89a33c2a3e33803e0764be0035da5e2c7a2e1a5235c885";
pub const LEGACY_WINDOWS_KIT_SHA256: &str =
    "ab30c4e7a4a86580a6fbefe7f79be941c713a0dd2b089d73fba6180729ab845b";
pub(crate) fn reviewed_kit_digest(bytes: &[u8]) -> Result<String> {
    let digest = crate::verify::digest(bytes);
    ensure!(
        digest == KIT_SHA256 || digest == WINDOWS_KIT_SHA256 || digest == LEGACY_WINDOWS_KIT_SHA256,
        "Takedawn source kit is not the host-reviewed version"
    );
    Ok(digest)
}
pub const LAUNCHER: &str =
    "pub fn game_tick(input: u32, state: u64) -> u64 { gamepkg::takedawn_tick(input, state) }\n";
/// Prepare the fixed standalone Windows SDK without WSL or vendor installers.
#[cfg(windows)]
pub fn prepare_windows_tools(root: &Path) -> Result<PathBuf> {
    ensure!(
        std::env::consts::ARCH == "x86_64",
        "the native Takedawn build tools currently require 64-bit Windows"
    );
    build::prepare_tools(root)
}
pub fn data_path(root: &Path, id: &str, creator: &str, schema: u32) -> PathBuf {
    let key = format!("GamePKG Takedawn native data v1\0{id}\0{creator}\0{schema}");
    root.join("takedawn-data")
        .join(crate::verify::digest(key.as_bytes()))
}

#[cfg(target_os = "linux")]
#[path = "takedawn/build.rs"]
pub mod build;
#[cfg(windows)]
#[path = "takedawn/windows_build.rs"]
pub mod build;

pub fn cache_path(root: &Path, digest: &str) -> PathBuf {
    #[cfg(windows)]
    return crate::native::path_for_digest(root, digest)
        .with_file_name(format!("takedawn-{}.exe", &WINDOWS_KIT_SHA256[..16]));
    #[cfg(not(windows))]
    crate::native::path_for_digest(root, digest)
        .with_file_name(format!("takedawn-{}", &KIT_SHA256[..16]))
}

pub(crate) fn validate_recipe(package: &mut crate::archive::Package) -> Result<()> {
    ensure!(
        package.read("source/src/lib.rs")? == LAUNCHER.as_bytes(),
        "Takedawn requires its reviewed native launcher"
    );
    for name in package.entries.keys() {
        if name.starts_with("source/") {
            ensure!(
                matches!(
                    name.as_str(),
                    "source/Cargo.toml" | "source/Cargo.lock" | "source/src/lib.rs"
                ),
                "Takedawn cannot add package-supplied game or build source"
            );
        }
        if name.starts_with("assets/") || name.starts_with("shaders/") {
            ensure!(
                name == KIT_ASSET,
                "Takedawn media must come from the selected ISO"
            );
        }
    }
    let cargo: toml::Value =
        toml::from_str(std::str::from_utf8(&package.read("source/Cargo.toml")?)?)?;
    crate::dependencies::validate(&cargo, &package.read("source/Cargo.lock")?)?;
    reviewed_kit_digest(&package.read(KIT_ASSET)?)?;
    Ok(())
}

/// Launches an installed signed package through its native window, with ROM
/// selection and play history owned by the host. No GUI shell window is made.
#[cfg(any(target_os = "linux", windows))]
pub fn play(
    root: &Path,
    id: &str,
    roms: &[PathBuf],
    settings: &crate::settings::Settings,
    frames: Option<u32>,
) -> Result<()> {
    let result = play_session(root, id, roms, settings, frames);
    if let Ok(manifest) = crate::registry::installed_manifest(root, id) {
        if manifest.builtin_engine.as_deref() == Some(PROFILE) {
            let folder = root.join("launch-errors");
            let key = crate::verify::digest(
                format!("GamePKG native launch v1\0{id}\0{}", manifest.creator_key).as_bytes(),
            );
            let path = folder.join(format!("{key}.txt"));
            if let Err(error) = &result {
                use std::io::Write;
                let save = || -> Result<()> {
                    ensure!(
                        !folder
                            .symlink_metadata()
                            .is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()),
                        "invalid launch diagnostics folder"
                    );
                    std::fs::create_dir_all(&folder)?;
                    let text = format!("{error:#}");
                    ensure!(
                        text.len() <= 1048576,
                        "launch diagnostic exceeded its bound"
                    );
                    let mut file = tempfile::NamedTempFile::new_in(&folder)?;
                    file.write_all(text.as_bytes())?;
                    file.as_file().sync_all()?;
                    file.persist(&path).map_err(|e| e.error)?;
                    Ok(())
                };
                let _ = save();
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    result
}

pub fn last_launch_error(root: &Path, id: &str) -> Result<Option<String>> {
    let manifest = crate::registry::installed_manifest(root, id)?;
    if manifest.builtin_engine.as_deref() != Some(PROFILE) {
        return Ok(None);
    }
    let key = crate::verify::digest(
        format!("GamePKG native launch v1\0{id}\0{}", manifest.creator_key).as_bytes(),
    );
    let folder = root.join("launch-errors");
    ensure!(
        !folder
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()),
        "invalid launch diagnostics folder"
    );
    let path = folder.join(format!("{key}.txt"));
    if !path.try_exists()? {
        return Ok(None);
    }
    Ok(Some(String::from_utf8(crate::files::read_regular(
        &path, 1048576, false,
    )?)?))
}

#[cfg(any(target_os = "linux", windows))]
fn play_session(
    root: &Path,
    id: &str,
    roms: &[PathBuf],
    settings: &crate::settings::Settings,
    frames: Option<u32>,
) -> Result<()> {
    use std::io::Write;
    let _guard = crate::build::BuildChange::read(root, id)?;
    crate::registry::installed_manifest(root, id)?;
    let original = crate::files::read_regular(
        &root.join(format!("{id}.gamepkg")),
        crate::archive::MAX_TOTAL,
        false,
    )?;
    let mut snapshot = tempfile::NamedTempFile::new()?;
    snapshot.write_all(&original)?;
    let mut package = crate::archive::Package::open(snapshot.path())?;
    let manifest = crate::verify::verify_embedded(&mut package)?;
    ensure!(
        manifest.builtin_engine.as_deref() == Some(PROFILE),
        "not a Takedawn package"
    );
    settings.validate()?;
    ensure!(
        settings.memory_ceiling_mib >= 4096,
        "Takedawn needs 4096 MiB. Increase Memory ceiling in Settings before playing."
    );
    ensure!(
        frames.is_none_or(|n| n > 0 && n <= 10000),
        "headless runs require 1 to 10000 frames"
    );
    let paths = if roms.is_empty() {
        crate::rom_selections::load(root, &manifest)?
    } else {
        roms.iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };
    ensure!(
        crate::rom_selections::ready_for(&manifest, &paths),
        "Choose the original supported Xbox ISO for this game"
    );
    let cached = cache_path(root, &crate::verify::digest(&original));
    let mut session = crate::takedawn_player::Session::start_cached(
        root,
        &manifest.id,
        &manifest.creator_key,
        manifest.save_schema,
        &cached,
        Path::new(&paths[0]),
        crate::takedawn_player::Options {
            headless: frames.is_some(),
            mute_audio: settings.mute_audio,
            audio_volume: settings.audio_volume,
        },
    )?;
    drop(_guard);
    let began = std::time::Instant::now();
    let mut admitted_at = None;
    let mut remembered = false;
    let mut waiting_since = std::time::Instant::now();
    let mut was_ready = false;
    loop {
        if let Some(status) = session.poll()? {
            ensure!(
                status.success(),
                "Native game stopped ({status}):\n{}",
                session.diagnostics()
            );
            ensure!(
                session.ready(),
                "Native game closed before it became ready:\n{}",
                session.diagnostics()
            );
            ensure!(
                frames.is_none(),
                "game closed before the requested headless frames"
            );
            return Ok(());
        }
        if !session.ready() {
            if was_ready {
                waiting_since = std::time::Instant::now();
                admitted_at = None;
                was_ready = false;
            }
            if session.disc_admitted() && admitted_at.is_none() {
                admitted_at = Some(std::time::Instant::now());
            }
            ensure!(
                admitted_at.map_or(waiting_since.elapsed().as_secs() < 300, |when| when
                    .elapsed()
                    .as_secs()
                    < 60),
                "The native game window did not open: {}",
                session.diagnostics()
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }
        was_ready = true;
        if !remembered {
            crate::rom_selections::save(root, &manifest, &paths)?;
            remembered = true;
        }
        if let Some(count) = frames {
            if session.frame_count() >= count {
                session.stop()?;
                return Ok(());
            }
            ensure!(
                began.elapsed().as_secs() < 60 + u64::from(count) * 2,
                "native headless run stopped rendering: {}",
                session.diagnostics()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    fn package(root: &Path) -> crate::manifest::Manifest {
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("fixture.gamepkg");
        crate::pack::pack(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/takedawn"),
            &path,
            &[111; 32],
        )
        .unwrap();
        crate::registry::install(root, &path, true).unwrap()
    }
    #[test]
    fn manifest_requires_the_exact_native_permissions_rom_and_memory() {
        let text = include_str!("../ports/takedawn/manifest.toml");
        crate::manifest::Manifest::parse(text.as_bytes()).unwrap();
        for (before, after) in [
            (
                "hard_memory_bytes = 4294967296",
                "hard_memory_bytes = 536870912",
            ),
            ("network = false", "network = true"),
            ("save = true", "save = false"),
            ("input = true", "input = false"),
            ("audio = true", "audio = false"),
            ("takedawn-5a00fc10-rust-v1", "takedawn-unreviewed"),
        ] {
            assert!(
                crate::manifest::Manifest::parse(text.replace(before, after).as_bytes()).is_err()
            );
        }
    }
    #[test]
    fn recipes_reject_replaced_kits_extra_source_and_custom_launchers() {
        use std::io::Write;
        for (extra, custom, reason) in [
            (None, false, "host-reviewed version"),
            (None, true, "reviewed native launcher"),
            (Some("source/build.rs"), false, "package-supplied"),
            (Some("source/src/extra.rs"), false, "package-supplied"),
            (Some("assets/extra.c"), false, "selected ISO"),
        ] {
            let file = tempfile::NamedTempFile::new().unwrap();
            let mut zip = zip::ZipWriter::new(file.reopen().unwrap());
            for (name, bytes) in [
                (
                    "source/Cargo.toml",
                    include_bytes!("../ports/takedawn/source/Cargo.toml").as_slice(),
                ),
                (
                    "source/Cargo.lock",
                    include_bytes!("../ports/takedawn/source/Cargo.lock").as_slice(),
                ),
                (
                    "source/src/lib.rs",
                    if custom {
                        b"pub fn game_tick(_:u32,_:u64)->u64 { 7 }\n".as_slice()
                    } else {
                        LAUNCHER.as_bytes()
                    },
                ),
                (KIT_ASSET, b"replaced source kit".as_slice()),
            ] {
                zip.start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
                zip.write_all(bytes).unwrap();
            }
            if let Some(name) = extra {
                zip.start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
                zip.write_all(b"unreviewed").unwrap();
            }
            zip.finish().unwrap();
            let mut package = crate::archive::Package::open(file.path()).unwrap();
            let error = validate_recipe(&mut package).unwrap_err().to_string();
            assert!(error.contains(reason), "{error}");
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "full offline native source build; requires private SDK tools and substantial compilation time"]
    fn installed_signed_package_builds_through_normal_native_action() {
        let evidence = PathBuf::from(std::env::var_os("GAMEPKG_TAKEDAWN_BUILD_EVIDENCE").unwrap());
        std::fs::create_dir(&evidence).unwrap();
        package(&evidence);
        // Optional maintainer recovery of a previous interrupted test's
        // independently verified checkpoints. The public builder never reads
        // this environment variable; it uses only its own library cache.
        if let Some(seed) = std::env::var_os("GAMEPKG_TAKEDAWN_TEST_UNIT_CACHE") {
            let cache = evidence.join("native-unit-cache/takedawn");
            std::fs::create_dir_all(&cache).unwrap();
            for entry in std::fs::read_dir(seed).unwrap() {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "lock") {
                    continue;
                }
                assert!(path.extension().is_some_and(|e| e == "o" || e == "sha256"));
                std::fs::copy(&path, cache.join(entry.file_name())).unwrap();
            }
        }
        assert!(!crate::native::is_built(&evidence, "com.gamepkg.takedawn").unwrap());
        crate::native::compile(&evidence, "com.gamepkg.takedawn").unwrap();
        assert!(crate::native::is_built(&evidence, "com.gamepkg.takedawn").unwrap());
        crate::integrity::check(&evidence, "com.gamepkg.takedawn").unwrap();
        eprintln!(
            "PASS: signed-package native Build and integrity; evidence: {}",
            evidence.display()
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires privately built reviewed executable and owned ISO; uses only a temporary library"]
    fn signed_package_plays_and_remembers_iso_without_copying_it() {
        let root = tempfile::tempdir().unwrap();
        package(root.path());
        let manifest =
            crate::registry::installed_manifest(root.path(), "com.gamepkg.takedawn").unwrap();
        let executable = PathBuf::from(std::env::var_os("GAMEPKG_TAKEDAWN_EXECUTABLE").unwrap());
        let iso = PathBuf::from(std::env::var_os("GAMEPKG_TAKEDAWN_ISO").unwrap());
        let path = crate::native::cache_path(root.path(), &manifest.id).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let bytes = crate::files::read_regular(&executable, 64 * 1048576, false).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        crate::integrity::record_build(&path, &bytes).unwrap();
        assert!(crate::native::is_built(root.path(), &manifest.id).unwrap());
        let settings = crate::settings::Settings {
            memory_ceiling_mib: 4096,
            ..Default::default()
        };
        play(
            root.path(),
            &manifest.id,
            std::slice::from_ref(&iso),
            &settings,
            Some(120),
        )
        .unwrap();
        let first = crate::progress::load(root.path(), &manifest.id, &manifest.creator_key)
            .unwrap()
            .played_ms;
        assert!(first > 1000);
        assert_eq!(
            crate::rom_selections::load(root.path(), &manifest).unwrap(),
            [iso.to_string_lossy().to_string()]
        );
        // An incorrect disc can pass the cheap picker size check. Full native
        // admission must reject it without replacing a known working selection
        // or charging the failed startup as play time.
        let wrong_disc = root.path().join("wrong.iso");
        std::fs::File::create(&wrong_disc)
            .unwrap()
            .set_len(128 * 1048576)
            .unwrap();
        let error = play(
            root.path(),
            &manifest.id,
            &[wrong_disc],
            &settings,
            Some(120),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("Native game stopped"),
            "{error:#}"
        );
        assert_eq!(
            crate::rom_selections::load(root.path(), &manifest).unwrap(),
            [iso.to_string_lossy().to_string()]
        );
        assert_eq!(
            crate::progress::load(root.path(), &manifest.id, &manifest.creator_key)
                .unwrap()
                .played_ms,
            first
        );
        assert!(last_launch_error(root.path(), &manifest.id)
            .unwrap()
            .is_some_and(|message| message.contains("Native game stopped")));
        play(root.path(), &manifest.id, &[], &settings, Some(120)).unwrap();
        assert!(last_launch_error(root.path(), &manifest.id)
            .unwrap()
            .is_none());
        assert!(
            crate::progress::load(root.path(), &manifest.id, &manifest.creator_key)
                .unwrap()
                .played_ms
                > first
        );
        assert!(
            !data_path(root.path(), &manifest.id, &manifest.creator_key, 1)
                .join("disc.iso")
                .exists()
        );
        assert!(
            crate::storage::usage(root.path(), &manifest)
                .unwrap()
                .save_bytes
                > 0
        );
        crate::integrity::check(root.path(), &manifest.id).unwrap();
    }
}
