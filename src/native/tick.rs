use crate::manifest::{Manifest, NativeGraphics};
use crate::port_host::{
    admission::CheckedSources,
    graphics::{PipelineRecipe, VerifiedPipelines},
    owner::VerifiedAssets,
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
    Plan::new(Target::Linux64, entry, vec![], LocalLayout::None, true)
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

fn open(root: &Path, id: &str) -> Result<crate::archive::Package> {
    crate::registry::installed_manifest(root, id)?;
    crate::archive::Package::open(&root.join(format!("{id}.gamepkg")))
}

fn prepare_graphics(
    root: &Path,
    sources: &CheckedSources,
    archive: &[u8],
    settings: &crate::settings::Settings,
) -> Result<Option<VerifiedPipelines>> {
    let manifest = sources.manifest();
    let Some(NativeGraphics::VertexColorV1) = manifest.native_graphics else {
        return Ok(None);
    };
    use eframe::wgpu;
    let stage = |stage| {
        manifest
            .shaders
            .iter()
            .find(|shader| shader.stage == stage)
            .cloned()
            .context("missing native Slang stage")
    };
    let recipe = PipelineRecipe {
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
    };
    VerifiedPipelines::prepare(root, sources, archive, settings, vec![recipe], 1).map(Some)
}

fn source_snapshot(root: &Path, id: &str) -> Result<(CheckedSources, Vec<u8>)> {
    let bytes = crate::files::read_regular(
        &root.join(format!("{id}.gamepkg")),
        crate::archive::MAX_TOTAL,
        false,
    )?;
    let mut snapshot = tempfile::NamedTempFile::new()?;
    snapshot.write_all(&bytes)?;
    let sources = CheckedSources::verify(&mut crate::archive::Package::open(snapshot.path())?)?;
    ensure!(
        sources.manifest().id == id,
        "installed native package ID changed"
    );
    Ok((sources, bytes))
}

pub(super) fn build(root: &Path, id: &str) -> Result<()> {
    let _lock = crate::build::BuildChange::acquire(root, id)?;
    let path = super::cache_path(root, id)?;
    let (sources, bytes) = source_snapshot(root, id)?;
    let plan = plan(sources.manifest())?;
    let pipelines = if sources.manifest().native_graphics.is_some() {
        prepare_graphics(root, &sources, &bytes, &crate::settings::load()?)?
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
    let bytes = crate::files::read_regular(&output, 256 * 1024 * 1024, false)?;
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

    let (sources, archive) = source_snapshot(root, id)?;
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
            root, &archive, settings,
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
    let bytes = crate::files::read_regular(&path, 256 * 1024 * 1024, false)?;
    ensure!(
        crate::verify::digest(&bytes) == receipt.binary,
        "native executable changed"
    );
    Ok((sources, receipt, bytes))
}

pub(super) fn ready(root: &Path, id: &str) -> bool {
    prepared(root, id, None).is_ok()
}

pub fn play(
    root: &Path,
    id: &str,
    settings: &crate::settings::Settings,
    graphics: crate::launch_options::LaunchArgs,
    smoke: bool,
) -> Result<()> {
    play_entry(root, id, &[], settings, graphics, smoke)
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
    let rom = selected_rom(root, manifest, supplied)?;
    let save_quota = if manifest.native_tick {
        2 * 1048576
    } else {
        crate::saves::MAX_SAVE as u64
    };
    let assets = if manifest.native_tick {
        VerifiedAssets::stage(&mut open(root, id)?, sources.identity())?
    } else {
        None
    };
    let artifact = Artifact::from_verified_cache(manifest.clone(), receipt, &bytes)?;
    let mut process = Process::start_in_process_with_assets(
        &artifact,
        root,
        rom,
        manifest.permissions.save.then_some((root, save_quota)),
        assets.as_ref(),
        NativeLaunch {
            graphics,
            visible: !smoke,
            mute_audio: settings.mute_audio || smoke,
            volume: settings.audio_volume,
        },
    )?;
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

fn selected_rom(
    root: &Path,
    manifest: &Manifest,
    supplied: &[PathBuf],
) -> Result<Option<fs::File>> {
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
    let Some(path) = paths.first() else {
        return Ok(None);
    };
    let file = crate::files::open_regular(path, crate::files::MAX_NATIVE_ROM_BYTES, true)
        .context("unable to open the selected ROM")?;
    ensure!(file.metadata()?.len() != 0, "the selected ROM is empty");
    Ok(Some(file))
}
