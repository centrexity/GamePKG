//! Offline native compilation of the fixed, verified Rust source kit.
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
const RUST_CACHE_WRAPPER: &str = include_str!("../../engines/takedawn/rust-port/rustc-cache.rs");

struct UnitCache {
    directory: PathBuf,
    context: String,
}

fn unit_cache(kit: &Path, tools: &Path, persistent: Option<&Path>) -> Result<UnitCache> {
    let directory = persistent.map_or_else(|| kit.join("unit-cache"), Path::to_owned);
    ensure!(
        !directory.symlink_metadata().is_ok_and(|m| !m.is_dir()),
        "native unit cache must be a real directory"
    );
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)?;
    let directory = directory.canonicalize()?;
    let version = Command::new(tools.join("bin/rustc"))
        .args(["--version", "--verbose"])
        .output()?;
    ensure!(
        version.status.success(),
        "unable to identify the Rust compiler"
    );
    let context = crate::verify::digest(
        format!(
            "GamePKG Rust unit toolchain v1\0{}\0{}\0{}\0{}",
            super::KIT_SHA256,
            crate::native::cpu_identity(),
            std::str::from_utf8(&version.stdout)?,
            crate::verify::digest(RUST_CACHE_WRAPPER.as_bytes())
        )
        .as_bytes(),
    );
    fs::write(kit.join("gamepkg-rustc-cache.rs"), RUST_CACHE_WRAPPER)?;
    let output = Command::new(tools.join("bin/rustc"))
        .arg(kit.join("gamepkg-rustc-cache.rs"))
        .args(["--edition=2021", "-O", "-o"])
        .arg(kit.join("gamepkg-rustc-cache"))
        .output()?;
    ensure!(
        output.status.success(),
        "unable to prepare native Rust cache wrapper: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    #[cfg(test)]
    eprintln!(
        "Verified Rust unit cache: {} ({context})",
        directory.display()
    );
    Ok(UnitCache { directory, context })
}

fn rust_sysroot() -> Result<PathBuf> {
    let output = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .context("Install stable Rust before building this game")?;
    ensure!(
        output.status.success(),
        "unable to locate the Rust compiler"
    );
    let root = PathBuf::from(std::str::from_utf8(&output.stdout)?.trim()).canonicalize()?;
    ensure!(
        root.join("bin/rustc").is_file() && root.join("bin/cargo").is_file(),
        "incomplete Rust toolchain"
    );
    Ok(root)
}

fn run(
    kit: &Path,
    toolchain: &Path,
    sysroot: Option<&Path>,
    cache: &UnitCache,
    cwd: &str,
    args: &[String],
) -> Result<()> {
    let mut command = Command::new("/usr/bin/timeout");
    command.args([
        "--signal=TERM",
        "--kill-after=5s",
        "14400",
        "/usr/bin/bwrap",
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--clearenv",
        "--ro-bind",
        "/usr",
        "/usr",
    ]);
    for path in ["/bin", "/lib", "/lib64", "/etc/ld.so.cache"] {
        if Path::new(path).exists() {
            command.args(["--ro-bind", path, path]);
        }
    }
    command
        .arg("--ro-bind")
        .arg(toolchain)
        .arg("/rust")
        .arg("--bind")
        .arg(kit)
        .arg("/sdk")
        .args([
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--dir",
            "/home",
            "--dir",
            "/home/gamepkg",
            "--setenv",
            "HOME",
            "/home/gamepkg",
            "--setenv",
            "PATH",
            "/rust/bin:/usr/bin:/bin",
            "--setenv",
            "RUSTC",
            "/rust/bin/rustc",
            "--setenv",
            "RUSTFLAGS",
            "-C linker=/usr/bin/clang-18 -C link-arg=-fuse-ld=/usr/bin/ld.lld-18",
            "--setenv",
            "CARGO_HOME",
            "/tmp/cargo",
            "--setenv",
            "LANG",
            "C.UTF-8",
            "--setenv",
            "GAMEPKG_TARGET_CPU",
            "native",
            "--chdir",
            cwd,
        ]);
    if let Some(sysroot) = sysroot {
        command.arg("--ro-bind").arg(sysroot).arg("/native");
    }
    command
        .arg("--bind")
        .arg(&cache.directory)
        .arg("/unit-cache")
        .args([
            "--setenv",
            "RUSTC",
            "/sdk/gamepkg-rustc-cache",
            "--setenv",
            "GAMEPKG_REAL_RUSTC",
            "/rust/bin/rustc",
            "--setenv",
            "GAMEPKG_RUST_UNIT_ROOT",
            "/sdk/prepared/src",
            "--setenv",
            "GAMEPKG_RUST_UNIT_CACHE",
            "/unit-cache",
            "--setenv",
            "GAMEPKG_RUST_UNIT_CONTEXT",
            &cache.context,
        ]);
    command.arg("--").args(args);
    use std::os::unix::process::CommandExt;
    let parent = unsafe { libc::getpid() };
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
            }
            Ok(())
        });
    }
    let output = crate::compiler::run(&mut command)?;
    ensure!(output.status.success(), "Native Takedawn build failed. Install Clang 18, CMake, Ninja, SDL2/epoxy/OpenSSL development files and Vulkan/glslang 15 development tools, or set Native tools folder in Settings. GAMEPKG_TAKEDAWN_NATIVE_SYSROOT overrides that folder.\n{}\n{}", output.stdout, output.stderr);
    Ok(())
}

/// Only a source kit whose digest matches host metadata is accepted. The
/// compiler sees system tools and its private source/output tree; no ROM,
/// library, player saves, home directory or network is mounted into the build.
pub fn compile(archive: &Path) -> Result<Vec<u8>> {
    compile_with_options(archive, true)
}
pub fn compile_with_options(archive: &Path, optimize: bool) -> Result<Vec<u8>> {
    compile_with_cache(archive, optimize, None)
}
fn compile_with_cache(
    archive: &Path,
    optimize: bool,
    persistent: Option<&Path>,
) -> Result<Vec<u8>> {
    ensure!(
        cfg!(target_arch = "x86_64"),
        "This prepared Takedawn source kit currently supports Linux x86-64"
    );
    let kit = crate::takedawn_kit::SourceKit::open(archive, super::KIT_SHA256)?;
    let tools = rust_sysroot()?;
    let cache = unit_cache(kit.directory(), &tools, persistent)?;
    #[cfg(test)]
    eprintln!(
        "Verified private compiler tree: {}",
        kit.directory().display()
    );
    let saved = crate::settings::load()?.native_sysroot_directory;
    let native = std::env::var_os("GAMEPKG_TAKEDAWN_NATIVE_SYSROOT")
        .or_else(|| (!saved.is_empty()).then(|| saved.into()))
        .map(PathBuf::from)
        .map(|p| p.canonicalize())
        .transpose()?;
    for tool in [
        "/usr/bin/clang-18",
        "/usr/bin/clang++-18",
        "/usr/bin/cmake",
        "/usr/bin/ninja",
        "/usr/bin/ld.lld-18",
        "/usr/bin/sha256sum",
    ] {
        ensure!(
            Path::new(tool).is_file(),
            "missing native build tool {tool}"
        );
    }
    // Discover missing native dependencies before compiling 24,001 game
    // functions. This host-owned probe reads no package or ROM instructions.
    fs::create_dir(kit.directory().join("gamepkg-preflight"))?;
    fs::write(kit.directory().join("gamepkg-preflight/CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\nproject(GamePKGNativeTools C CXX)\nfind_package(Vulkan REQUIRED)\nfind_package(glslang 15 CONFIG REQUIRED)\nfind_package(OpenSSL REQUIRED)\nfind_package(SDL2 REQUIRED)\nfind_package(PkgConfig REQUIRED)\npkg_check_modules(EPOXY REQUIRED epoxy)\n")?;
    let mut probe = vec![
        "cmake",
        "-S",
        "/sdk/gamepkg-preflight",
        "-B",
        "/sdk/gamepkg-preflight-build",
        "-G",
        "Ninja",
        "-DCMAKE_C_COMPILER=/usr/bin/clang-18",
        "-DCMAKE_CXX_COMPILER=/usr/bin/clang++-18",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    if native.is_some() {
        probe.push("-DCMAKE_PREFIX_PATH=/native".into());
        probe.push(
            "-DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=/usr/bin/ld.lld-18 -L/native/lib/x86_64-linux-gnu"
                .into(),
        );
    } else {
        probe.push("-DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=/usr/bin/ld.lld-18".into());
    }
    run(
        kit.directory(),
        &tools,
        native.as_deref(),
        &cache,
        "/sdk",
        &probe,
    )?;
    let mut rust_args = vec!["cargo".into(), "build".into(), "--offline".into()];
    if optimize {
        rust_args.push("--release".into());
    }
    run(
        kit.directory(),
        &tools,
        native.as_deref(),
        &cache,
        "/sdk/prepared",
        &rust_args,
    )?;
    run(
        kit.directory(),
        &tools,
        native.as_deref(),
        &cache,
        "/sdk/host/engines/takedawn/media-bridge",
        &[
            "cargo".into(),
            "build".into(),
            "--offline".into(),
            "--locked".into(),
            "--release".into(),
        ],
    )?;
    let mut args = vec![
        "cmake",
        "-S",
        "/sdk/game",
        "-B",
        "/sdk/native-build",
        "-G",
        "Ninja",
        "-DCMAKE_BUILD_TYPE=Release",
        "-DCMAKE_C_COMPILER=/usr/bin/clang-18",
        "-DCMAKE_CXX_COMPILER=/usr/bin/clang++-18",
        "-DXBOXRECOMP_DIR=/sdk/toolkit",
        "-DBO3_BIN_DIR=/sdk/native-bin",
        "-DBO3_GLYPHS=ON",
        "-DBO3_GLYPHS_ART_DIR=/sdk/glyphs",
        "-DCMAKE_PROJECT_TOP_LEVEL_INCLUDES=/sdk/host/engines/takedawn/rust-port/attach.cmake",
        "-DGAMEPKG_TAKEDAWN_RUST_TLS=/sdk/prepared/native_tls.c",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    args.push(format!(
        "-DGAMEPKG_TAKEDAWN_RUST_ARCHIVE=/sdk/prepared/target/{}/libgamepkg_takedawn_generated.a",
        if optimize { "release" } else { "debug" }
    ));
    if native.is_some() {
        args.push("-DCMAKE_PREFIX_PATH=/native".into());
        args.push(
            "-DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=/usr/bin/ld.lld-18 -L/native/lib/x86_64-linux-gnu"
                .into(),
        );
    } else {
        args.push("-DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=/usr/bin/ld.lld-18".into());
    }
    run(
        kit.directory(),
        &tools,
        native.as_deref(),
        &cache,
        "/sdk",
        &args,
    )?;
    run(
        kit.directory(),
        &tools,
        native.as_deref(),
        &cache,
        "/sdk",
        &[
            "cmake".into(),
            "--build".into(),
            "/sdk/native-build".into(),
            "--target".into(),
            "burnout3".into(),
            "-j".into(),
            "2".into(),
        ],
    )?;
    let bytes = crate::files::read_regular(
        &kit.directory().join("native-bin/burnout3-gamepkg-rust"),
        64 * 1048576,
        false,
    )?;
    crate::native::validate_executable(&bytes)?;
    Ok(bytes)
}

pub fn compile_package(root: &Path, id: &str) -> Result<()> {
    let _guard = crate::build::BuildChange::acquire(root, id)?;
    crate::registry::installed_manifest(root, id)?;
    let original = crate::files::read_regular(
        &root.join(format!("{id}.gamepkg")),
        crate::archive::MAX_TOTAL,
        false,
    )?;
    let mut snapshot = tempfile::NamedTempFile::new()?;
    snapshot.write_all(&original)?;
    let mut package = crate::archive::Package::open(snapshot.path())?;
    let manifest = crate::verify::verify_embedded(&mut package)?;
    ensure!(
        manifest.builtin_engine.as_deref() == Some(super::PROFILE),
        "not a Takedawn package"
    );
    super::validate_recipe(&mut package)?;
    let mut archive = tempfile::NamedTempFile::new()?;
    archive.write_all(&package.read(super::KIT_ASSET)?)?;
    let bytes = compile_with_cache(
        archive.path(),
        crate::settings::load()?.optimize_builds,
        Some(&root.join("native-unit-cache/takedawn")),
    )?;
    ensure!(
        crate::files::read_regular(
            &root.join(format!("{id}.gamepkg")),
            crate::archive::MAX_TOTAL,
            false
        )? == original,
        "installed package changed during compilation"
    );
    let target = super::cache_path(root, &crate::verify::digest(&original));
    let parent = target.parent().context("invalid native build path")?;
    fs::create_dir_all(parent)?;
    let mut output = tempfile::NamedTempFile::new_in(parent)?;
    output.write_all(&bytes)?;
    output.as_file().sync_all()?;
    output.persist(&target).map_err(|e| e.error)?;
    crate::integrity::record_build(&target, &bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rust_unit_checkpoints_reject_changed_inputs_and_corrupt_outputs() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("cache.rs");
        let binary = directory.path().join("cache-tests");
        std::fs::write(&source, super::RUST_CACHE_WRAPPER).unwrap();
        let output = std::process::Command::new("rustc")
            .arg(&source)
            .args(["--edition=2021", "--test", "-o"])
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = std::process::Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
