use crate::port_host::{
    admission::CheckedSources,
    owner::VerifiedAssets,
    preparation::{self, Artifact, Bootstrap, ExecutableReceipt, LocalLayout, Plan, Target},
    process::{NativeLaunch, Process},
};
use anyhow::{ensure, Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
use std::{
    fs,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

fn plan() -> Result<Plan> {
    Plan::new(
        Target::Linux64,
        "game_tick",
        vec![],
        LocalLayout::None,
        true,
    )
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

pub(super) fn build(root: &Path, id: &str) -> Result<()> {
    let _lock = crate::build::BuildChange::acquire(root, id)?;
    let path = super::cache_path(root, id)?;
    let sources = CheckedSources::verify(&mut open(root, id)?)?;
    ensure!(
        sources.manifest().native_tick,
        "native tick profile required"
    );
    let artifact = preparation::build_in_process(root, &sources, &plan()?, None)?;
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

fn prepared(root: &Path, id: &str) -> Result<(CheckedSources, ExecutableReceipt, Vec<u8>)> {
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
    let sources = CheckedSources::verify(&mut open(root, id)?)?;
    ensure!(
        sources.manifest().native_tick
            && receipt.game.source_kit == sources.identity()
            && receipt.game.sdk == crate::port_host::build_identity()
            && receipt.game.policy == crate::build_policy::REVISION
            && receipt.plan == plan()?
            && receipt.bootstrap == Bootstrap::InProcess,
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
    prepared(root, id).is_ok()
}

pub fn play(
    root: &Path,
    id: &str,
    settings: &crate::settings::Settings,
    graphics: crate::launch_options::LaunchArgs,
    smoke: bool,
) -> Result<()> {
    let build_lock = crate::build::BuildChange::read(root, id)?;
    let (sources, receipt, bytes) = prepared(root, id)?;
    ensure!(
        sources.manifest().limits.hard_memory_bytes <= settings.memory_ceiling_mib * 1048576,
        "the native tick build requires its signed memory ceiling"
    );
    let assets = VerifiedAssets::stage(&mut open(root, id)?, sources.identity())?;
    let artifact = Artifact::from_verified_cache(sources.manifest().clone(), receipt, &bytes)?;
    let mut process = Process::start_in_process_with_assets(
        &artifact,
        root,
        None,
        sources
            .manifest()
            .permissions
            .save
            .then_some((root, 2 * 1048576)),
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
                code == 0 && !smoke,
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
