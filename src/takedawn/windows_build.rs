//! Native Windows build dispatch for the fixed, host-reviewed source kit.
//! No ROM is opened and no game-supplied build script is accepted here.
use crate::takedawn_windows_sdk::compiler_path;
use anyhow::{ensure, Context, Result};
use std::os::windows::process::CommandExt;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) const SOURCES: &[(&str, &str)] = &[
    (
        "rustc-cache.rs",
        include_str!("../../engines/takedawn/rust-port/rustc-cache.rs"),
    ),
    (
        "build-windows-units.rs",
        include_str!("../../engines/takedawn/rust-port/build-windows-units.rs"),
    ),
    (
        "build-windows-runtime.rs",
        include_str!("../../engines/takedawn/rust-port/build-windows-runtime.rs"),
    ),
];
pub(crate) const SDK: &str = include_str!("../../engines/takedawn/windows-dependencies.json");

fn command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command.creation_flags(0x08000000);
    command
}
pub(crate) fn prepare_tools(root: &Path) -> Result<PathBuf> {
    // Preserve the verbatim SDK executable path: the bundled GCC lives deep
    // enough to exceed MAX_PATH. Only its source/output arguments use DOS paths.
    let sdk = crate::takedawn_windows_sdk::prepare(root)?.canonicalize()?;
    helpers(root, &sdk)?;
    Ok(sdk)
}
fn run(command: &mut Command) -> Result<()> {
    let result = crate::compiler::run(command)?;
    ensure!(
        result.status.success(),
        "Native Windows build failed:\n{}\n{}",
        result.stdout,
        result.stderr
    );
    Ok(())
}
fn helpers(root: &Path, sdk: &Path) -> Result<PathBuf> {
    let mut identity = SDK.to_owned();
    for (name, source) in SOURCES {
        identity.push_str(name);
        identity.push('\0');
        identity.push_str(source);
    }
    let directory = root
        .join("native-build-tools")
        .join(crate::verify::digest(identity.as_bytes()));
    fs::create_dir_all(&directory)?;
    let directory = compiler_path(&directory.canonicalize()?)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("build.lock"))?;
    lock.lock()?;
    // Stable source names/remapping keep the checkpoint executable byte-identical
    // across builds, so restarting compilation preserves completed game units.
    for (name, source) in SOURCES {
        fs::write(directory.join(name), source)?;
    }
    let rust = sdk.join("rust-1.89.0-windows-gnu");
    let linker = rust
        .join("lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained/x86_64-w64-mingw32-gcc.exe");
    for (name, _) in SOURCES {
        let source = directory.join(name);
        let executable = source.with_extension("exe");
        if crate::files::read_regular(&executable, 64 * 1048576, false).is_ok_and(|bytes| {
            crate::native::validate_executable(&bytes).is_ok()
                && crate::integrity::verify_build(&executable, &bytes).is_ok()
        }) {
            continue;
        }
        let temporary = tempfile::Builder::new()
            .suffix(".exe")
            .tempfile_in(&directory)?;
        run(command(&rust.join("bin/rustc.exe"))
            .arg(&source)
            .args([
                "--edition=2021",
                "--target",
                "x86_64-pc-windows-gnu",
                "-O",
                "-C",
                "link-self-contained=yes",
            ])
            .arg("-C")
            .arg(format!("linker={}", linker.display()))
            .arg(format!(
                "--remap-path-prefix={}=/gamepkg-build-tools",
                directory.display()
            ))
            .arg("-o")
            .arg(temporary.path()))?;
        let bytes = crate::files::read_regular(temporary.path(), 64 * 1048576, false)?;
        crate::native::validate_executable(&bytes)?;
        temporary.persist(&executable).map_err(|e| e.error)?;
        crate::integrity::record_build(&executable, &bytes)?;
    }
    Ok(directory)
}

pub fn compile_package(root: &Path, id: &str) -> Result<()> {
    ensure!(
        cfg!(target_arch = "x86_64"),
        "the native Takedawn runtime currently requires 64-bit Windows"
    );
    let _guard = crate::build::BuildChange::acquire(root, id)?;
    crate::registry::installed_manifest(root, id)?;
    let installed = root.join(format!("{id}.gamepkg"));
    let original = crate::files::read_regular(&installed, crate::archive::MAX_TOTAL, false)?;
    let mut snapshot = tempfile::NamedTempFile::new()?;
    snapshot.write_all(&original)?;
    let mut package = crate::archive::Package::open(snapshot.path())?;
    let manifest = crate::verify::verify_embedded(&mut package)?;
    ensure!(
        manifest.builtin_engine.as_deref() == Some(super::PROFILE),
        "not a Takedawn package"
    );
    super::validate_recipe(&mut package)?;
    let archive_bytes = package.read(super::KIT_ASSET)?;
    ensure!(crate::verify::digest(&archive_bytes) == super::WINDOWS_KIT_SHA256,
        "this package contains the older Linux-only source kit; update the package for native Windows builds");
    let mut archive = tempfile::NamedTempFile::new()?;
    archive.write_all(&archive_bytes)?;
    let kit = crate::takedawn_kit::SourceKit::open(archive.path(), super::WINDOWS_KIT_SHA256)?;
    drop(archive_bytes);
    let sdk = super::prepare_windows_tools(root)?.canonicalize()?;
    let helpers = helpers(root, &sdk)?;
    let cache = root.join("native-unit-cache/takedawn-windows");
    fs::create_dir_all(&cache)?;
    let cache = compiler_path(&cache.canonicalize()?)?;
    let work = tempfile::tempdir()?;
    let units = work.path().join("units");
    let output = work.path().join("runtime");
    let cpu = if crate::settings::load()?.optimize_builds {
        "native"
    } else {
        "x86-64"
    };
    run(command(&helpers.join("build-windows-units.exe"))
        .arg("--prepared")
        .arg(compiler_path(&kit.directory().join("prepared"))?)
        .arg("--rustc")
        .arg(sdk.join("rust-1.89.0-windows-gnu/bin/rustc.exe"))
        .arg("--ar")
        .arg(sdk.join("llvm-mingw-windows/bin/llvm-ar.exe"))
        .arg("--nm")
        .arg(sdk.join("llvm-mingw-windows/bin/llvm-nm.exe"))
        .arg("--cache-wrapper")
        .arg(helpers.join("rustc-cache.exe"))
        .arg("--output")
        .arg(&units)
        .arg("--cache")
        .arg(&cache)
        .arg("--cpu")
        .arg(cpu))?;
    run(command(&helpers.join("build-windows-runtime.exe"))
        .arg("--kit")
        .arg(compiler_path(kit.directory())?)
        .arg("--sdk")
        .arg(&sdk)
        .arg("--game-archive")
        .arg(units.join("libgamepkg_takedawn_generated_full_windows.a"))
        .arg("--output")
        .arg(&output)
        .arg("--cpu")
        .arg(cpu)
        .arg("--jobs")
        .arg(
            std::thread::available_parallelism()
                .map_or(1, |n| n.get())
                .to_string(),
        ))?;
    ensure!(
        crate::files::read_regular(&installed, crate::archive::MAX_TOTAL, false)? == original,
        "installed package changed during compilation"
    );
    let exe = crate::files::read_regular(
        &output.join("runtime/bin/burnout3-gamepkg-rust.exe"),
        64 * 1048576,
        false,
    )?;
    crate::native::validate_executable(&exe)?;
    let dll =
        crate::files::read_regular(&output.join("runtime/bin/SDL2.dll"), 32 * 1048576, false)?;
    let admitted = crate::files::read_regular(
        &sdk.join("SDL2/x86_64-w64-mingw32/bin/SDL2.dll"),
        32 * 1048576,
        false,
    )?;
    ensure!(
        dll == admitted,
        "native runtime SDL2 does not match the verified SDK"
    );
    let target = super::cache_path(root, &crate::verify::digest(&original));
    let parent = target.parent().context("invalid native build path")?;
    fs::create_dir_all(parent)?;
    publish(&parent.join("SDL2.dll"), &dll)?;
    publish(&target, &exe)
}
fn publish(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut output =
        tempfile::NamedTempFile::new_in(path.parent().context("invalid output path")?)?;
    output.write_all(bytes)?;
    output.as_file().sync_all()?;
    output.persist(path).map_err(|e| e.error)?;
    crate::integrity::record_build(path, bytes)
}
