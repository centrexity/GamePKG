//! Single-file, dependency-free Wasm compilation. Never fall back to a native build.
use crate::{
    archive::Package,
    registry,
    verify::{digest, verify_embedded},
};
use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn installed(root: &Path, id: &str) -> Result<PathBuf> {
    registry::list(root)?
        .into_iter()
        .find(|m| m.id == id)
        .context("package is not installed")?;
    Ok(root.join(format!("{id}.gamepkg")))
}
pub fn cache_path(root: &Path, id: &str) -> Result<PathBuf> {
    let bytes = fs::read(installed(root, id)?)?;
    Ok(root.join("builds").join(digest(&bytes)).join("game.wasm"))
}
pub fn is_built(root: &Path, id: &str) -> Result<bool> {
    let path = cache_path(root, id)?;
    let Ok(bytes) = fs::read(path) else {
        return Ok(false);
    };
    Ok(bytes.starts_with(b"\0asm\x01\0\0\0"))
}
pub fn compile(root: &Path, id: &str) -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("isolated builds currently require Linux and bubblewrap");
    }
    let archive = installed(root, id)?;
    let bytes = fs::read(archive)?;
    let cache = root.join("builds").join(digest(&bytes)).join("game.wasm");
    let mut snapshot = tempfile::NamedTempFile::new()?;
    snapshot.write_all(&bytes)?;
    snapshot.flush()?;
    let mut pkg = Package::open(snapshot.path())?;
    let manifest = verify_embedded(&mut pkg)?;
    let cargo: toml::Value = toml::from_str(std::str::from_utf8(&pkg.read("source/Cargo.toml")?)?)?;
    let edition = cargo
        .get("package")
        .and_then(|p| p.get("edition"))
        .and_then(|e| e.as_str())
        .unwrap_or("2015")
        .to_owned();
    let stage = tempfile::tempdir()?;
    let mut source = String::from_utf8(pkg.read("source/src/lib.rs")?)?;
    let parsed = syn::parse_file(&source)?;
    if parsed
        .items
        .iter()
        .any(|item| matches!(item, syn::Item::Fn(f) if f.sig.ident == "game_tick"))
    {
        source.push_str("\n#[unsafe(no_mangle)] pub extern \"C\" fn gamepkg_tick(input: u32, state: u64) -> u64 { game_tick(input, state) }\n");
    }
    source.push_str(include_str!("guest_api.rs.txt"));
    fs::write(stage.path().join("lib.rs"), source)?;
    let rustc = Command::new("rustup")
        .args(["which", "rustc"])
        .output()
        .context("Rust toolchain is required")?;
    if !rustc.status.success() {
        bail!("unable to locate Rust compiler");
    }
    let compiler = fs::canonicalize(std::str::from_utf8(&rustc.stdout)?.trim())?;
    let toolchain = compiler
        .parent()
        .and_then(Path::parent)
        .context("invalid compiler path")?;
    if !toolchain
        .join("lib/rustlib/wasm32-unknown-unknown/lib")
        .is_dir()
    {
        bail!("install the Wasm target with rustup target add wasm32-unknown-unknown");
    }
    let settings = crate::settings::load()?;
    crate::shaders::compile(root, &bytes, &mut pkg, &manifest, &settings)?;
    let mut cmd = Command::new("timeout");
    cmd.args([
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
            cmd.args(["--ro-bind", path, path]);
        }
    }
    cmd.arg("--ro-bind")
        .arg(toolchain)
        .arg("/toolchain")
        .args([
            "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp", "--bind",
        ])
        .arg(stage.path())
        .arg("/work")
        .args([
            "--chdir",
            "/work",
            "--setenv",
            "PATH",
            "/toolchain/bin:/usr/bin",
            "--",
            "/toolchain/bin/rustc",
            "lib.rs",
            "--edition",
            &edition,
            "--crate-name",
            "game",
            "--crate-type",
            "cdylib",
            "--target",
            "wasm32-unknown-unknown",
            "-o",
            "game.wasm",
        ]);
    cmd.args([
        "-C",
        if settings.optimize_builds {
            "opt-level=2"
        } else {
            "opt-level=0"
        },
    ]);
    let output = cmd
        .output()
        .context("isolated builds require timeout and bubblewrap")?;
    if !output.status.success() {
        bail!(
            "isolated Wasm build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let artifact = fs::read(stage.path().join("game.wasm"))?;
    let installed_now = fs::read(installed(root, id)?)?;
    if digest(&installed_now) != digest(&bytes) {
        bail!("installed package changed during build; rebuild the new version");
    }
    if !artifact.starts_with(b"\0asm\x01\0\0\0") {
        bail!("compiler did not produce a Wasm module");
    }
    let parent = cache.parent().context("invalid build cache")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    temp.write_all(&artifact)?;
    temp.as_file().sync_all()?;
    temp.persist(cache).map_err(|e| e.error)?;
    Ok(())
}
