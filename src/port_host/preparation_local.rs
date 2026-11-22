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
    if plan.target.windows() {
        ensure!(
            sources.manifest().native_graphics.is_some() == pipelines.is_some(),
            "Windows native pipelines must match the signed graphics profile"
        );
    }
    let payload = match pipelines {
        Some(pipelines) => pipelines.into_payload(sources)?,
        None => empty_payload(),
    };
    link_runtime(cache, sources, plan, payload)
}

/// Legacy image-only helper. Indexed builds pass their verified pipeline to
/// [`build_in_process`].
#[cfg(windows)]
pub fn build_windows_in_process(
    cache: &Path,
    sources: &CheckedSources,
    plan: &Plan,
) -> Result<Artifact> {
    ensure!(
        plan.target.windows(),
        "Windows compatibility build requires a Windows target"
    );
    build_in_process(cache, sources, plan, None)
}

fn empty_payload() -> PipelinePayload {
    PipelinePayload {
        encoded: b"[]".to_vec(),
        hashes: Vec::new(),
        samples: 1,
        preparation: Vec::new(),
    }
}

fn link_runtime(
    cache: &Path,
    sources: &CheckedSources,
    plan: &Plan,
    payload: PipelinePayload,
) -> Result<Artifact> {
    ensure!(
        matches!(
            plan.target,
            Target::Linux64 | Target::Windows64 | Target::Windows32
        ),
        "in-process builder requires Linux x64 or Windows x86/x64"
    );
    if plan.target.windows() {
        super::super::process::require_windows_compatibility(sources.manifest())?;
    }
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
    #[cfg(target_os = "linux")]
    let cargo = tools.root.join("bin/cargo").canonicalize()?;
    #[cfg(windows)]
    let cargo = crate::takedawn_windows_sdk::prepare_compat_cargo(cache, plan.target.triple())?
        .canonicalize()?;
    let cargo_hash = hash(&cargo)?;
    let mut command = Command::new(link_path(&cargo)?);
    #[cfg(target_os = "linux")]
    let (c_identity, linker) = {
        command.env_remove("CARGO_ENCODED_RUSTFLAGS");
        ("", link_path(&tools.linker)?)
    };
    #[cfg(windows)]
    let (c_identity, linker) = configure_windows(
        &mut command,
        cache,
        plan.target,
        &tools,
        &compatibility,
        config.path(),
    )?;
    command.current_dir(link_path(checkout)?).args([
        "rustc",
        "--offline",
        "--locked",
        "--profile",
        PROFILE,
        "--features",
        "gui",
        "--example",
        "native-service-runtime",
    ]);
    // The standalone Windows compiler already runs as the selected target.
    // An explicit Cargo target would withhold the verified DLL tool flags
    // from host-side crates that build scripts and proc macros use.
    if cfg!(target_os = "linux") && plan.target.windows() {
        command.args(["--target", plan.target.triple()]);
    }
    command
        .args([
            "--",
            "--cfg",
            "gamepkg_linked_runtime",
            "-C",
            "lto=off",
            "-C",
            "strip=debuginfo",
        ])
        .env("RUSTC", link_path(&tools.rustc)?)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CARGO_BUILD_TARGET")
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
        .arg(format!("linker={}", linker.display()))
        .arg(format!(
            "--emit=link={}",
            link_path(&output.path().join(filename(plan.target)))?.display()
        ));
    #[cfg(windows)]
    command.args(["-C", "linker-flavor=ld", "-C", "link-self-contained=yes"]);
    if let Some(pin) = compatibility.pin() {
        let cargo_home = compatibility.private_cargo_home().unwrap();
        #[cfg(target_os = "linux")]
        let target = cache
            .join("native-runtime-builds")
            .join(pin)
            .join("compiled")
            .join(&tools.identity);
        #[cfg(windows)]
        let target = {
            let _ = pin;
            cargo_home.with_file_name("compiled")
        };
        fs::create_dir_all(&target)?;
        command
            .env("CARGO_HOME", cargo_home)
            .env("CARGO_TARGET_DIR", link_path(&target.canonicalize()?)?);
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
            .arg(format!("link-arg={}", link_path(&object)?.display()));
    }
    // A cold Windows SDK verifies the vendored crates and builds the entire
    // trusted runtime in a fresh directory. The audited game compilation has
    // its own deadline and has already finished.
    let runtime_build_limit =
        std::time::Duration::from_secs(if cfg!(windows) { 1800 } else { 600 });
    let compiled = crate::compiler::run_timed(&mut command, runtime_build_limit)?;
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
            "{}\0{cargo_hash}\0{c_identity}\0{PROFILE}\0opt-level=3\0lto=off\0strip=debuginfo",
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

fn link_path(path: &Path) -> Result<PathBuf> {
    #[cfg(windows)]
    return crate::takedawn_windows_sdk::compiler_path(path);
    #[cfg(target_os = "linux")]
    Ok(path.to_path_buf())
}

#[cfg(windows)]
fn configure_windows(
    command: &mut Command,
    cache: &Path,
    target: Target,
    tools: &Tools,
    source: &Source,
    private: &Path,
) -> Result<(String, PathBuf)> {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    let (arch, clang_name) = match target {
        Target::Windows64 => ("x86_64", "x86_64-w64-mingw32"),
        Target::Windows32 => ("i686", "i686-w64-mingw32"),
        _ => anyhow::bail!("invalid Windows compatibility target"),
    };
    let (bin, identity) = crate::takedawn_windows_sdk::prepare_compat_clang(cache, arch)?;
    let clang = bin.join(format!("{clang_name}-clang.exe")).canonicalize()?;
    let ar = bin.join("llvm-ar.exe").canonicalize()?;
    let dlltool = bin.join("llvm-dlltool.exe").canonicalize()?;
    // Keep LLVM's import archives and the final PE linker in the same toolchain.
    // GNU ld linked the image but omitted some raw-dylib import descriptors.
    // Even a verbatim executable name past MAX_PATH can fail inside LLD's own
    // startup. Copy the already-verified linker into the private build folder
    // and retain the pinned bin directory in PATH for its runtime libraries.
    let linker_source = bin.join("ld.lld.exe").canonicalize()?;
    let linker = private.join("ld.lld.exe");
    let linker_hash = hash(&linker_source)?;
    fs::copy(&linker_source, &linker)?;
    ensure!(
        linker_hash == hash(&linker)?,
        "staged LLD executable changed"
    );
    // Desktop crates need import libraries beyond those shipped with Rust.
    let imports = bin
        .parent()
        .context("missing LLVM-MinGW installation directory")?
        .join(clang_name)
        .join("lib")
        .canonicalize()?;
    // Keep Win32 device prefixes out of the paths passed between Cargo,
    // rustc and MinGW. The tool locations are still canonical and verified.
    let temp = link_path(&std::env::temp_dir())?;
    command.env_clear().creation_flags(CREATE_NO_WINDOW).env(
        "SystemRoot",
        std::env::var_os("SystemRoot").context("Windows system directory unavailable")?,
    );
    command
        .env("TEMP", &temp)
        .env("TMP", temp)
        .env(
            "PATH",
            std::env::join_paths([
                link_path(&tools.root.join("bin"))?,
                link_path(
                    tools
                        .linker
                        .parent()
                        .context("missing compiler linker directory")?,
                )?,
                link_path(&bin)?,
            ])?,
        )
        .env("CC", link_path(&clang)?)
        .env("AR", link_path(&ar)?)
        // Rust's standalone GNU dlltool requires a separate GNU assembler.
        // LLVM's pinned tool produces the same import libraries directly.
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            format!(
                // Append the SDK search path after Rust's own sysroot so its
                // MinGW startup objects still use the matching CRT libraries.
                "-C\u{1f}linker-flavor=ld\u{1f}-C\u{1f}link-self-contained=yes\u{1f}-C\u{1f}dlltool={}\u{1f}-C\u{1f}link-arg=-L{}",
                link_path(&dlltool)?.display(),
                link_path(&imports)?.display(),
            ),
        )
        .env(
            format!(
                "CARGO_TARGET_{}_LINKER",
                target.triple().replace('-', "_").to_ascii_uppercase()
            ),
            // Use LLD for the host build scripts and proc macros too. GCC
            // also needs to rediscover CRT libraries relative to its own
            // image, which fails with an extended-length executable path.
            &linker,
        );
    if source.pin().is_none() {
        // Development checkouts can use their existing offline registry and
        // target directory. Installed builds get fresh paths below.
        for name in ["CARGO_HOME", "USERPROFILE", "CARGO_TARGET_DIR"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
    }
    Ok((identity, linker))
}
