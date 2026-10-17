use crate::{
    archive::Package,
    verify::{digest, verify_embedded},
};
use anyhow::{bail, ensure, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub(super) fn compile(root: &Path, id: &str) -> Result<()> {
    let settings = crate::settings::load()?;
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
    fs::write(stage.path().join("src/main.rs"),"include!(\"lib.rs\");fn main(){gamepkg::__gamepkg_initialize_engine();gamepkg::__gamepkg_packet(&[255]);while let Some((input,state))=gamepkg::__gamepkg_next_tick(){let result:u64=game_tick(input,state);let mut b=vec![0];b.extend(result.to_le_bytes());gamepkg::__gamepkg_packet(&b);}}")?;
    let main = stage.path().join("src/main.rs");
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
    crate::source::parse_build_tree(stage.path())?;
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
        extra = engine(stage.path(), profile, settings.optimize_builds)?;
    }
    if needs_shaders {
        crate::shaders::compile(root, &original, &mut package, &m, &settings)?;
    }
    let mut cmd = crate::compiler::command(64 * 1048576);
    sandbox(&mut cmd, stage.path());
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
            if settings.optimize_builds { 2 } else { 0 }
        ))
        .args(["-o", "/work/game"])
        .args(extra);
    let output = crate::compiler::run(&mut cmd)?;
    ensure!(
        output.status.success(),
        "native build failed: {}",
        output.stderr
    );
    let bytes = crate::files::read_regular(&stage.path().join("game"), 64 * 1048576, false)?;
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
    for p in ["/lib", "/lib64"] {
        if Path::new(p).exists() {
            cmd.args(["--ro-bind", p, p]);
        }
    }
    cmd.arg("--bind")
        .arg(stage)
        .arg("/work")
        .args(["--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp"]);
}
fn engine(stage: &Path, profile: &str, optimize: bool) -> Result<Vec<String>> {
    let doom = profile == crate::doom::PROFILE;
    ensure!(
        doom || profile == crate::zelda3::PROFILE,
        "unsupported native engine"
    );
    ensure!(Path::new("/usr/bin/clang-18").is_file(),"Native source ports need Clang 18 (install clang-18); WASI libraries are only needed for the separate Wasm build");
    let (kit, label) = if doom {
        (crate::doom_sources::FILES, "doom")
    } else {
        (crate::zelda3_sources::FILES, "zelda3")
    };
    for (name, bytes) in kit {
        let p = stage.join(name);
        fs::create_dir_all(p.parent().unwrap())?;
        fs::write(p, bytes)?;
    }
    let lock: serde_json::Value = serde_json::from_slice(if doom {
        include_bytes!("../../engines/doom/upstream-lock.json")
    } else {
        include_bytes!("../../engines/zelda3/upstream-lock.json")
    })?;
    for (name, hash) in lock["files"].as_object().context("invalid engine lock")? {
        if name.ends_with(".py") {
            continue;
        }
        ensure!(
            digest(&fs::read(
                stage.join(format!("engines/{label}/upstream/{name}"))
            )?) == hash.as_str().unwrap_or(""),
            "pinned engine source changed: {name}"
        );
    }
    if !doom {
        let path = stage.join("engines/zelda3/upstream/src/zelda_rtl.c");
        let mut source = fs::read_to_string(&path)?;
        for name in ["ZeldaReadSram", "ZeldaWriteSram"] {
            let begin = source
                .find(&format!("void {name}() {{"))
                .context("missing pinned SRAM function")?;
            let brace = begin + source[begin..].find('{').unwrap();
            let mut depth = 0;
            let mut end = None;
            for (offset, byte) in source.as_bytes()[brace..].iter().enumerate() {
                match byte {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    end = Some(brace + offset + 1);
                    break;
                }
            }
            source.replace_range(begin..end.context("invalid pinned SRAM function")?, "");
        }
        fs::write(path, source)?;
    }
    let adapter = stage.join(format!("engines/{label}/adapters/engine.c"));
    let mut text = fs::read_to_string(&adapter)?;
    if doom {
        let a = text.find("__wasi_errno_t __wrap___wasi_fd_write").unwrap();
        let b = text.find("void DG_Init").unwrap();
        text.replace_range(a..b, "");
        text = text.replace("#include <wasi/api.h>", "").replace(
            "#define EXPORT(name) __attribute__((export_name(name)))",
            "#define EXPORT(name)",
        );
    } else {
        text = text.replace(
            "const char *file, int line, const char *function",
            "const char *file, unsigned int line, const char *function",
        );
        let a = text.find("#include <wasi/api.h>").unwrap();
        let b = text[a..]
            .find("__attribute__((export_name(\"reset\")))")
            .unwrap()
            + a;
        text.replace_range(a..b, "");
        for name in [
            "allocate",
            "initialize",
            "step",
            "pixels",
            "audio",
            "sram",
            "save_dirty",
            "reset",
            "status",
            "position",
            "controller",
        ] {
            text = text.replace(&format!("__attribute__((export_name(\"{name}\")))"), "");
        }
    }
    if doom {
        text.push_str("\nint __wrap_printf(const char *format,...){return 0;} int __wrap_puts(const char *text){return 0;} int __wrap_putchar(int c){return c;}\n");
    }
    if !doom {
        text.push_str(include_str!("zelda_stdio.c.txt"));
    }
    fs::write(adapter, text)?;
    let mut sources: Vec<String> = if doom {
        lock["sources"]
            .as_array()
            .context("missing sources")?
            .iter()
            .map(|n| format!("/work/engines/doom/upstream/{}", n.as_str().unwrap()))
            .collect()
    } else {
        let mut v = Vec::new();
        for (name, _) in kit {
            if let Some(tail) = name.strip_prefix("engines/zelda3/upstream/src/") {
                if tail.ends_with(".c")
                    && !tail.contains('/')
                    && ![
                        "main.c",
                        "audio.c",
                        "config.c",
                        "glsl_shader.c",
                        "opengl.c",
                        "zelda_cpu_infra.c",
                    ]
                    .contains(&tail)
                {
                    v.push(format!("/work/{name}"));
                }
            }
        }
        v.sort();
        for n in ["ppu.c", "dma.c", "dsp.c", "snes_other.c"] {
            v.push(format!("/work/engines/zelda3/upstream/snes/{n}"));
        }
        v
    };
    sources.push(format!("/work/engines/{label}/adapters/engine.c"));
    fs::create_dir(stage.join("objects"))?;
    let mut cmd = crate::compiler::command(64 * 1048576);
    sandbox(&mut cmd, stage);
    cmd.args([
        "--chdir",
        "/work/objects",
        "--",
        "/usr/bin/clang-18",
        "-c",
        "-fPIC",
        "-march=native",
        "-fno-strict-aliasing",
        "-ffunction-sections",
        "-fdata-sections",
    ])
    .arg(if optimize { "-O2" } else { "-O0" })
    .arg(format!("-I/work/engines/{label}/upstream"))
    .arg(format!("-I/work/engines/{label}/adapters/include"));
    if doom {
        cmd.args([
            "-DDOOMGENERIC_RESX=320",
            "-DDOOMGENERIC_RESY=200",
            "-DFEATURE_SOUND",
            "-DNORMALUNIX",
            "-D_DEFAULT_SOURCE",
        ]);
    }
    cmd.args(sources);
    let out = crate::compiler::run(&mut cmd)?;
    if !out.status.success() {
        bail!("native {label} engine compilation failed: {}", out.stderr);
    }
    let mut args = vec![
        "--cfg".into(),
        if doom {
            "gamepkg_doom".into()
        } else {
            "gamepkg_zelda".into()
        },
    ];
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
    if doom {
        for name in [
            "fopen", "fclose", "fread", "fwrite", "fseek", "ftell", "feof", "ferror", "fflush",
            "fgetc", "fputc", "fprintf", "fgets", "remove", "rename", "mkdir", "getenv", "access",
            "printf", "puts", "putchar",
        ] {
            args.extend(["-C".into(), format!("link-arg=-Wl,--wrap={name}")]);
        }
    }
    if !doom {
        for name in [
            "printf", "puts", "putchar", "fprintf", "fopen", "fread", "fwrite", "fclose", "getenv",
        ] {
            args.extend(["-C".into(), format!("link-arg=-Wl,--wrap={name}")]);
        }
    }
    Ok(args)
}
