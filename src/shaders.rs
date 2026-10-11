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
    process::Command,
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
    let compiler = std::env::var_os("GAMEPKG_SLANG_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            (!settings.slang_directory.is_empty()).then(|| PathBuf::from(&settings.slang_directory))
        })
        .context("install Slang 2026.19 and set its folder in Settings or GAMEPKG_SLANG_ROOT")?;
    let compiler = fs::canonicalize(compiler)?;
    let exe = compiler.join("bin/slangc");
    let version = Command::new(&exe)
        .arg("-version")
        .output()
        .context("unable to run Slang compiler")?;
    if !version.status.success()
        || format!(
            "{}{}",
            String::from_utf8_lossy(&version.stdout),
            String::from_utf8_lossy(&version.stderr)
        )
        .trim()
            != SLANG_VERSION
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
    let cache = root.join("shader-builds").join(identity);
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
        let mut command = Command::new("timeout");
        command.args([
            "60",
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
        if settings.shader_backend == "dxil" {
            command.args(["-profile", "sm_6_0"]);
        }
        let output = command
            .output()
            .context("shader builds require timeout and bubblewrap")?;
        if !output.status.success() {
            bail!(
                "isolated shader build failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
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
