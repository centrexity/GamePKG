use crate::manifest::{Manifest, NativeGraphics};
#[cfg(target_os = "linux")]
use crate::port_host::owner::VerifiedAssets;
use crate::port_host::{
    admission::CheckedSources,
    graphics::{textured_v1_recipe, textured_v2_recipe, PipelineRecipe, VerifiedPipelines},
    preparation::{self, Artifact, Bootstrap, ExecutableReceipt, LocalLayout, Plan, Target},
    process::{NativeLaunch, Process},
};
use crate::registry::VerifiedIdentity;
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
        Some(NativeGraphics::TexturedV2) => textured_v2_recipe(),
        None => return Ok(None),
    };
    VerifiedPipelines::prepare(root, sources, package, settings, vec![recipe], 1).map(Some)
}

type SourceSnapshot = (
    CheckedSources,
    crate::archive::Package,
    Option<tempfile::NamedTempFile>,
);
type PreparedSources = (
    CheckedSources,
    crate::archive::Package,
    Option<tempfile::NamedTempFile>,
    ExecutableReceipt,
    Vec<u8>,
);

fn source_snapshot_with_policy(root: &Path, id: &str, scan_source: bool) -> Result<SourceSnapshot> {
    let (bytes, digest) = crate::files::archive_snapshot(&root.join(format!("{id}.gamepkg")))?;
    let mut package = crate::archive::Package::open_snapshot(bytes, digest)?;
    let sources = if scan_source {
        CheckedSources::verify(&mut package)?
    } else {
        CheckedSources::verify_authenticated(&mut package)?
    };
    ensure!(
        sources.manifest().id == id,
        "installed native package ID changed"
    );
    Ok((sources, package, None))
}

fn source_snapshot(root: &Path, id: &str) -> Result<SourceSnapshot> {
    source_snapshot_with_policy(root, id, true)
}

pub(super) fn build(root: &Path, id: &str) -> Result<()> {
    let _lock = crate::build::BuildChange::acquire(root, id)?;
    let (sources, package, _snapshot) = source_snapshot(root, id)?;
    let archive_digest = package.archive_digest()?;
    let identity = VerifiedIdentity {
        tree_identity: sources.identity().to_owned(),
        archive_digest: archive_digest.clone(),
    };
    let path = super::cache_path_for_identity(root, &identity, sources.manifest());
    let plan = plan(sources.manifest())?;
    let pipelines = if sources.manifest().native_graphics.is_some() {
        prepare_graphics(root, &sources, &package, &crate::settings::load()?)?
    } else {
        None
    };
    let artifact = preparation::build_in_process(root, &sources, &plan, pipelines)?;
    ensure!(
        crate::files::hash_archive(&root.join(format!("{id}.gamepkg")))? == archive_digest,
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

fn receipt_policy_matches(
    receipt: &ExecutableReceipt,
    manifest: &Manifest,
    identity: &VerifiedIdentity,
    shader_preparation: &[String],
) -> Result<bool> {
    Ok(receipt.format == preparation::RECEIPT_FORMAT
        && receipt.source.is_well_formed()
        && receipt.source.scanner_policy == crate::source::SCAN_POLICY
        && receipt.source.archive_digest == identity.archive_digest
        && receipt.source.tree_identity == identity.tree_identity
        && receipt.source.source_kit == identity.tree_identity
        && receipt.game.source_kit == identity.tree_identity
        && receipt.game.sdk == crate::port_host::build_identity()
        && receipt.game.policy == crate::build_policy::REVISION
        && receipt.plan == plan(manifest)?
        && receipt.bootstrap == Bootstrap::InProcess
        && receipt.game.shaders.len() == shader_preparation.len()
        && receipt.shader_preparation.as_slice() == shader_preparation)
}

fn verified_executable(
    root: &Path,
    manifest: &Manifest,
    identity: &VerifiedIdentity,
    settings: Option<&crate::settings::Settings>,
) -> Result<(ExecutableReceipt, Vec<u8>)> {
    let path = super::cache_path_for_identity(root, identity, manifest);
    let auth = crate::files::read_regular(&path.with_extension("receipt"), 2 * 1024 * 1024, false)
        .context("Build this game before playing")?;
    let (signature, json) = auth
        .split_at_checked(64)
        .context("invalid native build receipt")?;
    key(root, false)?
        .verifying_key()
        .verify(json, &Signature::from_slice(signature)?)?;
    let receipt: ExecutableReceipt = serde_json::from_slice(json).context(
        "native build receipt lacks a valid source attestation; rebuild from verified source",
    )?;
    ensure!(
        receipt.format == preparation::RECEIPT_FORMAT,
        "native build receipt is stale; rebuild from verified source"
    );
    let shader_preparation = if manifest.native_graphics.is_some() {
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
            &identity.archive_digest,
            settings,
        )?]
    } else {
        Vec::new()
    };
    ensure!(
        receipt_policy_matches(&receipt, manifest, identity, &shader_preparation)?,
        "native build receipt is stale; rebuild from verified source"
    );
    let bytes = crate::files::read_regular(&path, super::MAX_COMPATIBILITY_BINARY_BYTES, false)?;
    ensure!(
        crate::verify::digest(&bytes) == receipt.binary,
        "native executable changed"
    );
    Ok((receipt, bytes))
}

fn prepared(
    root: &Path,
    id: &str,
    settings: Option<&crate::settings::Settings>,
) -> Result<PreparedSources> {
    let (sources, package, snapshot) = source_snapshot_with_policy(root, id, false)?;
    let identity = VerifiedIdentity {
        tree_identity: sources.identity().to_owned(),
        archive_digest: package.archive_digest()?,
    };
    let (receipt, bytes) = verified_executable(root, sources.manifest(), &identity, settings)?;
    Ok((sources, package, snapshot, receipt, bytes))
}

pub(super) fn ready_verified(
    root: &Path,
    manifest: &Manifest,
    identity: &VerifiedIdentity,
) -> bool {
    verified_executable(root, manifest, identity, None).is_ok()
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
    #[cfg(target_os = "linux")]
    let (sources, mut package, _snapshot, receipt, bytes) = prepared(root, id, Some(settings))?;
    #[cfg(not(target_os = "linux"))]
    let (sources, _package, _snapshot, receipt, bytes) = prepared(root, id, Some(settings))?;
    let manifest = sources.manifest();
    super::require_compatibility(manifest)?;
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
            VerifiedAssets::stage(&mut package, sources.identity())?
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
    use crate::native::cache_path_for_identity;
    use crate::port_host::{
        admission::{ObjectHash, Receipt},
        preparation::SourceAttestation,
    };

    type ReceiptMutation = (&'static str, Box<dyn Fn(&mut ExecutableReceipt)>);
    static DESKTOP_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct DesktopEnv {
        previous_display: Option<std::ffi::OsString>,
        previous_runtime: Option<std::ffi::OsString>,
        _runtime: tempfile::TempDir,
        _socket: std::os::unix::net::UnixListener,
    }

    impl DesktopEnv {
        fn install() -> Self {
            let runtime = tempfile::tempdir().unwrap();
            let display = format!("gamepkg-test-{}", std::process::id());
            let socket = std::os::unix::net::UnixListener::bind(runtime.path().join(&display))
                .expect("create launch test Wayland socket");
            let previous_display = std::env::var_os("WAYLAND_DISPLAY");
            let previous_runtime = std::env::var_os("XDG_RUNTIME_DIR");
            std::env::set_var("WAYLAND_DISPLAY", &display);
            std::env::set_var("XDG_RUNTIME_DIR", runtime.path().as_os_str());
            Self {
                previous_display,
                previous_runtime,
                _runtime: runtime,
                _socket: socket,
            }
        }
    }

    impl Drop for DesktopEnv {
        fn drop(&mut self) {
            match &self.previous_display {
                Some(value) => std::env::set_var("WAYLAND_DISPLAY", value),
                None => std::env::remove_var("WAYLAND_DISPLAY"),
            }
            match &self.previous_runtime {
                Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
                None => std::env::remove_var("XDG_RUNTIME_DIR"),
            }
        }
    }

    fn install_minimal(root: &Path, id: &str, source: &str) {
        install_recipe(root, id, source, "native_services = true\n");
    }

    fn install_native_tick(root: &Path, id: &str, source: &str) {
        install_recipe(root, id, source, "native_tick = true\n");
    }

    fn install_recipe(root: &Path, id: &str, source: &str, profile: &str) {
        let recipe = root.join("recipe");
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
        let manifest =
            include_str!("../../examples/minimal/manifest.toml").replace("com.gamepkg.minimal", id);
        fs::write(recipe.join("manifest.toml"), format!("{profile}{manifest}")).unwrap();
        fs::write(recipe.join("source/src/lib.rs"), source).unwrap();
        let package = root.join(format!("{id}.fixture.gamepkg"));
        crate::pack::pack(&recipe, &package, &[97; 32]).unwrap();
        crate::registry::install(root, &package, true).unwrap();
    }

    fn fake_receipt(
        manifest: &Manifest,
        identity: &VerifiedIdentity,
        binary: &[u8],
    ) -> ExecutableReceipt {
        ExecutableReceipt {
            format: preparation::RECEIPT_FORMAT,
            game: Receipt {
                policy: crate::build_policy::REVISION.into(),
                sdk: crate::port_host::build_identity().to_string(),
                source_kit: identity.tree_identity.clone(),
                compiler: "ab".repeat(32),
                auditor: "cd".repeat(32),
                target: "x86_64-unknown-linux-gnu".into(),
                shaders: Vec::new(),
                objects: vec![ObjectHash {
                    name: "game-0000.o".into(),
                    sha256: "ef".repeat(32),
                }],
            },
            plan: plan(manifest).unwrap(),
            cpu: "portable".into(),
            toolchain: "00".repeat(32),
            arguments: Vec::new(),
            runtime: "11".repeat(32),
            shader_preparation: Vec::new(),
            binary: crate::verify::digest(binary),
            source: SourceAttestation {
                scanner_policy: crate::source::SCAN_POLICY.into(),
                archive_digest: identity.archive_digest.clone(),
                tree_identity: identity.tree_identity.clone(),
                source_kit: identity.tree_identity.clone(),
            },
            bootstrap: Bootstrap::InProcess,
            raw_objects: vec![ObjectHash {
                name: "game-0000.o".into(),
                sha256: "ef".repeat(32),
            }],
            object_normalization: Default::default(),
        }
    }

    fn write_signed_receipt(root: &Path, path: &Path, json: &[u8]) {
        let signature = key(root, true).unwrap().sign(json);
        let mut authenticated = signature.to_bytes().to_vec();
        authenticated.extend(json);
        write(path, &authenticated).unwrap();
    }

    fn stage_fake_build(root: &Path, id: &str) -> (Manifest, VerifiedIdentity, PathBuf, Vec<u8>) {
        stage_fake_build_with(
            root,
            id,
            "pub fn game_main() {}",
            b"signed native fixture".to_vec(),
            false,
        )
    }

    fn stage_fake_build_with(
        root: &Path,
        id: &str,
        source: &str,
        binary: Vec<u8>,
        native_tick: bool,
    ) -> (Manifest, VerifiedIdentity, PathBuf, Vec<u8>) {
        if native_tick {
            install_native_tick(root, id, source);
        } else {
            install_minimal(root, id, source);
        }
        let (sources, package, _snapshot) = source_snapshot_with_policy(root, id, false).unwrap();
        let manifest = sources.manifest().clone();
        let identity = VerifiedIdentity {
            tree_identity: sources.identity().into(),
            archive_digest: package.archive_digest().unwrap(),
        };
        let receipt = fake_receipt(&manifest, &identity, &binary);
        let path = cache_path_for_identity(root, &identity, &manifest);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write(&path, &binary).unwrap();
        write_signed_receipt(
            root,
            &path.with_extension("receipt"),
            &serde_json::to_vec(&receipt).unwrap(),
        );
        crate::source::scan_probe::reset();
        (manifest, identity, path, binary)
    }

    #[test]
    fn launch_preparation_authenticates_without_repeating_source_admission() {
        let temp = tempfile::tempdir().unwrap();
        let root = &temp.path().join("library");
        let id = "com.gamepkg.launch-attestation";
        let (_manifest, identity, _path, _binary) = stage_fake_build(root, id);
        let (_sources, _package, _snapshot, receipt, _bytes) = prepared(root, id, None).unwrap();
        assert_eq!(crate::source::scan_probe::count(), 0);
        assert_eq!(receipt.source.archive_digest, identity.archive_digest);
        assert_eq!(receipt.source.tree_identity, identity.tree_identity);
        assert_eq!(receipt.source.source_kit, identity.tree_identity);
    }

    #[test]
    fn correctly_attested_package_launches_without_source_scan() {
        let _env_lock = DESKTOP_ENV.lock().unwrap();
        let _desktop = DesktopEnv::install();
        let temp = tempfile::tempdir().unwrap();
        let root = &temp.path().join("library");
        let id = "com.gamepkg.attested-launch";
        let executable = fs::read("/bin/true").unwrap();
        stage_fake_build_with(root, id, "pub fn game_main() {}", executable, true);
        let settings = crate::settings::Settings {
            memory_ceiling_mib: 512,
            mute_audio: true,
            ..Default::default()
        };
        crate::source::scan_probe::reset();
        play_entry(root, id, &[], &settings, Default::default(), false).unwrap();
        assert_eq!(crate::source::scan_probe::count(), 0);
    }

    #[test]
    fn play_entry_never_scans_source_during_launch_preparation() {
        let temp = tempfile::tempdir().unwrap();
        let root = &temp.path().join("library");
        let id = "com.gamepkg.play-no-scan";
        install_minimal(root, id, "pub fn game_main() {}");
        let settings = crate::settings::Settings {
            memory_ceiling_mib: 512,
            mute_audio: true,
            ..Default::default()
        };
        crate::source::scan_probe::reset();
        assert!(play_entry(root, id, &[], &settings, Default::default(), true).is_err());
        assert_eq!(crate::source::scan_probe::count(), 0);
    }

    #[test]
    fn build_admission_still_runs_the_authoritative_source_scan() {
        let temp = tempfile::tempdir().unwrap();
        let root = &temp.path().join("library");
        let id = "com.gamepkg.build-admission";
        install_minimal(root, id, "pub fn game_main() {}");
        crate::source::scan_probe::reset();
        let (bytes, digest) =
            crate::files::archive_snapshot(&root.join(format!("{id}.gamepkg"))).unwrap();
        let mut package = crate::archive::Package::open_snapshot(bytes, digest).unwrap();
        CheckedSources::verify(&mut package).unwrap();
        assert_eq!(crate::source::scan_probe::count(), 1);
    }

    #[test]
    fn modified_signed_source_fails_before_native_launch() {
        let temp = tempfile::tempdir().unwrap();
        let root = &temp.path().join("library");
        let id = "com.gamepkg.source-tamper";
        let (_manifest, _identity, _path, _binary) = stage_fake_build(root, id);

        // A fresh signed package with different admitted source but the same ID
        // must not inherit the old receipt's attestation.
        let other = temp.path().join("other-library");
        install_minimal(&other, id, "pub fn game_main() { let _ = 1; }");
        let changed = fs::read(other.join(format!("{id}.gamepkg"))).unwrap();
        write(&root.join(format!("{id}.gamepkg")), &changed).unwrap();
        assert!(prepared(root, id, None).is_err());
    }

    #[test]
    fn receipt_from_another_package_fails_before_native_launch() {
        let temp = tempfile::tempdir().unwrap();
        let root = &temp.path().join("library");
        let source_id = "com.gamepkg.receipt-source";
        let target_id = "com.gamepkg.receipt-target";
        let (_, _, source_path, _) = stage_fake_build(root, source_id);
        let (_, _, target_path, _) = stage_fake_build(root, target_id);

        let receipt = fs::read(source_path.with_extension("receipt")).unwrap();
        write(&target_path.with_extension("receipt"), &receipt).unwrap();
        let error = match prepared(root, target_id, None) {
            Ok(_) => panic!("receipt from another package was accepted"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("rebuild from verified source"), "{error}");
    }

    #[test]
    fn stale_tampered_and_older_receipts_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let root = &temp.path().join("library");
        let id = "com.gamepkg.receipt-tamper";
        let (manifest, identity, path, binary) = stage_fake_build(root, id);
        let receipt_path = path.with_extension("receipt");
        let original = serde_json::to_vec(&fake_receipt(&manifest, &identity, &binary)).unwrap();
        verified_executable(root, &manifest, &identity, None).unwrap();

        let other = VerifiedIdentity {
            tree_identity: "aa".repeat(32),
            archive_digest: "bb".repeat(32),
        };
        let mutations: Vec<ReceiptMutation> = vec![
            (
                "stale scanner policy",
                Box::new(|r| r.source.scanner_policy = "v0".into()),
            ),
            (
                "previous scanner policy",
                Box::new(|r| r.source.scanner_policy = "gamepkg-source-scan-v1".into()),
            ),
            ("stale receipt format", Box::new(|r| r.format = 1)),
            ("stale SDK", Box::new(|r| r.game.sdk = "stale".into())),
            (
                "stale build policy",
                Box::new(|r| r.game.policy = "stale".into()),
            ),
            (
                "receipt from another package",
                Box::new(move |r| {
                    r.source.archive_digest = other.archive_digest.clone();
                    r.source.tree_identity = other.tree_identity.clone();
                    r.source.source_kit = other.tree_identity.clone();
                    r.game.source_kit = other.tree_identity.clone();
                }),
            ),
            (
                "unknown sampler-free bootstrap",
                Box::new(|r| r.bootstrap = Bootstrap::HistoricalPipe),
            ),
        ];
        for (label, mutate) in mutations {
            let mut altered = fake_receipt(&manifest, &identity, &binary);
            mutate(&mut altered);
            write_signed_receipt(root, &receipt_path, &serde_json::to_vec(&altered).unwrap());
            assert!(
                verified_executable(root, &manifest, &identity, None).is_err(),
                "{label} was accepted"
            );
        }

        // A receipt that predates the attestation field must not deserialize.
        let mut old: serde_json::Value = serde_json::from_slice(&original).unwrap();
        old.as_object_mut().unwrap().remove("source");
        let old_json = serde_json::to_vec(&old).unwrap();
        write_signed_receipt(root, &receipt_path, &old_json);
        let error = verified_executable(root, &manifest, &identity, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("rebuild from verified source"), "{error}");

        // A modified receipt must fail its signature check.
        write_signed_receipt(root, &receipt_path, &original);
        let mut tampered =
            crate::files::read_regular(&receipt_path, 2 * 1024 * 1024, false).unwrap();
        tampered[100] ^= 1;
        write(&receipt_path, &tampered).unwrap();
        assert!(verified_executable(root, &manifest, &identity, None).is_err());

        // The native executable hash remains a hard binding.
        write_signed_receipt(root, &receipt_path, &original);
        write(&path, b"changed executable").unwrap();
        let error = verified_executable(root, &manifest, &identity, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("native executable changed"), "{error}");
        write(&path, &binary).unwrap();
    }

    #[test]
    fn modified_installed_package_fails_before_native_launch() {
        let temp = tempfile::tempdir().unwrap();
        let root = &temp.path().join("library");
        let id = "com.gamepkg.package-tamper";
        let (_manifest, _identity, _path, _binary) = stage_fake_build(root, id);
        let package = root.join(format!("{id}.gamepkg"));
        let mut bytes = fs::read(&package).unwrap();
        bytes.push(0);
        write(&package, &bytes).unwrap();
        assert!(prepared(root, id, None).is_err());
    }

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
        crate::source::scan_probe::reset();
        crate::native::compile(root, &manifest.id).unwrap();
        assert!(
            crate::source::scan_probe::count() > 0,
            "admitted build did not scan guest source"
        );
        let settings = crate::settings::Settings {
            memory_ceiling_mib: 512,
            mute_audio: true,
            ..Default::default()
        };
        let play = || play_entry(root, &manifest.id, &[], &settings, Default::default(), true);
        crate::source::scan_probe::reset();
        assert!(play()
            .unwrap_err()
            .to_string()
            .contains("select the required game-data folder"));
        assert_eq!(crate::source::scan_probe::count(), 0);
        let installation = temp.path().join("installation");
        fs::create_dir_all(installation.join("packages")).unwrap();
        fs::create_dir(installation.join("data")).unwrap();
        fs::write(installation.join("packages/picture"), b"package").unwrap();
        fs::write(installation.join("data/start.cfg"), b"startup").unwrap();
        fs::write(installation.join("private"), b"outside").unwrap();
        crate::game_data::remember(root, &manifest, &installation).unwrap();
        crate::source::scan_probe::reset();
        play().unwrap();
        assert_eq!(crate::source::scan_probe::count(), 0);
        assert_eq!(
            fs::read(installation.join("data/start.cfg")).unwrap(),
            b"startup"
        );
    }
}
