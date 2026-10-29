use crate::{
    archive::Package,
    verify::{digest, verify_embedded},
};
#[cfg(target_os = "linux")]
use anyhow::{bail, Context};
use anyhow::{ensure, Result};
#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::{fs, path::Path};
pub(super) fn compile(root: &Path, id: &str) -> Result<()> {
    let settings = crate::settings::load()?;
    compile_with_settings(root, id, &settings)
}
pub(super) fn compile_with_settings(
    root: &Path,
    id: &str,
    settings: &crate::settings::Settings,
) -> Result<()> {
    settings.validate()?;
    let _guard = crate::build::BuildChange::acquire(root, id)?;
    crate::registry::installed_manifest(root, id)?;
    let original = crate::files::read_regular(
        &root.join(format!("{id}.gamepkg")),
        crate::archive::MAX_TOTAL,
        false,
    )?;
    let mut archive = tempfile::NamedTempFile::new()?;
    use std::io::Write;
    archive.write_all(&original)?;
    let mut package = Package::open(archive.path())?;
    let m = verify_embedded(&mut package)?;
    ensure!(m.entry_world == "game", "unsupported runtime entry world");
    package.resolve_shared_assets(root)?;
    let cargo: toml::Value =
        toml::from_str(std::str::from_utf8(&package.read("source/Cargo.toml")?)?)?;
    crate::dependencies::validate(&cargo, &package.read("source/Cargo.lock")?)?;
    let stage = tempfile::tempdir()?;
    let mut needs_shaders = false;
    for name in package
        .entries
        .keys()
        .filter(|n| n.starts_with("source/src/"))
        .cloned()
        .collect::<Vec<_>>()
    {
        let dest = stage.path().join(name.strip_prefix("source/").unwrap());
        fs::create_dir_all(dest.parent().unwrap())?;
        let bytes = package.read(&name)?;
        if let Ok(source) = std::str::from_utf8(&bytes) {
            needs_shaders |= super::source_flags(source)?.1;
        }
        fs::write(dest, bytes)?;
    }
    let mut bindings = include_str!("guest.rs.txt").to_owned();
    let mut helpers = include_str!("../input_buttons.rs").replace("//!", "//");
    helpers.push_str(include_str!("../guest_helpers.rs.txt"));
    helpers.push_str(include_str!("../network_guest.rs.txt"));
    helpers.push_str(include_str!("pixels.rs.txt"));
    let legacy = include_str!("../guest_api.rs.txt");
    helpers.push_str(
        &legacy[legacy.find("    pub const BUTTON_B").unwrap()
            ..legacy.find("    /// Submit mono").unwrap()],
    );
    helpers
        .push_str("\nfn shader_index(source:&str,entry:&str)->Option<u32>{match(source,entry){\n");
    if m.shaders.is_empty() {
        helpers.push_str("(\"shaders/main.slang\",\"fragmentMain\")=>Some(0),\n");
    }
    for (index, s) in m.shaders.iter().enumerate() {
        if s.stage == "fragment" {
            helpers.push_str(&format!("({:?},{:?})=>Some({index}),\n", s.source, s.entry));
        }
    }
    helpers.push_str("_=>None}}\n");
    bindings = bindings
        .replace(
            "/* GAMEPKG_CALL_LIMIT */",
            &super::MAX_CALLS_PER_TICK.to_string(),
        )
        .replace("// GAMEPKG_HELPERS", &helpers)
        .replace("// GAMEPKG_ENGINE", include_str!("engine.rs.txt"))
        .replace(
            "pub(super) fn __gamepkg_packet",
            "pub(crate) fn __gamepkg_packet",
        )
        .replace(
            "pub(super) fn __gamepkg_next_tick",
            "pub(crate) fn __gamepkg_next_tick",
        );
    let mut source = String::from_utf8(package.read("source/src/lib.rs")?)?;
    source.push_str(&bindings);
    fs::write(stage.path().join("src/lib.rs"), source)?;
    fs::write(stage.path().join("src/main.rs"),"include!(\"lib.rs\");fn main(){gamepkg::__gamepkg_initialize_engine();gamepkg::__gamepkg_packet(&[255]);while let Some((input,state))=gamepkg::__gamepkg_next_tick(){gamepkg::__gamepkg_begin_tick();let result:u64=game_tick(input,state);let mut b=vec![0];b.extend(result.to_le_bytes());gamepkg::__gamepkg_packet(&b);}}")?;
    #[cfg(target_os = "linux")]
    let binary = compile_linux(
        root,
        &original,
        &mut package,
        &m,
        settings,
        stage.path(),
        needs_shaders,
    )?;
    #[cfg(windows)]
    let binary = {
        crate::source::parse_build_tree(stage.path())?;
        if needs_shaders {
            crate::shaders::compile(root, &original, &mut package, &m, settings)?;
        }
        super::windows_build::compile(root, stage.path(), &m, settings)?
    };
    let bytes = crate::files::read_regular(&binary, 64 * 1048576, false)?;
    super::validate_executable(&bytes)?;
    ensure!(
        digest(&crate::files::read_regular(
            &root.join(format!("{id}.gamepkg")),
            crate::archive::MAX_TOTAL,
            false
        )?) == digest(&original),
        "The installed package changed during the build"
    );
    let target = super::path_for_digest(root, &digest(&original));
    fs::create_dir_all(target.parent().unwrap())?;
    let mut saved = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
    saved.write_all(&bytes)?;
    saved.as_file().sync_all()?;
    saved.persist(&target).map_err(|e| e.error)?;
    crate::integrity::record_build(&target, &bytes)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn compile_linux(
    root: &Path,
    original: &[u8],
    package: &mut Package,
    m: &crate::manifest::Manifest,
    settings: &crate::settings::Settings,
    stage: &Path,
    needs_shaders: bool,
) -> Result<PathBuf> {
    let main = stage.join("src/main.rs");
    let mut text = fs::read_to_string(&main)?
        .replace("fn main(){", "fn main(){__gamepkg_security::lockdown();");
    let mut allowed = vec![
        libc::SYS_read,
        libc::SYS_write,
        libc::SYS_close,
        libc::SYS_fstat,
        libc::SYS_lseek,
        libc::SYS_mmap,
        libc::SYS_mprotect,
        libc::SYS_munmap,
        libc::SYS_brk,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_ioctl,
        libc::SYS_readv,
        libc::SYS_writev,
        libc::SYS_madvise,
        libc::SYS_nanosleep,
        libc::SYS_getpid,
        libc::SYS_gettimeofday,
        libc::SYS_getrusage,
        libc::SYS_futex,
        libc::SYS_clock_gettime,
        libc::SYS_clock_nanosleep,
        libc::SYS_sched_yield,
        libc::SYS_exit,
        libc::SYS_exit_group,
        libc::SYS_sigaltstack,
        libc::SYS_set_tid_address,
        libc::SYS_set_robust_list,
        libc::SYS_getrandom,
        libc::SYS_restart_syscall,
        libc::SYS_gettid,
        libc::SYS_tgkill,
        libc::SYS_fcntl,
    ];
    #[cfg(target_pointer_width = "32")]
    allowed.extend([
        libc::SYS_mmap2,
        // Linux i386 and ARM use the same time64 syscall numbers. libc
        // does not export these names for every supported 32-bit target.
        403, // clock_gettime64
        407, // clock_nanosleep_time64
        422, // futex_time64
        libc::SYS_fstat64,
        libc::SYS_fcntl64,
    ]);
    allowed.sort_unstable();
    allowed.dedup();
    let arch = match std::env::consts::ARCH {
        "x86_64" => 0xc000003eu32,
        "x86" => 0x40000003,
        "aarch64" => 0xc00000b7,
        "arm" => 0x40000028,
        _ => bail!("native syscall isolation is not available for this CPU"),
    };
    text.push_str(
        &include_str!("seccomp.rs.txt")
            .replace(
                "/* GAMEPKG_SYSCALLS */",
                &allowed
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            )
            .replace("/* GAMEPKG_ARCH */", &arch.to_string()),
    );
    fs::write(main, text)?;
    crate::source::parse_build_tree(stage)?;
    let rustup = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .into_iter()
        .chain(std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cargo")))
        .map(|p| p.join("bin/rustup"))
        .find(|p| p.is_absolute() && p.is_file())
        .unwrap_or_else(|| "rustup".into());
    let mut locate = crate::compiler::command(32 * 1048576);
    locate.arg(rustup).args(["which", "rustc"]);
    let output = crate::compiler::run(&mut locate)?;
    ensure!(
        output.status.success(),
        "Install Rust to build this game: {}",
        output.stderr
    );
    let compiler = fs::canonicalize(output.stdout.trim())?;
    let toolchain = compiler
        .parent()
        .and_then(Path::parent)
        .context("invalid Rust installation")?;
    let mut extra = Vec::new();
    if let Some(profile) = m.builtin_engine.as_deref() {
        extra = engine(stage, profile, settings.optimize_builds)?;
    }
    if needs_shaders {
        crate::shaders::compile(root, original, package, m, settings)?;
    }
    let mut cmd = crate::compiler::command(64 * 1048576);
    sandbox(&mut cmd, stage);
    cmd.arg("--ro-bind")
        .arg(toolchain)
        .arg("/toolchain")
        .args([
            "--chdir",
            "/work",
            "--setenv",
            "PATH",
            "/toolchain/bin:/usr/bin",
            "--",
            "/toolchain/bin/rustc",
            "--target",
            env!("GAMEPKG_BUILD_TARGET"),
            "--edition=2021",
            "-Awarnings",
            "-C",
            "linker=/usr/bin/clang-18",
            "-C",
            "link-arg=-fuse-ld=/usr/bin/ld.lld-18",
            "--crate-name",
            "gamepkg_native_worker",
            "src/main.rs",
            "-C",
            "panic=abort",
            "-C",
            "strip=symbols",
            "-C",
            "target-cpu=native",
            "-C",
        ])
        .arg(format!(
            "opt-level={}",
            if settings.optimize_builds { 3 } else { 0 }
        ))
        .args(if settings.optimize_builds {
            vec!["-C", "lto=thin", "-C", "codegen-units=1"]
        } else {
            Vec::new()
        })
        .args(["-o", "/work/game"])
        .args(extra);
    let output = crate::compiler::run(&mut cmd)?;
    ensure!(
        output.status.success(),
        "native build failed: {}",
        output.stderr
    );
    Ok(stage.join("game"))
}

#[cfg(target_os = "linux")]
fn sandbox(cmd: &mut std::process::Command, stage: &Path) {
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
    for p in ["/lib", "/lib64", "/lib32"] {
        if Path::new(p).exists() {
            cmd.args(["--ro-bind", p, p]);
        }
    }
    cmd.arg("--bind")
        .arg(stage)
        .arg("/work")
        .args(["--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp"]);
}
#[cfg(target_os = "linux")]
fn engine(stage: &Path, profile: &str, optimize: bool) -> Result<Vec<String>> {
    ensure!(Path::new("/usr/bin/clang-18").is_file(),"Native source ports need Clang 18 (install clang-18); WASI libraries are only needed for the separate Wasm build");
    let plan = super::engine_sources::stage(stage, profile)?;
    let label = plan.label;
    fs::create_dir(stage.join("objects"))?;
    let mut cmd = crate::compiler::command(64 * 1048576);
    sandbox(&mut cmd, stage);
    cmd.args([
        "--chdir",
        "/work/objects",
        "--",
        "/usr/bin/clang-18",
        "-target",
        env!("GAMEPKG_BUILD_TARGET"),
        "-c",
        "-fPIC",
        "-fno-strict-aliasing",
        "-ffunction-sections",
        "-fdata-sections",
    ])
    .arg(if optimize { "-O3" } else { "-O0" })
    .arg(if matches!(std::env::consts::ARCH, "arm" | "aarch64") {
        "-mcpu=native"
    } else {
        "-march=native"
    });
    for include in &plan.includes {
        cmd.arg(format!("-I/work/{include}"));
    }
    for definition in plan.definitions {
        cmd.arg(format!("-D{definition}"));
    }
    cmd.args(plan.sources.iter().map(|source| format!("/work/{source}")));
    let out = crate::compiler::run(&mut cmd)?;
    if !out.status.success() {
        bail!("native {label} engine compilation failed: {}", out.stderr);
    }
    let mut args = vec!["--cfg".into(), plan.configuration.into()];
    for entry in fs::read_dir(stage.join("objects"))? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|e| e == "o") {
            args.extend([
                "-C".into(),
                format!(
                    "link-arg=/work/objects/{}",
                    entry.file_name().to_string_lossy()
                ),
            ]);
        }
    }
    args.extend(["-C".into(), "link-arg=-lm".into()]);
    for name in plan.wraps {
        args.extend(["-C".into(), format!("link-arg=-Wl,--wrap={name}")]);
    }
    Ok(args)
}
