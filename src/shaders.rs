//! Pinned Slang compilation inside the same deny-by-default Linux build boundary.
use crate::{
    archive::Package,
    manifest::{Manifest, Shader},
    settings::Settings,
    verify::digest,
};
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
pub const SLANG_VERSION: &str = "2026.19";
pub fn compile(
    root: &Path,
    archive: &[u8],
    package: &mut Package,
    manifest: &Manifest,
    settings: &Settings,
) -> Result<PathBuf> {
    if !cfg!(target_os = "linux") {
        bail!("isolated shader builds currently require Linux");
    }
    settings.validate()?;
    let (compiler, cache) = compiler_cache(root, archive, settings)?;
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
        command
            .arg("--ro-bind")
            .arg(&compiler)
            .arg("/compiler")
            .args([
                "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp", "--bind",
            ])
            .arg(stage.path())
            .arg("/work")
            .args([
                "--chdir",
                "/work",
                "--setenv",
                "LD_LIBRARY_PATH",
                "/compiler/lib",
                "--",
                "/compiler/bin/slangc",
            ])
            .arg(&shader.source)
            .args([
                "-entry",
                &shader.entry,
                "-stage",
                &shader.stage,
                "-target",
                &settings.shader_backend,
                "-o",
                &binary,
                "-reflection-json",
                &reflection,
            ]);
        if settings.shader_backend == "spirv" {
            command.args(["-profile", "glsl_450"]);
        }
        if settings.shader_backend == "glsl" {
            command.args(["-profile", "glsl_330"]);
        }
        if settings.shader_backend == "dxil" {
            command.args(["-profile", "sm_6_0"]);
        }
        let output = crate::compiler::run(&mut command)?;
        if !output.status.success() {
            bail!("isolated shader build failed: {}", output.stderr);
        }
        let bytes = read_output(&stage.path().join(&binary))?;
        if bytes.is_empty()
            || (settings.shader_backend == "spirv" && !bytes.starts_with(&[3, 2, 35, 7]))
        {
            bail!("invalid shader compiler output");
        }
        let metadata = read_output(&stage.path().join(&reflection))?;
        let _: serde_json::Value =
            serde_json::from_slice(&metadata).context("invalid shader reflection")?;
        artifacts.push((binary, bytes));
        artifacts.push((reflection, metadata));
    }
    // Cache publication follows complete compilation; a failed shader cannot publish partial output.
    fs::create_dir_all(root.join("shader-builds"))?;
    let destination = tempfile::tempdir_in(root.join("shader-builds"))?;
    let mut cache_matches = cache.is_dir();
    for (name, bytes) in artifacts {
        cache_matches &= fs::read(cache.join(&name)).is_ok_and(|previous| previous == bytes);
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
        return Ok(cache);
    }
    fs::rename(destination.path(), &cache)?;
    Ok(cache)
}
fn read_output(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    if fs::metadata(path)?.len() > 16 * 1024 * 1024 {
        bail!("shader output exceeds 16 MiB");
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        bail!("shader output exceeds 16 MiB");
    }
    Ok(bytes)
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
            "spirv_asm"
                | "__intrinsic_asm"
                | "__require_prelude"
                | "__requirePrelude"
                | "__target_intrinsic"
                | "__spirv_instruction"
        ) {
            bail!("shader target-code injection is not allowed: {token}");
        }
    }
    Ok(())
}

fn compiler_cache(root: &Path, archive: &[u8], settings: &Settings) -> Result<(PathBuf, PathBuf)> {
    let compiler = std::env::var_os("GAMEPKG_SLANG_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            (!settings.slang_directory.is_empty()).then(|| PathBuf::from(&settings.slang_directory))
        })
        .context("install Slang 2026.19 and set its folder in Settings or GAMEPKG_SLANG_ROOT")?;
    let compiler = fs::canonicalize(compiler)?;
    let exe = compiler.join("bin/slangc");
    let version = crate::compiler::run(
        crate::compiler::command(16 * 1024 * 1024)
            .arg(&exe)
            .arg("-version"),
    )
    .context("unable to run Slang compiler")?;
    if !version.status.success()
        || format!("{}{}", version.stdout, version.stderr).trim() != SLANG_VERSION
    {
        bail!("Slang version must be exactly {SLANG_VERSION}");
    }
    let identity = digest(
        format!(
            "{}:{SLANG_VERSION}:{}:{}:{}",
            digest(archive),
            settings.shader_backend,
            digest(&fs::read(&exe)?),
            digest(&fs::read(compiler.join("lib/libslang-compiler.so"))?)
        )
        .as_bytes(),
    );
    Ok((compiler, root.join("shader-builds").join(identity)))
}

pub fn load_fragment_sources(
    root: &Path,
    archive: &[u8],
    manifest: &Manifest,
    settings: &Settings,
) -> Result<Vec<String>> {
    if settings.shader_backend != "glsl" {
        bail!("this desktop shader renderer requires GLSL; select it in Settings and rebuild");
    }
    let (_, cache) = compiler_cache(root, archive, settings)?;
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
        let metadata: serde_json::Value = serde_json::from_slice(&read_output(
            &cache.join(format!("{index}.reflection.json")),
        )?)?;
        if !metadata
            .get("parameters")
            .and_then(|v| v.as_array())
            .context("shader reflection must declare its resource parameters")?
            .is_empty()
        {
            bail!("desktop fragment passes currently require no external resource bindings");
        }
        sources.push(String::from_utf8(read_output(
            &cache.join(format!("{index}.shader")),
        )?)?);
    }
    Ok(sources)
}
