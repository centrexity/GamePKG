use crate::manifest::{Manifest, NativeGraphics};
#[cfg(target_os = "linux")]
use crate::port_host::owner::VerifiedAssets;
use crate::port_host::{
    admission::CheckedSources,
    graphics::{textured_v1_recipe, PipelineRecipe, VerifiedPipelines},
    preparation::{self, Artifact, Bootstrap, ExecutableReceipt, LocalLayout, Plan, Target},
    process::{NativeLaunch, Process},
};
use anyhow::{ensure, Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn plan(manifest: &Manifest) -> Result<Plan> {
    let entry = if manifest.uses_service_runtime() {
        ensure!(
            manifest.permissions.files.is_empty()
                && !manifest.permissions.microphone
                && manifest.shared_assets.is_empty(),
            "this native service entry supports ROM/save grants without ambient file or microphone access"
        );
        "game_main"
    } else {
        ensure!(
            manifest.native_tick,
            "native compatibility profile required"
        );
        "game_tick"
    };
    #[cfg(target_os = "linux")]
    let target = Target::Linux64;
    #[cfg(all(windows, target_arch = "x86_64"))]
    let target = Target::Windows64;
    #[cfg(all(windows, target_arch = "x86"))]
    let target = Target::Windows32;
    Plan::new(target, entry, vec![], LocalLayout::None, true)
}

fn key(root: &Path, create: bool) -> Result<SigningKey> {
    let path = root.join(".native-build-key");
    if create && !path.exists() {
        let mut seed = [0; 32];
        crate::random::fill(&mut seed)?;
        let mut file = tempfile::NamedTempFile::new_in(root)?;
        file.write_all(&seed)?;
        file.as_file().sync_all()?;
        match file.persist_noclobber(&path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.error.into()),
        }
    }
    let bytes = crate::files::read_regular(&path, 32, false)?;
    Ok(SigningKey::from_bytes(
        &bytes.try_into().ok().context("invalid native cache key")?,
    ))
}

#[cfg(target_os = "linux")]
fn open(root: &Path, id: &str) -> Result<crate::archive::Package> {
    crate::registry::installed_manifest(root, id)?;
    crate::archive::Package::open(&root.join(format!("{id}.gamepkg")))
}

fn prepare_graphics(
    root: &Path,
    sources: &CheckedSources,
    package: &crate::archive::Package,
    settings: &crate::settings::Settings,
) -> Result<Option<VerifiedPipelines>> {
    let manifest = sources.manifest();
    use eframe::wgpu;
    let stage = |stage| {
        manifest
            .shaders
            .iter()
            .find(|shader| shader.stage == stage)
            .cloned()
            .context("missing native Slang stage")
    };
    let recipe = match manifest.native_graphics {
        Some(NativeGraphics::VertexColorV1) => PipelineRecipe {
            vertex: stage("vertex")?,
            fragment: stage("fragment")?,
            source_hashes: None,
            stride: 32,
            attributes: wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4].to_vec(),
            uniform_bytes: 64,
            textures: Vec::new(),
            blend: None,
            cull: None,
            depth_write: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            raster: Default::default(),
            uniform_policy: Default::default(),
        },
        Some(NativeGraphics::TexturedV1) => textured_v1_recipe(),
        None => return Ok(None),
    };
    VerifiedPipelines::prepare(root, sources, package, settings, vec![recipe], 1).map(Some)
}

fn source_snapshot(
    root: &Path,
    id: &str,
) -> Result<(
    CheckedSources,
    crate::archive::Package,
    tempfile::NamedTempFile,
)> {
    let snapshot = crate::files::snapshot_archive(&root.join(format!("{id}.gamepkg")))?;
    let mut package = crate::archive::Package::open(snapshot.path())?;
    let sources = CheckedSources::verify(&mut package)?;
    ensure!(
        sources.manifest().id == id,
        "installed native package ID changed"
    );
    Ok((sources, package, snapshot))
}

pub(super) fn build(root: &Path, id: &str) -> Result<()> {
    let _lock = crate::build::BuildChange::acquire(root, id)?;
    let path = super::cache_path(root, id)?;
    let (sources, package, _snapshot) = source_snapshot(root, id)?;
    let plan = plan(sources.manifest())?;
    let pipelines = if sources.manifest().native_graphics.is_some() {
        prepare_graphics(root, &sources, &package, &crate::settings::load()?)?
    } else {
        None
    };
    let artifact = preparation::build_in_process(root, &sources, &plan, pipelines)?;
    ensure!(
        super::cache_path(root, id)? == path,
        "installed game changed during the build"
    );
    let receipt = serde_json::to_vec(artifact.receipt())?;
    let signature = key(root, true)?.sign(&receipt);
    let output = artifact.executable()?;
    fs::create_dir_all(path.parent().context("native cache directory")?)?;
    let bytes = crate::files::read_regular(&output, super::MAX_COMPATIBILITY_BINARY_BYTES, false)?;
    write(&path, &bytes)?;
    crate::integrity::record_build(&path, &bytes)?;
    let mut authenticated = signature.to_bytes().to_vec();
    authenticated.extend(receipt);
    write(&path.with_extension("receipt"), &authenticated)
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().context("native cache directory")?)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}

fn prepared(
    root: &Path,
    id: &str,
    settings: Option<&crate::settings::Settings>,
) -> Result<(CheckedSources, ExecutableReceipt, Vec<u8>)> {
    let path = super::cache_path(root, id)?;
    let auth = crate::files::read_regular(&path.with_extension("receipt"), 2 * 1024 * 1024, false)
        .context("Build this game before playing")?;
    let (signature, json) = auth
        .split_at_checked(64)
        .context("invalid native build receipt")?;
    key(root, false)?
        .verifying_key()
        .verify(json, &Signature::from_slice(signature)?)?;
    let receipt: ExecutableReceipt = serde_json::from_slice(json)?;

    let (sources, package, _snapshot) = source_snapshot(root, id)?;
    let shader_preparation = if sources.manifest().native_graphics.is_some() {
        let loaded;
        let settings = match settings {
            Some(settings) => settings,
            None => {
                loaded = crate::settings::load()?;
                &loaded
            }
        };
        vec![crate::shaders::native_preparation_identity(
            root,
            &package.archive_digest()?,
            settings,
        )?]
    } else {
        Vec::new()
    };
    ensure!(
        receipt.game.source_kit == sources.identity()
            && receipt.game.sdk == crate::port_host::build_identity()
            && receipt.game.policy == crate::build_policy::REVISION
            && receipt.plan == plan(sources.manifest())?
            && receipt.bootstrap == Bootstrap::InProcess
            && receipt.game.shaders.len() == shader_preparation.len()
            && receipt.shader_preparation == shader_preparation,
        "native build receipt is stale; rebuild from verified source"
    );
    let bytes = crate::files::read_regular(&path, super::MAX_COMPATIBILITY_BINARY_BYTES, false)?;
    ensure!(
        crate::verify::digest(&bytes) == receipt.binary,
        "native executable changed"
    );
    Ok((sources, receipt, bytes))
}

pub(super) fn ready(root: &Path, id: &str) -> bool {
    prepared(root, id, None).is_ok()
}

pub(super) fn play_entry(
    root: &Path,
    id: &str,
    supplied: &[PathBuf],
    settings: &crate::settings::Settings,
    graphics: crate::launch_options::LaunchArgs,
    smoke: bool,
) -> Result<()> {
    settings.validate()?;
    let build_lock = crate::build::BuildChange::read(root, id)?;
    let (sources, receipt, bytes) = prepared(root, id, Some(settings))?;
    let manifest = sources.manifest();
    ensure!(
        manifest.limits.hard_memory_bytes <= settings.memory_ceiling_mib * 1048576,
        "increase the memory ceiling in Settings to at least {} MiB for this game",
        manifest.limits.hard_memory_bytes.div_ceil(1048576)
    );
    let roms = selected_roms(root, manifest, supplied)?;
    let options = NativeLaunch {
        graphics,
        visible: !smoke,
        mute_audio: settings.mute_audio || smoke,
        volume: settings.audio_volume,
    };
    let artifact = Artifact::from_verified_cache(manifest.clone(), receipt, &bytes)?;
    #[cfg(target_os = "linux")]
    let mut process = {
        let assets = if manifest.native_tick {
            VerifiedAssets::stage(&mut open(root, id)?, sources.identity())?
        } else {
            None
        };
        let game_data = if let Some(data) = &manifest.game_data {
            let path = crate::game_data::load(root, manifest)?.context(
                "select the required game-data folder in the library or use --game-data",
            )?;
            crate::game_data::open_native_directories(data, &path)?
        } else {
            Vec::new()
        };
        Process::start_in_process_with_data(
            &artifact,
            root,
            roms,
            manifest
                .permissions
                .save
                .then_some((root, manifest.native_save_quota())),
            assets.as_ref(),
            game_data,
            options,
        )?
    };
    #[cfg(windows)]
    let mut process = Process::start_windows(&artifact, root, roms, options)?;
    drop(build_lock);
    let start = Instant::now();
    loop {
        if let Some(code) = process.poll()? {
            ensure!(
                code == 0
                    && !smoke
                    && (!manifest.uses_service_runtime() || process.first_frame_presented()),
                "native game exited before expected completion ({code}): {}",
                process.diagnostics()
            );
            return Ok(());
        }
        if smoke && process.first_frame_presented() {
            process.stop()?;
            return Ok(());
        }
        ensure!(
            process.first_frame_presented() || start.elapsed() < Duration::from_secs(60),
            "native game failed to present: {}",
            process.diagnostics()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn selected_roms(root: &Path, manifest: &Manifest, supplied: &[PathBuf]) -> Result<Vec<fs::File>> {
    let remembered;
    let paths = if supplied.is_empty() {
        remembered = crate::rom_selections::load(root, manifest)?
            .into_iter()
            .filter(|path| !path.trim().is_empty())
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        &remembered
    } else {
        supplied
    };
    ensure!(
        paths.len() == manifest.required_rom_count(),
        "select the required ROM in the library or supply it with --rom"
    );
    paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let file = crate::files::open_regular(path, crate::files::MAX_NATIVE_ROM_BYTES, true)
                .with_context(|| format!("unable to open selected ROM {}", index + 1))?;
            ensure!(
                file.metadata()?.len() != 0,
                "selected ROM {} is empty",
                index + 1
            );
            Ok(file)
        })
        .collect()
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::*;

    #[test]
    #[ignore = "builds and runs a signed game via the confined Linux desktop launcher"]
    fn signed_game_reads_only_the_declared_installation_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = &temp.path().join("library");
        let recipe = temp.path().join("recipe");
        fs::create_dir_all(recipe.join("source/src")).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal");
        for file in [
            "CREDITS.md",
            "LICENSE",
            "NOTICE",
            "thumbnail.png",
            "source/Cargo.toml",
            "source/Cargo.lock",
        ] {
            fs::copy(fixture.join(file), recipe.join(file)).unwrap();
        }
        let original = include_str!("../../examples/minimal/manifest.toml")
            .replace("com.gamepkg.minimal", "com.gamepkg.game-data-test")
            .replace(
                "hard_memory_bytes = 16777216",
                "hard_memory_bytes = 536870912",
            );
        fs::write(
            recipe.join("manifest.toml"),
            format!(
                r#"native_services = true
{original}
[game_data]
title = "Fixture installation"
[[game_data.layouts]]
directories = ["packages", "data"]
paths = ["data/start.cfg", "packages/picture"]
min_bytes = 1
"#
            ),
        )
        .unwrap();
        fs::write(
            recipe.join("source/src/lib.rs"),
            r#"pub fn game_main() {
    let grants = match gamepkg::grants() { Ok(v) => v, Err(_) => return };
    if grants.len() != 2 || grants[0].kind != 6 || grants[1].kind != 6 { return; }
    for (index, name, expected) in [(0, "picture", b"package"), (1, "start.cfg", b"startup")] {
        let root = grants[index].id;
        if gamepkg::file_exists(root, "private") != Ok(false) { return; }
        if gamepkg::file_open(root, true, name) != Err(-4) { return; }
        let file = match gamepkg::file_open(root, false, name) { Ok(v) => v, Err(_) => return };
        let mut bytes = [0u8;7];
        if gamepkg::file_read(file, 0, &mut bytes) != Ok(7) || &bytes != expected { return; }
        if gamepkg::file_close(file) != Ok(()) { return; }
    }
    if gamepkg::present(0x295277ff, &[]).is_err() { return; }
    loop {
        let (now, generation) = match gamepkg::monotonic() {
            Ok(value) => value, Err(_) => return,
        };
        if gamepkg::wait_until(now.saturating_add(10000000), generation).is_err() { return; }
    }
}"#,
        )
        .unwrap();

        let path = temp.path().join("fixture.gamepkg");
        crate::pack::pack(&recipe, &path, &[97; 32]).unwrap();
        let manifest = crate::registry::install(root, &path, true).unwrap();
        crate::native::compile(root, &manifest.id).unwrap();
        let settings = crate::settings::Settings {
            memory_ceiling_mib: 512,
            mute_audio: true,
            ..Default::default()
        };
        let play = || play_entry(root, &manifest.id, &[], &settings, Default::default(), true);
        assert!(play()
            .unwrap_err()
            .to_string()
            .contains("select the required game-data folder"));
        let installation = temp.path().join("installation");
        fs::create_dir_all(installation.join("packages")).unwrap();
        fs::create_dir(installation.join("data")).unwrap();
        fs::write(installation.join("packages/picture"), b"package").unwrap();
        fs::write(installation.join("data/start.cfg"), b"startup").unwrap();
        fs::write(installation.join("private"), b"outside").unwrap();
        crate::game_data::remember(root, &manifest, &installation).unwrap();
        play().unwrap();
        assert_eq!(
            fs::read(installation.join("data/start.cfg")).unwrap(),
            b"startup"
        );
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    #[ignore = "requires an already built isolated Doom preview library on native Windows"]
    fn prebuilt_doom_presents_on_the_windows_desktop() {
        let library = std::env::var_os("GAMEPKG_TEST_DOOM_NATIVE_LIBRARY").unwrap();
        let selected = std::env::var_os("GAMEPKG_TEST_DOOM_ROMS").unwrap();
        let paths = std::env::split_paths(&selected).collect::<Vec<_>>();
        let original = paths
            .iter()
            .map(|path| crate::files::hash_archive(path).unwrap())
            .collect::<Vec<_>>();
        let root = Path::new(&library);
        let id = "com.gamepkg.doom";
        let manifest = crate::registry::installed_manifest(root, id).unwrap();
        assert!(crate::native::is_built(root, id).unwrap());
        assert_eq!(manifest.required_rom_count(), paths.len());
        let settings = crate::settings::Settings {
            mute_audio: true,
            ..Default::default()
        };
        play_entry(root, id, &paths, &settings, Default::default(), true).unwrap();
        for (path, hash) in paths.iter().zip(original) {
            assert_eq!(crate::files::hash_archive(path).unwrap(), hash);
        }
    }

    #[test]
    #[ignore = "requires native Windows with AppContainer, Vulkan, signed Doom and selected WADs"]
    fn signed_doom_uses_the_normal_windows_build_and_play_path() {
        let package = std::env::var_os("GAMEPKG_TEST_DOOM_NATIVE_PACKAGE").unwrap();
        let selected = std::env::var_os("GAMEPKG_TEST_DOOM_ROMS").unwrap();
        let paths = std::env::split_paths(&selected).collect::<Vec<_>>();
        let before = paths
            .iter()
            .map(|path| crate::files::hash_archive(path).unwrap())
            .collect::<Vec<_>>();
        let library = tempfile::tempdir().unwrap();
        let root = library.path();
        let manifest = crate::registry::install(root, Path::new(&package), true).unwrap();
        assert_eq!(manifest.id, "com.gamepkg.doom");
        assert!(crate::native::build_supported(&manifest));
        assert_eq!(manifest.required_rom_count(), paths.len());
        crate::native::compile(root, &manifest.id).unwrap();
        assert!(crate::native::is_built(root, &manifest.id).unwrap());
        let settings = crate::settings::Settings {
            mute_audio: true,
            ..Default::default()
        };
        play_entry(
            root,
            &manifest.id,
            &paths,
            &settings,
            Default::default(),
            true,
        )
        .unwrap();
        for (path, hash) in paths.iter().zip(before) {
            assert_eq!(crate::files::hash_archive(path).unwrap(), hash);
        }
        fs::write(
            crate::native::cache_path(root, &manifest.id).unwrap(),
            b"test",
        )
        .unwrap();
        assert!(!crate::native::is_built(root, &manifest.id).unwrap());
    }
}
