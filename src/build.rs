//! Offline locked Cargo builds with host-owned dependencies and no native fallback.
use crate::{
    archive::Package,
    registry,
    verify::{digest, verify_embedded},
};
use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
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
    Ok(bytes.starts_with(b"\0asm\x01\0\0\0") || bytes.starts_with(b"\0asm\x0d\0\x01\0"))
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
    pkg.resolve_shared_assets(root)?;
    let cargo: toml::Value = toml::from_str(std::str::from_utf8(&pkg.read("source/Cargo.toml")?)?)?;
    let approved = crate::allowlist::approved(&cargo, &pkg.read("source/Cargo.lock")?)?;
    let stage = tempfile::tempdir()?;
    let mut source = String::from_utf8(pkg.read("source/src/lib.rs")?)?;
    let parsed = syn::parse_file(&source)?;
    if parsed
        .items
        .iter()
        .any(|item| matches!(item, syn::Item::Fn(f) if f.sig.ident == "game_tick"))
    {
        let export = if manifest.runtime_profile == "component-v1" {
            "tick"
        } else {
            "gamepkg_tick"
        };
        source.push_str(&format!("\n#[unsafe(no_mangle)] pub extern \"C\" fn {export}(input: u32, state: u64) -> u64 {{ game_tick(input, state) }}\n"));
    }
    source.push_str(if manifest.runtime_profile == "component-v1" {
        include_str!("component_guest.rs.txt")
    } else {
        include_str!("guest_api.rs.txt")
    });
    for name in pkg
        .entries
        .keys()
        .filter(|name| name.starts_with("source/src/"))
        .cloned()
        .collect::<Vec<_>>()
    {
        let destination = stage.path().join(name.strip_prefix("source/").unwrap());
        fs::create_dir_all(destination.parent().unwrap())?;
        fs::write(destination, pkg.read(&name)?)?;
    }
    fs::write(stage.path().join("src/lib.rs"), source)?;
    let mut staged = cargo.clone();
    let table = staged.as_table_mut().unwrap();
    table.insert(
        "lib".into(),
        toml::Value::Table(toml::map::Map::from_iter([
            ("path".into(), toml::Value::String("src/lib.rs".into())),
            (
                "crate-type".into(),
                toml::Value::Array(vec![toml::Value::String("cdylib".into())]),
            ),
        ])),
    );
    let mut locked: toml::Value =
        toml::from_str(std::str::from_utf8(&pkg.read("source/Cargo.lock")?)?)?;
    locked["version"] = toml::Value::Integer(4);
    if approved {
        crate::allowlist::stage(&stage.path().join("vendor/byteorder-lite"))?;
        staged["dependencies"]["byteorder-lite"] = toml::Value::Table(toml::map::Map::from_iter([
            (
                "path".into(),
                toml::Value::String("vendor/byteorder-lite".into()),
            ),
            ("default-features".into(), toml::Value::Boolean(false)),
        ]));
        for package in locked["package"].as_array_mut().unwrap() {
            if package.get("source").is_some() {
                let table = package.as_table_mut().unwrap();
                table.remove("source");
                table.remove("checksum");
            }
        }
    }
    let level = if crate::settings::load()?.optimize_builds {
        2
    } else {
        0
    };
    let cargo_text = format!(
        "{}\n[profile.release]\nopt-level = {level}\n",
        toml::to_string(&staged)?
    );
    fs::write(stage.path().join("Cargo.toml"), cargo_text)?;
    fs::write(stage.path().join("Cargo.lock"), toml::to_string(&locked)?)?;
    let crate_name = cargo["package"]["name"].as_str().unwrap().replace('-', "_");

    let rustc = crate::compiler::run(
        crate::compiler::command(32 * 1024 * 1024).args(["rustup", "which", "rustc"]),
    )
    .context("Rust toolchain is required")?;
    if !rustc.status.success() {
        bail!("unable to locate Rust compiler");
    }
    let compiler = fs::canonicalize(rustc.stdout.trim())?;
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
    let mut cmd = crate::compiler::command(32 * 1024 * 1024);
    cmd.args([
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
            "--setenv",
            "CARGO_HOME",
            "/tmp/cargo",
            "--",
            "/toolchain/bin/cargo",
            "build",
            "--offline",
            "--locked",
            "--release",
            "--lib",
            "--target",
            "wasm32-unknown-unknown",
        ]);
    let output = crate::compiler::run(&mut cmd)?;
    if !output.status.success() {
        bail!("isolated Wasm build failed: {}", output.stderr);
    }
    let artifact = fs::read(stage.path().join(format!(
        "target/wasm32-unknown-unknown/release/{crate_name}.wasm"
    )))?;
    let installed_now = fs::read(installed(root, id)?)?;
    if digest(&installed_now) != digest(&bytes) {
        bail!("installed package changed during build; rebuild the new version");
    }
    if !artifact.starts_with(b"\0asm\x01\0\0\0") {
        bail!("compiler did not produce a Wasm module");
    }
    let artifact = if manifest.runtime_profile == "component-v1" {
        crate::hostapi::componentize(artifact).context("unable to encode game WIT component")?
    } else {
        artifact
    };
    if artifact.len() > 32 * 1024 * 1024 {
        bail!("compiled Wasm exceeds 32 MiB build limit");
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
