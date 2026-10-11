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

pub(crate) struct BuildChange(fs::File);
impl BuildChange {
    pub(crate) fn acquire(root: &Path, id: &str) -> Result<Self> {
        Self::lock(root, id, false)
    }
    pub(crate) fn read(root: &Path, id: &str) -> Result<Self> {
        Self::lock(root, id, true)
    }
    fn lock(root: &Path, id: &str, shared: bool) -> Result<Self> {
        let directory = root.join("build-locks");
        if directory
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir())
        {
            bail!("build lock folder must be a real directory");
        }
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{}.lock", digest(id.as_bytes())));
        if path
            .symlink_metadata()
            .is_ok_and(|m| !m.is_file() || m.file_type().is_symlink())
        {
            bail!("build lock must be a regular file");
        }
        let mut options = fs::OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(path)?;
        (if shared {
            file.try_lock_shared()
        } else {
            file.try_lock()
        })
        .map_err(|e| anyhow::anyhow!("another build or game load is active: {e}"))?;
        Ok(Self(file))
    }
}
impl Drop for BuildChange {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
fn installed(root: &Path, id: &str) -> Result<PathBuf> {
    registry::installed_manifest(root, id)?;
    Ok(root.join(format!("{id}.gamepkg")))
}
pub fn cache_path(root: &Path, id: &str) -> Result<PathBuf> {
    let bytes = crate::files::read_regular(&installed(root, id)?, 256 * 1024 * 1024, false)?;
    Ok(root.join("builds").join(digest(&bytes)).join("game.wasm"))
}
pub fn is_built(root: &Path, id: &str) -> Result<bool> {
    let path = cache_path(root, id)?;
    let Ok(bytes) = crate::files::read_regular(&path, 32 * 1024 * 1024, false) else {
        return Ok(false);
    };
    Ok(artifact_has_tick(&bytes)
        && crate::integrity::verify_build(&cache_path(root, id)?, &bytes).is_ok())
}
pub(crate) fn artifact_has_tick(bytes: &[u8]) -> bool {
    if bytes.len() > 32 * 1024 * 1024 {
        return false;
    }
    let core = bytes.starts_with(b"\0asm\x01\0\0\0");
    let component = bytes.starts_with(b"\0asm\x0d\0\x01\0");
    if !core && !component {
        return false;
    }
    let Ok(types) = wasmparser::Validator::new().validate_all(bytes) else {
        return false;
    };
    let mut depth: u32 = 0;
    let mut found = false;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        match payload {
            Ok(
                wasmparser::Payload::ModuleSection { .. }
                | wasmparser::Payload::ComponentSection { .. },
            ) => depth += 1,
            Ok(wasmparser::Payload::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(wasmparser::Payload::ExportSection(exports)) if core && depth == 0 => {
                found |= exports.into_iter().any(|export| {
                    export.is_ok_and(|export| {
                        export.name == "gamepkg_tick"
                            && export.kind == wasmparser::ExternalKind::Func
                            && {
                                let function = types[types.as_ref().core_function_at(export.index)]
                                    .unwrap_func();
                                function.params()
                                    == [wasmparser::ValType::I32, wasmparser::ValType::I64]
                                    && function.results() == [wasmparser::ValType::I64]
                            }
                    })
                });
            }
            Ok(wasmparser::Payload::ComponentExportSection(exports)) if component && depth == 0 => {
                found |= exports.into_iter().any(|export| {
                    export.is_ok_and(|export| {
                        export.name.0 == "tick"
                            && export.kind == wasmparser::ComponentExternalKind::Func
                            && {
                                use wasmparser::{
                                    component_types::ComponentValType, PrimitiveValType,
                                };
                                let function =
                                    &types[types.as_ref().component_function_at(export.index)];
                                function.params.len() == 2
                                    && matches!(
                                        function.params[0].1,
                                        ComponentValType::Primitive(PrimitiveValType::U32)
                                    )
                                    && matches!(
                                        function.params[1].1,
                                        ComponentValType::Primitive(PrimitiveValType::U64)
                                    )
                                    && matches!(
                                        function.result,
                                        Some(ComponentValType::Primitive(PrimitiveValType::U64))
                                    )
                            }
                    })
                });
            }
            Err(_) => return false,
            _ => (),
        }
    }
    found
}
pub fn compile(root: &Path, id: &str) -> Result<()> {
    #[cfg(windows)]
    {
        crate::windows_build::compile(root, id)
    }
    #[cfg(not(windows))]
    {
        if !cfg!(target_os = "linux") {
            bail!("isolated builds currently require Linux and bubblewrap");
        }
        compile_with_settings(root, id, &crate::settings::load()?)
    }
}
pub(crate) fn compile_with_settings(
    root: &Path,
    id: &str,
    settings: &crate::settings::Settings,
) -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("isolated builds currently require Linux and bubblewrap");
    }
    settings.validate()?;
    let archive = installed(root, id)?;
    let _build_lock = BuildChange::acquire(root, id)?;
    let bytes = crate::files::read_regular(&archive, 256 * 1024 * 1024, false)?;
    let cache = root.join("builds").join(digest(&bytes)).join("game.wasm");
    let mut snapshot = tempfile::NamedTempFile::new()?;
    snapshot.write_all(&bytes)?;
    snapshot.flush()?;
    let mut pkg = Package::open(snapshot.path())?;
    let manifest = verify_embedded(&mut pkg)?;
    pkg.resolve_shared_assets(root)?;
    let cargo: toml::Value = toml::from_str(std::str::from_utf8(&pkg.read("source/Cargo.toml")?)?)?;
    let approved = crate::allowlist::approved(&cargo, &pkg.read("source/Cargo.lock")?)?;
    if approved
        && !settings
            .approved_crates
            .iter()
            .any(|name| name == crate::allowlist::NAME)
    {
        bail!("selected compiler settings do not approve the guest dependency");
    }

    let stage = tempfile::tempdir()?;
    let mut source = String::from_utf8(pkg.read("source/src/lib.rs")?)?;
    if manifest.entry_world != "game" {
        bail!("unsupported runtime entry world");
    }
    let export = if manifest.runtime_profile == "component-v1" {
        "tick"
    } else {
        "gamepkg_tick"
    };
    source.push_str(&format!("\n#[unsafe(no_mangle)] pub extern \"C\" fn {export}(input: u32, state: u64) -> u64 {{ game_tick(input, state) }}\n"));
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
    let level = if settings.optimize_builds { 2 } else { 0 };
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
    crate::shaders::compile(root, &bytes, &mut pkg, &manifest, settings)?;
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
    let artifact = crate::files::read_regular(
        &stage.path().join(format!(
            "target/wasm32-unknown-unknown/release/{crate_name}.wasm"
        )),
        32 * 1024 * 1024,
        false,
    )?;
    let installed_now =
        crate::files::read_regular(&installed(root, id)?, 256 * 1024 * 1024, false)?;
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
    if !artifact_has_tick(&artifact) {
        bail!("compiled artifact lacks a valid playable entry point");
    }
    let parent = cache.parent().context("invalid build cache")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    temp.write_all(&artifact)?;
    temp.as_file().sync_all()?;
    temp.persist(&cache).map_err(|e| e.error)?;
    crate::integrity::record_build(&cache, &artifact)?;
    Ok(())
}

/// Creator workflow: publish a signed source archive only after a real isolated build.
pub fn package(directory: &Path, output: &Path, seed: &[u8; 32]) -> Result<()> {
    let root = fs::canonicalize(directory)?;
    let parent = fs::canonicalize(
        output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    if parent.starts_with(&root) {
        bail!("output must be outside package directory");
    }
    let output = parent.join(
        output
            .file_name()
            .context("output must name a package file")?,
    );
    match fs::symlink_metadata(&output) {
        Ok(_) => bail!("output package already exists"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let stage = tempfile::tempdir()?;
    let candidate = stage.path().join("candidate.gamepkg");
    crate::pack::pack(directory, &candidate, seed)?;
    let library = stage.path().join("library");
    let manifest = crate::registry::install(&library, &candidate, true)?;
    compile(&library, &manifest.id)?;
    let mut published = tempfile::NamedTempFile::new_in(&parent)?;
    std::io::copy(&mut fs::File::open(&candidate)?, &mut published)?;
    published.as_file().sync_all()?;
    published
        .persist_noclobber(output)
        .map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::artifact_has_tick;
    #[test]
    fn game_build_locks_are_exclusive_and_release_on_drop() {
        let root = tempfile::tempdir().unwrap();
        let first = super::BuildChange::acquire(root.path(), "one").unwrap();
        assert!(super::BuildChange::acquire(root.path(), "one").is_err());
        let other = super::BuildChange::acquire(root.path(), "two").unwrap();
        drop(other);
        drop(first);
        let reader = super::BuildChange::read(root.path(), "one").unwrap();
        let second_reader = super::BuildChange::read(root.path(), "one").unwrap();
        assert!(super::BuildChange::acquire(root.path(), "one").is_err());
        drop(reader);
        drop(second_reader);
        super::BuildChange::acquire(root.path(), "one").unwrap();
    }
    #[test]
    fn readiness_requires_a_valid_root_tick_export() {
        assert!(!artifact_has_tick(&wat::parse_str("(module)").unwrap()));
        assert!(artifact_has_tick(&wat::parse_str("(module (func (export \"gamepkg_tick\") (param i32 i64) (result i64) local.get 1))").unwrap()));
        assert!(!artifact_has_tick(
            &wat::parse_str("(module (memory (export \"gamepkg_tick\") 1))").unwrap()
        ));
        assert!(!artifact_has_tick(
            &wat::parse_str("(component (core module (func (export \"gamepkg_tick\"))))").unwrap()
        ));
        assert!(!artifact_has_tick(
            &wat::parse_str("(module (func (export \"gamepkg_tick\")))").unwrap()
        ));
        assert!(!artifact_has_tick(b"\0asm\x01\0\0\0broken"));
    }
}
