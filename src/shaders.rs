//! Pinned Slang compilation inside platform-specific confined build workers.
use crate::{
    archive::Package,
    manifest::{Manifest, Shader},
    settings::Settings,
    verify::digest,
};
use anyhow::{bail, Context, Result};
use std::{
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
pub const SLANG_VERSION: &str = "2026.19";
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheReceipt {
    format_version: u32,
    package_digest: String,
    backend: String,
    cache_digest: String,
    files: std::collections::BTreeMap<String, String>,
}
fn receipt_path(root: &Path, archive: &[u8], backend: &str) -> PathBuf {
    root.join("builds")
        .join(digest(archive))
        .join(format!("shaders-{backend}.json"))
}
fn publish_receipt(
    root: &Path,
    archive: &[u8],
    settings: &Settings,
    receipt: &CacheReceipt,
) -> Result<()> {
    let path = receipt_path(root, archive, &settings.shader_backend);
    fs::create_dir_all(path.parent().unwrap())?;
    let mut output = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    output.write_all(&serde_json::to_vec(receipt)?)?;
    output.as_file().sync_all()?;
    output.persist(path)?;
    Ok(())
}
pub(crate) fn playback_cache(
    root: &Path,
    archive: &[u8],
    settings: &Settings,
    count: usize,
) -> Result<(PathBuf, std::collections::BTreeMap<String, String>)> {
    settings.validate()?;
    let path = receipt_path(root, archive, &settings.shader_backend);
    if !path.try_exists()? {
        // Older builds require the installed compiler to locate their original cache.
        return Ok((
            compiler_cache(root, archive, settings)?.1,
            Default::default(),
        ));
    }
    let receipt: CacheReceipt =
        serde_json::from_slice(&crate::files::read_regular(&path, 64 * 1024, false)?)?;
    if receipt.format_version != 1
        || receipt.package_digest != digest(archive)
        || receipt.backend != settings.shader_backend
        || receipt.files.len() != count * 2
        || receipt.cache_digest.len() != 64
        || !receipt.cache_digest.bytes().all(|c| c.is_ascii_hexdigit())
    {
        bail!("shader cache receipt does not match the signed package and backend");
    }
    let cache = root.join("shader-builds").join(&receipt.cache_digest);
    Ok((cache, receipt.files))
}

pub fn compile(
    root: &Path,
    archive: &[u8],
    package: &mut Package,
    manifest: &Manifest,
    settings: &Settings,
) -> Result<PathBuf> {
    if !cfg!(any(target_os = "linux", windows)) {
        bail!("isolated shader builds require Linux or Windows");
    }
    settings.validate()?;
    let (compiler, cache) = compiler_cache(root, archive, settings)?;
    let dxc = dxc_root(root, settings)?;
    let defaults = vec![Shader {
        source: "shaders/main.slang".into(),
        entry: "fragmentMain".into(),
        stage: "fragment".into(),
    }];
    let entries = if manifest.shaders.is_empty() {
        &defaults
    } else {
        &manifest.shaders
    };
    let stage = tempfile::tempdir()?;
    for name in package
        .entries
        .keys()
        .filter(|name| name.starts_with("shaders/"))
        .cloned()
        .collect::<Vec<_>>()
    {
        let path = stage.path().join(&name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, package.read(&name)?)?;
    }
    let mut artifacts = Vec::new();
    for (index, shader) in entries.iter().enumerate() {
        shader.validate()?;
        if !package.entries.contains_key(&shader.source) {
            bail!("shader source is missing: {}", shader.source);
        }
        let binary = format!("{index}.shader");
        let reflection = format!("{index}.reflection.json");
        let _ = run_shader(
            &compiler,
            dxc.as_deref(),
            stage.path(),
            &shader_args(shader, settings, &binary, &reflection),
        )?;
        let bytes = read_output(&stage.path().join(&binary))?;
        if bytes.is_empty()
            || (settings.shader_backend == "spirv" && !bytes.starts_with(&[3, 2, 35, 7]))
            || (settings.shader_backend == "dxil" && !bytes.starts_with(b"DXBC"))
        {
            bail!("invalid shader compiler output");
        }
        let metadata = read_output(&stage.path().join(&reflection))?;
        let reflected: serde_json::Value =
            serde_json::from_slice(&metadata).context("invalid shader reflection")?;
        if shader.stage == "fragment"
            && matches!(settings.shader_backend.as_str(), "glsl" | "spirv" | "wgsl")
        {
            validate_fragment_reflection(&reflected, &shader.entry)?;
            if settings.shader_backend != "glsl" {
                fragment_source(&bytes, &settings.shader_backend)?;
            }
        }
        artifacts.push((binary, bytes));
        artifacts.push((reflection, metadata));
    }
    // Cache publication follows complete compilation; a failed shader cannot publish partial output.
    fs::create_dir_all(root.join("shader-builds"))?;
    let destination = tempfile::tempdir_in(root.join("shader-builds"))?;
    let receipt = CacheReceipt {
        format_version: 1,
        package_digest: digest(archive),
        backend: settings.shader_backend.clone(),
        cache_digest: cache.file_name().unwrap().to_str().unwrap().to_owned(),
        files: artifacts
            .iter()
            .map(|(name, bytes)| (name.clone(), digest(bytes)))
            .collect(),
    };
    let mut cache_matches = cache.is_dir();
    for (name, bytes) in artifacts {
        cache_matches &= crate::files::read_regular(&cache.join(&name), 16 * 1024 * 1024, false)
            .is_ok_and(|previous| previous == bytes);
        let mut file = fs::File::create(destination.path().join(name))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    fs::write(
        destination.path().join("manifest.toml"),
        toml::to_string(manifest)?,
    )?;
    if cache.exists() {
        if !cache_matches {
            bail!(
                "shader cache is damaged; remove {} and rebuild",
                cache.display()
            );
        }
        publish_receipt(root, archive, settings, &receipt)?;
        return Ok(cache);
    }
    fs::rename(destination.path(), &cache)?;
    publish_receipt(root, archive, settings, &receipt)?;
    Ok(cache)
}
fn shader_args(
    shader: &Shader,
    settings: &Settings,
    binary: &str,
    reflection: &str,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        shader.source.as_str(),
        "-entry",
        &shader.entry,
        "-stage",
        &shader.stage,
        "-target",
        &settings.shader_backend,
        "-o",
        binary,
        "-reflection-json",
        reflection,
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    match settings.shader_backend.as_str() {
        "spirv" => {
            args.extend(["-profile", "glsl_450", "-DGAMEPKG_SEPARATE_SAMPLER=1"].map(Into::into))
        }
        "glsl" => args.extend(["-profile", "glsl_330"].map(Into::into)),
        "dxil" => args.extend(["-profile", "sm_6_0"].map(Into::into)),
        _ => (),
    }
    args
}

#[cfg(target_os = "linux")]
fn run_shader(
    compiler: &Path,
    dxc: Option<&Path>,
    stage: &Path,
    args: &[OsString],
) -> Result<Vec<u8>> {
    let mut command = crate::compiler::command(16 * 1024 * 1024);
    command.args([
        "bwrap",
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--clearenv",
        "--ro-bind",
        "/usr",
        "/usr",
    ]);
    for path in ["/lib", "/lib64"] {
        if Path::new(path).exists() {
            command.args(["--ro-bind", path, path]);
        }
    }
    if let Some(dxc) = dxc {
        command.arg("--ro-bind").arg(dxc.join("lib")).arg("/dxc");
    }
    command
        .arg("--ro-bind")
        .arg(compiler)
        .arg("/compiler")
        .args([
            "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp", "--bind",
        ])
        .arg(stage)
        .arg("/work")
        .args([
            "--chdir",
            "/work",
            "--setenv",
            "LD_LIBRARY_PATH",
            if dxc.is_some() {
                "/dxc:/compiler/lib"
            } else {
                "/compiler/lib"
            },
            "--",
            "/compiler/bin/slangc",
        ])
        .args(args);
    let output = crate::compiler::run(&mut command)?;
    if !output.status.success() {
        bail!("isolated shader build failed: {}", output.stderr);
    }
    Ok([output.stdout.as_bytes(), output.stderr.as_bytes()].concat())
}

#[cfg(windows)]
fn run_shader(
    compiler: &Path,
    dxc: Option<&Path>,
    stage: &Path,
    args: &[OsString],
) -> Result<Vec<u8>> {
    use gamepkg_takedawn_windows_runner::Sandbox;
    use std::time::{Duration, Instant};
    let stage = stage.canonicalize()?;
    let compiler = compiler.canonicalize()?;
    let identity = digest(stage.as_os_str().as_encoded_bytes());
    let sandbox = Sandbox::open(&format!("GamePKG.Shader.{}", &identity[..32]))?;
    sandbox.grant_runtime(&compiler)?;
    let mut args = args.to_vec();
    let mut paths = vec![crate::takedawn_windows_sdk::compiler_path(
        &compiler.join("bin"),
    )?];
    if let Some(dxc) = dxc {
        let dxc = dxc.canonicalize()?;
        sandbox.grant_runtime(&dxc)?;
        let bin = crate::takedawn_windows_sdk::compiler_path(&dxc.join("bin/x64"))?;
        args.push("-dxc-path".into());
        // Slang expects the directory, not the DLL filename.
        args.push(bin.as_os_str().to_owned());
        paths.push(bin);
    }
    sandbox.grant_saves(&stage)?;
    let directory = crate::takedawn_windows_sdk::compiler_path(&stage)?;
    let temporary = directory.join("tmp");
    fs::create_dir_all(&temporary)?;
    let values = [
        (
            "SystemRoot".into(),
            std::env::var_os("SystemRoot").context("Windows system directory unavailable")?,
        ),
        (
            "LOCALAPPDATA".into(),
            std::env::var_os("LOCALAPPDATA").context("Windows profile directory unavailable")?,
        ),
        ("PATH".into(), std::env::join_paths(paths)?),
        ("TEMP".into(), temporary.as_os_str().to_owned()),
        ("TMP".into(), temporary.into_os_string()),
    ];
    crate::windows_compiler::run(
        &compiler.join("bin/slangc.exe"),
        &args,
        &directory,
        &values,
        &sandbox,
        Instant::now() + Duration::from_secs(60),
    )
}

#[cfg(not(any(target_os = "linux", windows)))]
fn run_shader(_: &Path, _: Option<&Path>, _: &Path, _: &[OsString]) -> Result<Vec<u8>> {
    bail!("isolated shader builds require Linux or Windows")
}

fn read_output(path: &Path) -> Result<Vec<u8>> {
    crate::files::read_regular(path, 16 * 1024 * 1024, false)
        .context("shader output must be a regular file of at most 16 MiB")
}

/// Conservative source policy, not a replacement for compiler isolation or GPU validation.
/// Inspect comments and strings too so preprocessing cannot hide forbidden constructs.
pub fn validate_source(bytes: &[u8]) -> Result<()> {
    let source = std::str::from_utf8(bytes).context("Slang sources must be UTF-8")?;
    let source = source.replace("\\\r\n", "").replace("\\\n", "");
    // Token pasting can manufacture a forbidden identifier after this scan.
    // Reject every pair of hash tokens, including pairs separated by comments.
    let bytes = source.as_bytes();
    let mut previous = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"/*") {
            i += 2;
            while i < bytes.len() && !bytes[i..].starts_with(b"*/") {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if !bytes[i].is_ascii_whitespace() {
            if previous == b'#' && bytes[i] == b'#' {
                bail!("shader macro token pasting is not allowed");
            }
            previous = bytes[i];
        }
        i += 1;
    }
    for token in source.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if matches!(
            token,
            "include"
                | "include_next"
                | "import"
                | "__include"
                | "spirv_asm"
                | "__intrinsic_asm"
                | "__require_prelude"
                | "__requirePrelude"
                | "__target_intrinsic"
                | "__spirv_instruction"
        ) {
            bail!("shader file inclusion or target-code injection is not allowed: {token}");
        }
    }
    Ok(())
}

fn dxc_root(_root: &Path, settings: &Settings) -> Result<Option<PathBuf>> {
    if settings.shader_backend != "dxil" {
        return Ok(None);
    }
    #[cfg(windows)]
    {
        let selected = std::env::var_os("GAMEPKG_DXC_ROOT")
            .map(PathBuf::from)
            .or_else(|| {
                (!settings.dxc_directory.is_empty()).then(|| PathBuf::from(&settings.dxc_directory))
            });
        Ok(Some(crate::takedawn_windows_sdk::prepare_dxc(
            _root,
            selected.as_deref(),
        )?))
    }
    #[cfg(not(windows))]
    {
        let root = std::env::var_os("GAMEPKG_DXC_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            (!settings.dxc_directory.is_empty()).then(|| PathBuf::from(&settings.dxc_directory))
        })
        .context(
            "DXIL requires pinned DXC 1.9.2609; run tools/install_dxc.py and set GAMEPKG_DXC_ROOT",
        )?;
        let root = fs::canonicalize(root)?;
        for (name, expected) in [
            (
                "libdxcompiler.so",
                "d40e4a7b3b46490313f4bfd08101946483a673831b3ea65c93d746de1c2f9216",
            ),
            (
                "libdxil.so",
                "730b31420ea0c38f711c1eb581613bfd0797c53371a55f721838b29d10881c8c",
            ),
        ] {
            let mut bytes = Vec::new();
            use std::io::Read;
            fs::File::open(root.join("lib").join(name))?
                .take(64 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 64 * 1024 * 1024 {
                bail!("DXC library exceeds size limit");
            }
            if digest(&bytes) != expected {
                bail!("DXC library checksum mismatch: {name}");
            }
        }
        Ok(Some(root))
    }
}

fn compiler_cache(root: &Path, archive: &[u8], settings: &Settings) -> Result<(PathBuf, PathBuf)> {
    let selected = std::env::var_os("GAMEPKG_SLANG_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            (!settings.slang_directory.is_empty()).then(|| PathBuf::from(&settings.slang_directory))
        });
    #[cfg(windows)]
    let compiler = crate::takedawn_windows_sdk::prepare_slang(root, selected.as_deref())?;
    #[cfg(not(windows))]
    let compiler =
        fs::canonicalize(selected.context(
            "install Slang 2026.19 and set its folder in Settings or GAMEPKG_SLANG_ROOT",
        )?)?;
    #[cfg(windows)]
    let (exe, library) = (
        compiler.join("bin/slangc.exe"),
        compiler.join("bin/slang-compiler.dll"),
    );
    #[cfg(not(windows))]
    let (exe, library) = (
        compiler.join("bin/slangc"),
        compiler.join("lib/libslang-compiler.so"),
    );
    #[cfg(windows)]
    let (version_ok, version_text) = {
        let stage = tempfile::tempdir()?;
        let output = run_shader(&compiler, None, stage.path(), &["-version".into()])?;
        (true, String::from_utf8_lossy(&output).into_owned())
    };
    #[cfg(not(windows))]
    let (version_ok, version_text) = {
        let version = crate::compiler::run(
            crate::compiler::command(16 * 1024 * 1024)
                .arg(&exe)
                .arg("-version"),
        )
        .context("unable to run Slang compiler")?;
        (
            version.status.success(),
            format!("{}{}", version.stdout, version.stderr),
        )
    };
    if !version_ok || version_text.trim() != SLANG_VERSION {
        bail!("Slang version must be exactly {SLANG_VERSION}");
    }
    let dxc_identity = if dxc_root(root, settings)?.is_some() {
        ":dxc-1.9.2609"
    } else {
        ""
    };
    let sampler_identity = if settings.shader_backend == "spirv" {
        ":separate-sampler-v1"
    } else {
        ""
    };
    let identity = digest(
        format!(
            "{}:{SLANG_VERSION}:{}:{}:{}{dxc_identity}{sampler_identity}",
            digest(archive),
            settings.shader_backend,
            digest(&fs::read(&exe)?),
            digest(&fs::read(library)?)
        )
        .as_bytes(),
    );
    Ok((compiler, root.join("shader-builds").join(identity)))
}

pub fn load_desktop_sources(
    root: &Path,
    archive: &[u8],
    manifest: &Manifest,
    settings: &Settings,
) -> Result<Vec<String>> {
    if settings.renderer == "opengl" {
        return load_fragment_sources(root, archive, manifest, settings);
    }
    if !matches!(settings.shader_backend.as_str(), "wgsl" | "spirv") {
        bail!("Vulkan shaders need WGSL or SPIR-V. Select WGSL in Settings and rebuild this game.");
    }
    // Reuse all checksum, reflection and resource validation before creating a GPU module.
    load_fragment_sources(root, archive, manifest, settings)?;
    let (cache, _) = playback_cache(root, archive, settings, manifest.shaders.len().max(1))?;
    (0..manifest.shaders.len().max(1))
        .map(|index| {
            let bytes = read_output(&cache.join(format!("{index}.shader")))?;
            let module = if settings.shader_backend == "wgsl" {
                naga::front::wgsl::parse_str(std::str::from_utf8(&bytes)?)?
            } else {
                naga::front::spv::parse_u8_slice(
                    &bytes,
                    &naga::front::spv::Options {
                        strict_capabilities: true,
                        ..Default::default()
                    },
                )?
            };
            let info = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)?;
            Ok(naga::back::wgsl::write_string(
                &module,
                &info,
                naga::back::wgsl::WriterFlags::empty(),
            )?)
        })
        .collect()
}

pub fn load_fragment_sources(
    root: &Path,
    archive: &[u8],
    manifest: &Manifest,
    settings: &Settings,
) -> Result<Vec<String>> {
    if !matches!(settings.shader_backend.as_str(), "glsl" | "spirv" | "wgsl") {
        bail!("this desktop shader renderer requires GLSL, SPIR-V, or WGSL; select one in Settings and rebuild");
    }
    let (cache, expected) = playback_cache(root, archive, settings, manifest.shaders.len().max(1))?;
    let stages: Vec<&str> = if manifest.shaders.is_empty() {
        vec!["fragment"]
    } else {
        manifest.shaders.iter().map(|s| s.stage.as_str()).collect()
    };
    let mut sources = Vec::new();
    for (index, stage) in stages.iter().enumerate() {
        if *stage != "fragment" {
            bail!("desktop shader passes currently require fragment entries");
        }
        let reflection_name = format!("{index}.reflection.json");
        let reflection = read_output(&cache.join(&reflection_name))?;
        if !expected.is_empty() && expected.get(&reflection_name) != Some(&digest(&reflection)) {
            bail!("shader cache checksum mismatch; rebuild verified source");
        }
        let metadata: serde_json::Value = serde_json::from_slice(&reflection)?;
        let entry = manifest
            .shaders
            .get(index)
            .map_or("fragmentMain", |shader| shader.entry.as_str());
        validate_fragment_reflection(&metadata, entry)?;
        let binary_name = format!("{index}.shader");
        let bytes = read_output(&cache.join(&binary_name))?;
        if !expected.is_empty() && expected.get(&binary_name) != Some(&digest(&bytes)) {
            bail!("shader cache checksum mismatch; rebuild verified source");
        }
        sources.push(if settings.shader_backend != "glsl" {
            fragment_source(&bytes, &settings.shader_backend)?
        } else {
            String::from_utf8(bytes)?
        });
    }
    Ok(sources)
}

fn validate_fragment_reflection(metadata: &serde_json::Value, entry: &str) -> Result<()> {
    let parameters = metadata
        .get("parameters")
        .and_then(|value| value.as_array())
        .context("shader reflection must declare its resource parameters")?;
    let texture = parameters.len() == 1
        && parameters[0]["name"] == "gamepkgTexture"
        && parameters[0]["binding"]["kind"] == "descriptorTableSlot"
        && parameters[0]["binding"]["index"] == 0
        && parameters[0]["type"]["kind"] == "resource"
        && parameters[0]["type"]["baseShape"] == "texture2D"
        && parameters[0]["type"]["combined"] == true
        && parameters[0]["type"]["resultType"]
            == serde_json::json!({"kind":"vector","elementCount":4,"elementType":{"kind":"scalar","scalarType":"float32"}})
        && ((parameters[0]["type"]["sizes"]
            == serde_json::json!([{"kind":"descriptorTableSlot","value":1}])
            && (parameters[0]["binding"].get("count").is_none()
                || parameters[0]["binding"]["count"] == 1))
            || (parameters[0]["type"]["sizes"]
                == serde_json::json!([{"kind":"descriptorTableSlot","value":2}])
                && parameters[0]["binding"]["count"] == 2));
    let separate = parameters.len() == 2
        && parameters[0]["name"] == "gamepkgTexture"
        && parameters[0]["binding"]["kind"] == "descriptorTableSlot"
        && parameters[0]["binding"]["index"] == 0
        && parameters[0]["type"]["kind"] == "resource"
        && parameters[0]["type"]["baseShape"] == "texture2D"
        && parameters[0]["type"].get("combined").is_none()
        && parameters[0]["type"]["resultType"]
            == serde_json::json!({"kind":"vector","elementCount":4,"elementType":{"kind":"scalar","scalarType":"float32"}})
        && parameters[0]["type"]["sizes"]
            == serde_json::json!([{"kind":"descriptorTableSlot","value":1}])
        && parameters[1]["name"] == "gamepkgSampler"
        && parameters[1]["binding"]["kind"] == "descriptorTableSlot"
        && parameters[1]["binding"]["index"] == 1
        && parameters[1]["type"]["kind"] == "samplerState"
        && parameters[1]["type"]["sizes"]
            == serde_json::json!([{"kind":"descriptorTableSlot","value":1}]);
    if !parameters.is_empty() && !texture && !separate {
        bail!("desktop fragment passes reject unsupported resource bindings; only gamepkgTexture is allowed");
    }
    let entries = metadata
        .get("entryPoints")
        .and_then(|value| value.as_array())
        .context("shader reflection must declare its entry points")?;
    if entries.len() != 1
        || entries[0].get("name").and_then(|value| value.as_str()) != Some(entry)
        || entries[0].get("stage").and_then(|value| value.as_str()) != Some("fragment")
    {
        bail!("shader reflection entry point or stage does not match the signed manifest");
    }
    Ok(())
}

struct ShaderText(String);
impl std::fmt::Write for ShaderText {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if self.0.len().saturating_add(text.len()) > 16 * 1024 * 1024 {
            return Err(std::fmt::Error);
        }
        self.0.push_str(text);
        Ok(())
    }
}
fn fragment_source(bytes: &[u8], backend: &str) -> Result<String> {
    if bytes.len() > 16 * 1024 * 1024 {
        bail!("shader exceeds size limit");
    }
    let module = match backend {
        "spirv" => naga::front::spv::parse_u8_slice(
            bytes,
            &naga::front::spv::Options {
                strict_capabilities: true,
                ..Default::default()
            },
        )
        .context("invalid desktop SPIR-V shader")?,
        "wgsl" => naga::front::wgsl::parse_str(std::str::from_utf8(bytes)?)
            .context("invalid desktop WGSL shader")?,
        _ => bail!("unsupported shader translation backend"),
    };
    if module.entry_points.len() != 1 || module.entry_points[0].stage != naga::ShaderStage::Fragment
    {
        bail!("desktop translated shader requires one fragment entry");
    }
    let mut texture = None;
    let mut sampler = None;
    for (handle, variable) in module.global_variables.iter() {
        if variable.binding.is_none()
            && !matches!(
                variable.space,
                naga::AddressSpace::Handle
                    | naga::AddressSpace::Uniform
                    | naga::AddressSpace::Storage { .. }
                    | naga::AddressSpace::PushConstant
            )
        {
            continue;
        }
        let Some(binding) = &variable.binding else {
            bail!("unbound shader resource");
        };
        if binding.group != 0 || variable.space != naga::AddressSpace::Handle {
            bail!("unsupported shader resource bindings");
        }
        match module.types[variable.ty].inner {
            naga::TypeInner::Image {
                dim: naga::ImageDimension::D2,
                arrayed: false,
                class:
                    naga::ImageClass::Sampled {
                        kind: naga::ScalarKind::Float,
                        multi: false,
                    },
            } if binding.binding == 0 && texture.is_none() => texture = Some(handle),
            naga::TypeInner::Sampler { comparison: false }
                if binding.binding == 1 && sampler.is_none() =>
            {
                sampler = Some(handle)
            }
            _ => bail!(
                "unsupported shader resource bindings: {:?}",
                variable.binding
            ),
        }
    }
    if texture.is_some() != sampler.is_some() {
        bail!("shader texture requires its single sampler");
    }
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .context("shader validation failed")?;
    let options = naga::back::glsl::Options {
        version: naga::back::glsl::Version::Desktop(330),
        ..Default::default()
    };
    let pipeline = naga::back::glsl::PipelineOptions {
        shader_stage: naga::ShaderStage::Fragment,
        entry_point: module.entry_points[0].name.clone(),
        multiview: None,
    };
    let mut source = ShaderText(String::new());
    let reflected = naga::back::glsl::Writer::new(
        &mut source,
        &module,
        &info,
        &options,
        &pipeline,
        Default::default(),
    )?
    .write()?;
    if !reflected.uniforms.is_empty() || !reflected.push_constant_items.is_empty() {
        bail!("shader translation introduced uniform bindings");
    }
    if let (Some(texture), Some(sampler)) = (texture, sampler) {
        if reflected.texture_mapping.len() != 1 {
            bail!("shader requires one translated texture");
        }
        let (name, mapping) = reflected.texture_mapping.iter().next().unwrap();
        if mapping.texture != texture
            || mapping.sampler != Some(sampler)
            || name != "_group_0_binding_0_fs"
        {
            bail!("shader texture mapping mismatch");
        }
        // The translator owns this identifier; replacement is shorter, so the output budget remains intact.
        Ok(rename_texture(&source.0, name))
    } else {
        if !reflected.texture_mapping.is_empty() {
            bail!("shader translation introduced texture bindings");
        }
        Ok(source.0)
    }
}

fn rename_texture(source: &str, name: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut previous = 0;
    for (start, _) in source.match_indices(name) {
        let end = start + name.len();
        let identifier = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
        if (start > 0 && identifier(source.as_bytes()[start - 1]))
            || (end < source.len() && identifier(source.as_bytes()[end]))
        {
            continue;
        }
        output.push_str(&source[previous..start]);
        output.push_str("gamepkgTexture_0");
        previous = end;
    }
    output.push_str(&source[previous..]);
    output
}

#[cfg(test)]
mod translation_tests {
    #[test]
    fn texture_identifier_renaming_preserves_other_identifiers() {
        let name = "_group_0_binding_0_fs";
        assert_eq!(
            super::rename_texture(
                "_group_0_binding_0_fs + _group_0_binding_0_fs_extra + x_group_0_binding_0_fs",
                name
            ),
            "gamepkgTexture_0 + _group_0_binding_0_fs_extra + x_group_0_binding_0_fs"
        );
    }
    #[test]
    fn translated_text_stops_at_the_output_budget() {
        use std::fmt::Write;
        let mut output = super::ShaderText("x".repeat(16 * 1024 * 1024));
        assert!(output.write_str("").is_ok());
        assert!(output.write_str("x").is_err());
        assert_eq!(output.0.len(), 16 * 1024 * 1024);
        assert!(super::fragment_source(b"invalid", "spirv").is_err());
        assert!(super::fragment_source(b"invalid", "wgsl").is_err());
    }
}

#[cfg(test)]
mod wgsl_tests {
    #[test]
    fn translated_texture_rejects_extra_slots_storage_depth_and_array_images() {
        let prefix="@group(0) @binding(0) var image: texture_2d<f32>; @group(0) @binding(1) var filtering: sampler; ";
        let suffix="@fragment fn main() -> @location(0) vec4<f32> { return textureSample(image, filtering, vec2<f32>(0.5)); }";
        let source = format!("{prefix}{suffix}");
        let output = super::fragment_source(source.as_bytes(), "wgsl").unwrap();
        assert!(output.contains("uniform sampler2D gamepkgTexture_0"));
        for invalid in [source.replace("@binding(1)","@binding(2)"),source.replace("@group(0)","@group(1)"),source.replace("texture_2d<f32>","texture_2d_array<f32>"),source.replace("texture_2d<f32>","texture_depth_2d"), format!("{prefix}@group(0) @binding(2) var<uniform> extra: vec4<f32>; {suffix}"),format!("{prefix}@group(0) @binding(2) var storage: texture_storage_2d<rgba8unorm,write>; {suffix}")] {
            assert!(super::fragment_source(invalid.as_bytes(),"wgsl").is_err());
        }
    }
    #[test]
    fn desktop_translation_rejects_bindings_and_other_stages() {
        for source in [
            "@group(0) @binding(0) var<uniform> value: vec4<f32>; @fragment fn main() -> @location(0) vec4<f32> { return value; }",
            "@compute @workgroup_size(1) fn main() {}",
        ] {
            assert!(super::fragment_source(source.as_bytes(), "wgsl").is_err());
        }
        let valid = "@fragment fn main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }";
        let output = super::fragment_source(valid.as_bytes(), "wgsl").unwrap();
        assert!(output.contains("#version 330 core"));
    }
}

#[cfg(test)]
mod reflection_tests {
    #[test]
    fn texture_reflection_allows_only_one_combined_float_sampler_at_slot_zero() {
        let valid = serde_json::json!({"parameters":[{"name":"gamepkgTexture","binding":{"kind":"descriptorTableSlot","index":0},"type":{"kind":"resource","baseShape":"texture2D","combined":true,"resultType":{"kind":"vector","elementCount":4,"elementType":{"kind":"scalar","scalarType":"float32"}},"sizes":[{"kind":"descriptorTableSlot","value":1}]}}],"entryPoints":[{"name":"main","stage":"fragment"}]});
        super::validate_fragment_reflection(&valid, "main").unwrap();
        for (pointer, value) in [
            ("/parameters/0/name", serde_json::json!("other")),
            ("/parameters/0/binding/index", serde_json::json!(1)),
            ("/parameters/0/type/combined", serde_json::json!(false)),
            (
                "/parameters/0/type/baseShape",
                serde_json::json!("texture3D"),
            ),
            ("/parameters/0/type/kind", serde_json::json!("array")),
            (
                "/parameters/0/type/resultType/elementType/scalarType",
                serde_json::json!("uint32"),
            ),
            ("/parameters/0/type/sizes/0/value", serde_json::json!(2)),
        ] {
            let mut invalid = valid.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(
                super::validate_fragment_reflection(&invalid, "main").is_err(),
                "{pointer}"
            );
        }
        let mut duplicate = valid.clone();
        duplicate["parameters"]
            .as_array_mut()
            .unwrap()
            .push(valid["parameters"][0].clone());
        assert!(super::validate_fragment_reflection(&duplicate, "main").is_err());
    }
    #[test]
    fn split_sampler_reflection_requires_the_exact_texture_and_sampler_pair() {
        let mut valid = serde_json::json!({"parameters":[{"name":"gamepkgTexture","binding":{"kind":"descriptorTableSlot","index":0},"type":{"kind":"resource","baseShape":"texture2D","resultType":{"kind":"vector","elementCount":4,"elementType":{"kind":"scalar","scalarType":"float32"}},"sizes":[{"kind":"descriptorTableSlot","value":1}]}},{"name":"gamepkgSampler","binding":{"kind":"descriptorTableSlot","index":1},"type":{"kind":"samplerState","sizes":[{"kind":"descriptorTableSlot","value":1}]}}],"entryPoints":[{"name":"main","stage":"fragment"}]});
        super::validate_fragment_reflection(&valid, "main").unwrap();
        for (pointer, value) in [
            ("/parameters/1/name", serde_json::json!("other")),
            ("/parameters/1/binding/index", serde_json::json!(2)),
            (
                "/parameters/1/type/kind",
                serde_json::json!("samplerComparisonState"),
            ),
            (
                "/parameters/0/type/baseShape",
                serde_json::json!("texture3D"),
            ),
            ("/parameters/1/type/sizes/0/value", serde_json::json!(2)),
        ] {
            let mut invalid = valid.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(
                super::validate_fragment_reflection(&invalid, "main").is_err(),
                "{pointer}"
            );
        }
        valid["parameters"].as_array_mut().unwrap().pop();
        valid["parameters"][0]["type"]["combined"] = serde_json::json!(true);
        valid["parameters"][0]["type"]["sizes"][0]["value"] = serde_json::json!(2);
        valid["parameters"][0]["binding"]["count"] = serde_json::json!(2);
        super::validate_fragment_reflection(&valid, "main").unwrap();
        valid["parameters"][0]["binding"]["count"] = serde_json::json!(3);
        assert!(super::validate_fragment_reflection(&valid, "main").is_err());
    }
    #[test]
    fn reflected_entry_must_match_the_signed_request() {
        let valid = serde_json::json!({"parameters":[], "entryPoints":[{"name":"blueMain", "stage":"fragment"}]});
        super::validate_fragment_reflection(&valid, "blueMain").unwrap();
        assert!(super::validate_fragment_reflection(&valid, "greenMain").is_err());
        for value in [
            serde_json::json!({"parameters":[]}),
            serde_json::json!({"parameters":[],"entryPoints":[{"name":"blueMain"}]}),
            serde_json::json!({"parameters":[],"entryPoints":[{"name":"blueMain","stage":"vertex"}]}),
            serde_json::json!({"parameters":[],"entryPoints":[{"name":"blueMain","stage":"compute"}]}),
            serde_json::json!({"parameters":[],"entryPoints":[]}),
            serde_json::json!({"parameters":[],"entryPoints":[{"name":"blueMain"},{"name":"greenMain"}]}),
        ] {
            assert!(super::validate_fragment_reflection(&value, "blueMain").is_err());
        }
    }
}

#[cfg(all(test, any(target_os = "linux", windows)))]
mod platform_tests {
    use super::*;
    #[test]
    #[ignore = "requires pinned Slang and executes confined native compiler processes"]
    fn signed_native_shader_builds_and_denied_host_include() {
        let root = tempfile::Builder::new()
            .prefix("gamepkg shaders with spaces ")
            .tempdir()
            .unwrap();
        let source = root.path().join("source");
        for name in [
            "manifest.toml",
            "thumbnail.png",
            "LICENSE",
            "NOTICE",
            "CREDITS.md",
            "source/Cargo.toml",
            "source/Cargo.lock",
            "shaders/main.slang",
        ] {
            let path = source.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("examples/minimal")
                    .join(name),
                path,
            )
            .unwrap();
        }
        fs::create_dir_all(source.join("source/src")).unwrap();
        fs::write(source.join("source/src/lib.rs"), "pub fn game_tick(_:u32,state:u64)->u64 { if gamepkg::draw_shader(0) { state+1 } else { state } }").unwrap();
        let path = root.path().join("shader.gamepkg");
        crate::pack::pack(&source, &path, &[113; 32]).unwrap();
        let manifest = crate::registry::install(root.path(), &path, true).unwrap();
        let archive = fs::read(&path).unwrap();
        for backend in ["spirv", "wgsl", "glsl"] {
            let settings = Settings {
                shader_backend: backend.into(),
                renderer: if backend == "glsl" {
                    "opengl"
                } else {
                    "vulkan"
                }
                .into(),
                ..Settings::default()
            };
            crate::native::compile_with_settings(root.path(), &manifest.id, &settings).unwrap();
            let sources =
                load_desktop_sources(root.path(), &archive, &manifest, &settings).unwrap();
            assert_eq!(sources.len(), 1);
            let mut game = crate::runtime::Game::load_target_configured(
                root.path(),
                &manifest.id,
                &[],
                None,
                &settings,
            )
            .unwrap();
            assert!(game.custom_renderer);
            assert_eq!(game.shader_sources.len(), 1);
            assert_eq!(game.step(1).unwrap(), 1);
            drop(game);
            let cache = playback_cache(root.path(), &archive, &settings, 1)
                .unwrap()
                .0;
            fs::write(cache.join("0.shader"), b"damaged shader").unwrap();
            assert!(load_desktop_sources(root.path(), &archive, &manifest, &settings).is_err());
            assert!(crate::runtime::Game::load_target_configured(
                root.path(),
                &manifest.id,
                &[],
                None,
                &settings
            )
            .is_err());
            println!("Verified signed native {backend} shader Build, reflection/translation, native SDK request and damaged shader rejection");
        }
        #[cfg(windows)]
        {
            let settings = Settings {
                shader_backend: "dxil".into(),
                ..Settings::default()
            };
            let mut package = Package::open(&path).unwrap();
            crate::verify::verify_embedded(&mut package).unwrap();
            compile(root.path(), &archive, &mut package, &manifest, &settings).unwrap();
            let (cache, _) = playback_cache(root.path(), &archive, &settings, 1).unwrap();
            assert!(fs::read(cache.join("0.shader"))
                .unwrap()
                .starts_with(b"DXBC"));
            assert!(
                load_desktop_sources(root.path(), &archive, &manifest, &settings)
                    .unwrap_err()
                    .to_string()
                    .contains("Vulkan shaders need")
            );
            fs::write(cache.join("0.shader"), b"damaged DXIL").unwrap();
            assert!(
                compile(root.path(), &archive, &mut package, &manifest, &settings)
                    .unwrap_err()
                    .to_string()
                    .contains("shader cache is damaged")
            );
            println!("Verified signed confined DXIL compilation, reflection/cache, corrupt-output rejection and explicit renderer limitation; no DXIL playback claim");
        }
        let backends = if cfg!(windows) {
            vec!["spirv", "dxil"]
        } else {
            vec!["spirv"]
        };
        for backend in backends {
            let settings = Settings {
                shader_backend: backend.into(),
                ..Settings::default()
            };
            let (compiler, _) = compiler_cache(root.path(), &archive, &settings).unwrap();
            let dxc = dxc_root(root.path(), &settings).unwrap();
            let stage = root.path().join(format!("include-boundary-{backend}"));
            fs::create_dir_all(&stage).unwrap();
            let canary = root.path().join("not-granted.slang");
            fs::write(
                &canary,
                "float4 privateColor() { return float4(0.75,0.5,0.25,1.0); }",
            )
            .unwrap();
            fs::write(stage.join("main.slang"), format!("#include \"{}\"\n[shader(\"fragment\")] float4 fragmentMain() : SV_Target {{ return privateColor(); }}", canary.to_str().unwrap().replace('\\', "/"))).unwrap();
            let shader = Shader {
                source: "main.slang".into(),
                entry: "fragmentMain".into(),
                stage: "fragment".into(),
            };
            let args = shader_args(&shader, &settings, "control.spv", "control.json");
            let mut command = std::process::Command::new(compiler.join(if cfg!(windows) {
                "bin/slangc.exe"
            } else {
                "bin/slangc"
            }));
            command.args(&args).current_dir(&stage);
            #[cfg(windows)]
            if let Some(dxc) = &dxc {
                command
                    .arg("-dxc-path")
                    .arg(crate::takedawn_windows_sdk::compiler_path(&dxc.join("bin/x64")).unwrap());
            }
            let control =
                crate::compiler::run_timed(&mut command, std::time::Duration::from_secs(20))
                    .unwrap();
            assert!(control.status.success(), "{}", control.stderr);
            assert!(
                run_shader(&compiler, dxc.as_deref(), &stage, &args).is_err(),
                "confined Slang must not read an ungranted host include"
            );
            println!("Verified confined {backend} host-include denial with successful same-file unconfined control; no game media used");
        }
    }
}
