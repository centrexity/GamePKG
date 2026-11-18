//! Trusted compatibility-layer link from the installed SDK or development checkout.
use super::*;
use crate::port_host::graphics::{PipelinePayload, VerifiedPipelines};
use crate::port_host::sdk_source::Source;

const RUNTIME: &str = include_str!("../../examples/maintainer/native_service_runtime.rs");
const PROFILE: &str = "native-service";

/// Link checked native objects into the fixed in-process runtime. The trusted
/// app snapshot and Cargo dependencies are GamePKG build inputs. Package
/// files never enter the Cargo workspace.
pub fn build_in_process(
    cache: &Path,
    sources: &CheckedSources,
    plan: &Plan,
    pipelines: Option<VerifiedPipelines>,
) -> Result<Artifact> {
    ensure!(
        plan.target == Target::Linux64,
        "in-process builder requires Linux x64"
    );
    ensure!(
        sources.manifest().required_rom_count() <= 1,
        "in-process bootstrap currently supports one inherited ROM descriptor"
    );
    let payload = match pipelines {
        Some(pipelines) => pipelines.into_payload(sources)?,
        None => PipelinePayload {
            encoded: b"[]".to_vec(),
            hashes: Vec::new(),
            samples: 1,
            preparation: Vec::new(),
        },
    };
    let compatibility = Source::open(cache)?;
    let checkout = compatibility.root();
    let tools = tools(cache, plan.target)?;
    let game = compile_and_audit(&tools, sources, plan, payload.hashes)?;
    let audited = &game.audited;
    let entry = checkout.join("examples/maintainer/native_service_runtime.rs");
    ensure!(
        fs::read(entry)? == RUNTIME.as_bytes(),
        "trusted runtime source changed"
    );
    ensure!(
        fs::read(checkout.join("Cargo.lock"))? == include_bytes!("../../Cargo.lock"),
        "trusted runtime lockfile changed"
    );

    let (windows, tasks, table) = runtime_bindings(plan);
    let task_limit = 32;
    let task_stack = 1024 * 1024;
    let process_memory = crate::port_host::local_runtime::Runtime::required_budget(
        plan.layout.windows(),
        task_limit,
        task_stack,
    )
    .map_err(|error| anyhow::anyhow!("native memory layout: {error:?}"))?
    .max(super::super::player::Options::default().process_memory);
    let manifest = toml::to_string(sources.manifest())?;
    let sdk = super::super::build_identity();
    let config = tempfile::tempdir()?;
    let payload_path = config.path().join("pipelines.json");
    fs::write(&payload_path, &payload.encoded)?;
    let payload_hash = crate::verify::digest(&payload.encoded);
    let shader_hashes = &audited.receipt().shaders;
    let samples = payload.samples;
    let source_kit = sources.identity();
    let generated = format!(
        r#"const MANIFEST: &str = {manifest:?};
const SDK: &str = {sdk:?};
const SOURCE_KIT: &str = {source_kit:?};
const PIPELINES: &[u8] = include_bytes!("pipelines.json");
const PIPELINE_PAYLOAD_HASH: &str = {payload_hash:?};
const SHADER_HASHES: &[&str] = &{shader_hashes:?};
const SAMPLES: u32 = {samples};
const PROCESS_MEMORY: u64 = {process_memory};
unsafe extern "C" {{fn game_service_entry();}}
{tasks}
fn local_runtime(budget: u64) -> local::runtime::Result<local::runtime::Runtime> {{
    local::runtime::Runtime::new({windows}, vec![{table}], budget, {task_limit}, {task_stack})
}}
"#
    );
    let config_path = config.path().join("config.rs");
    fs::write(&config_path, &generated)?;
    let output = tempfile::tempdir()?;
    let cargo = tools.root.join("bin/cargo").canonicalize()?;
    let cargo_hash = hash(&cargo)?;
    let mut command = Command::new(&cargo);
    command
        .current_dir(checkout)
        .args([
            "rustc",
            "--offline",
            "--locked",
            "--profile",
            PROFILE,
            "--features",
            "gui",
            "--example",
            "native-service-runtime",
            "--",
            "--cfg",
            "gamepkg_linked_runtime",
            "-C",
            "lto=off",
            "-C",
            "strip=debuginfo",
        ])
        .env("RUSTC", &tools.rustc)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CARGO_BUILD_TARGET")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_INCREMENTAL")
        .env("CARGO_PROFILE_NATIVE_SERVICE_OPT_LEVEL", "3")
        .env("CARGO_PROFILE_NATIVE_SERVICE_LTO", "false")
        .env("CARGO_PROFILE_NATIVE_SERVICE_DEBUG", "0")
        .env_remove("GAMEPKG_NATIVE_SDK_SHA256")
        .env_remove("GAMEPKG_NATIVE_SDK_PIN")
        .env_remove("GAMEPKG_BUNDLED_AUDIO_DECODER")
        .env_remove("GAMEPKG_BUNDLED_AUDIO_CONVERTER")
        .env("GAMEPKG_LOCAL_RUNTIME_CONFIG", &config_path)
        .arg("-C")
        .arg(format!("linker={}", tools.linker.display()))
        .arg(format!(
            "--emit=link={}",
            output.path().join(filename(plan.target)).display()
        ));
    if let Some(pin) = compatibility.pin() {
        let target = cache
            .join("native-runtime-builds")
            .join(pin)
            .join("compiled")
            .join(&tools.identity);
        fs::create_dir_all(&target)?;
        command
            .env("CARGO_HOME", compatibility.private_cargo_home().unwrap())
            .env("CARGO_TARGET_DIR", target.canonicalize()?);
    }
    let decoder_hash = if sources.manifest().native_tick
        && sources.manifest().limits.music_memory_bytes.is_some()
    {
        let decoder = crate::audio_decode::bundled_bytes()?;
        let decoder_path = config.path().join("recorded-audio.wasm");
        fs::write(&decoder_path, decoder)?;
        command.env("GAMEPKG_BUNDLED_AUDIO_DECODER", decoder_path);
        crate::verify::digest(decoder)
    } else {
        String::new()
    };
    for object in audited.objects()? {
        command
            .arg("-C")
            .arg(format!("link-arg={}", object.display()));
    }
    let compiled = crate::compiler::run_timed(&mut command, std::time::Duration::from_secs(600))?;
    ensure!(
        compiled.status.success(),
        "trusted compatibility link failed: {}",
        compiled.stderr
    );
    ensure!(
        cargo_hash == hash(&cargo)?,
        "trusted compatibility builder changed"
    );
    let source_identity = compatibility.pin().unwrap_or("development checkout");
    let runtime = crate::verify::digest(
        format!("{sdk}\0{RUNTIME}\0{source_identity}\0{decoder_hash}\0{generated}").as_bytes(),
    );
    let toolchain = crate::verify::digest(
        format!(
            "{}\0{cargo_hash}\0{PROFILE}\0opt-level=3\0lto=off\0strip=debuginfo",
            tools.identity
        )
        .as_bytes(),
    );
    finish(
        sources,
        plan,
        game,
        LinkedRuntime {
            directory: output,
            bootstrap: Bootstrap::InProcess,
            toolchain,
            identity: runtime,
            shader_preparation: payload.preparation,
        },
    )
}
