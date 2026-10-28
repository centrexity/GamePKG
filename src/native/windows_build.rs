//! Ordinary source-game compilation using the pinned portable Rust toolchain.
//! The compiler and linker remain inside an AppContainer and owned job.
use anyhow::{ensure, Context, Result};
use gamepkg_takedawn_windows_runner::Sandbox;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn environment(
    stage: &Path,
    toolchain: &Path,
    target: &str,
) -> Result<(PathBuf, Vec<(OsString, OsString)>)> {
    // Third-party CRTs must receive ordinary drive/UNC working and temp paths;
    // keep canonical extended paths for image selection and ACL operations.
    let directory = crate::takedawn_windows_sdk::compiler_path(stage)?;
    let temporary = crate::takedawn_windows_sdk::compiler_path(&stage.join("tmp"))?;
    std::fs::create_dir_all(&temporary)?;
    let binary = crate::takedawn_windows_sdk::compiler_path(&toolchain.join("bin"))?;
    let linker = crate::takedawn_windows_sdk::compiler_path(
        &toolchain.join(format!("lib/rustlib/{target}/bin/self-contained")),
    )?;
    let llvm = crate::takedawn_windows_sdk::compiler_path(
        &toolchain.join(format!("lib/rustlib/{target}/bin")),
    )?;
    let values = vec![
        (
            "SystemRoot".into(),
            std::env::var_os("SystemRoot").context("Windows system directory unavailable")?,
        ),
        (
            "LOCALAPPDATA".into(),
            std::env::var_os("LOCALAPPDATA").context("Windows profile directory unavailable")?,
        ),
        ("PATH".into(), std::env::join_paths([binary, llvm, linker])?),
        ("TEMP".into(), temporary.as_os_str().to_owned()),
        ("TMP".into(), temporary.as_os_str().to_owned()),
    ];
    Ok((directory, values))
}

fn run(
    compiler: &Path,
    args: &[OsString],
    stage: &Path,
    toolchain: &Path,
    target: &str,
    end: Instant,
) -> Result<Vec<u8>> {
    let identity = crate::verify::digest(stage.as_os_str().as_encoded_bytes());
    let sandbox = Sandbox::open(&format!("GamePKG.Compiler.{}", &identity[..32]))?;
    sandbox.grant_runtime(toolchain)?;
    sandbox.grant_saves(stage)?;
    let (directory, values) = environment(stage, toolchain, target)?;
    crate::windows_compiler::run(compiler, args, &directory, &values, &sandbox, end)
}

pub(super) fn compile(
    root: &Path,
    stage: &Path,
    manifest: &crate::manifest::Manifest,
    settings: &crate::settings::Settings,
) -> Result<PathBuf> {
    ensure!(
        manifest.builtin_engine.is_none(),
        "this engine requires its reviewed native builder"
    );
    crate::source::parse_build_tree(stage)?;
    let (toolchain, target) = crate::takedawn_windows_sdk::prepare_source(root)?;
    let toolchain = toolchain.canonicalize()?;
    let stage = stage.canonicalize()?;
    let output = stage.join("game.exe");
    // This is an executable path, so retain canonical Windows extended-path
    // syntax. Only GNU file/search/output arguments need ordinary syntax.
    // The official bundled GNU linker avoids LLVM's GetLongPathNameW startup
    // lookup, which requires access outside the private toolchain directory.
    let linker = toolchain.join(format!("lib/rustlib/{target}/bin/self-contained/ld.exe"));
    // Rust creates a normal static library with allocator/runtime support.
    // The host launches both tools through the same confined process boundary,
    // so neither stage needs to launch a nested compiler/linker process.
    // No game code executes during either compilation stage.
    let entry = stage.join("src/gamepkg_native_entry.rs");
    std::fs::write(
        &entry,
        r#"include!("main.rs");
#[export_name="main"]
pub extern "C" fn __gamepkg_native_entry()->i32 {
    main();
    let _=std::io::Write::flush(&mut std::io::stdout());
    let _=std::io::Write::flush(&mut std::io::stderr());
    0
}
"#,
    )?;
    let archive = stage.join("game.a");
    let end = Instant::now() + Duration::from_secs(60);
    let mut args: Vec<OsString> = [
        "--edition=2021",
        "-Awarnings",
        "--crate-name",
        "gamepkg_native_worker",
        "src/gamepkg_native_entry.rs",
        "--crate-type=staticlib",
        "-C",
        "panic=abort",
        "-C",
        "strip=symbols",
        "-C",
        "target-cpu=native",
        "--target",
        target,
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.extend([
        "-C".into(),
        format!("opt-level={}", if settings.optimize_builds { 3 } else { 0 }).into(),
    ]);
    if settings.optimize_builds {
        args.extend(["-C", "lto=fat", "-C", "codegen-units=1"].map(Into::into));
    }
    args.push("-o".into());
    args.push(crate::takedawn_windows_sdk::compiler_path(&archive)?.into_os_string());
    let _ = run(
        &toolchain.join("bin/rustc.exe"),
        &args,
        &stage,
        &toolchain,
        target,
        end,
    )
    .context("native Rust static-library stage")?;
    let bytes = crate::files::read_regular(&archive, 256 * 1048576, false)?;
    ensure!(
        bytes.len() > 8 && bytes.starts_with(b"!<arch>\n"),
        "invalid native Rust static library"
    );
    drop(bytes);
    let lib = toolchain.join(format!("lib/rustlib/{target}/lib"));
    let file = |path: &Path| -> Result<OsString> {
        Ok(crate::takedawn_windows_sdk::compiler_path(path)?.into_os_string())
    };
    let machine = match target {
        "x86_64-pc-windows-gnu" => "i386pep",
        "i686-pc-windows-gnu" => "i386pe",
        _ => anyhow::bail!("unsupported native Windows linker target"),
    };
    let mut link: Vec<OsString> = [
        "-m",
        machine,
        "--dynamicbase",
        "--disable-auto-image-base",
        "--nxcompat",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    link.push(file(&lib.join("self-contained/crt2.o"))?);
    link.push(file(&lib.join("rsbegin.o"))?);
    link.push("--start-group".into());
    link.push(file(&archive)?);
    link.push("-L".into());
    link.push(file(&lib.join("self-contained"))?);
    // Native libraries required by the pinned Windows GNU Rust std and CRT.
    // The verified Rust static library supplies every Rust dependency itself.
    for name in [
        "kernel32",
        "ntdll",
        "userenv",
        "ws2_32",
        "dbghelp",
        "gcc_eh",
        ":libpthread.a",
        "msvcrt",
        "mingwex",
        "mingw32",
        "gcc",
        "user32",
    ] {
        link.push(format!("-l{name}").into());
    }
    link.push("--end-group".into());
    link.push(file(&lib.join("rsend.o"))?);
    link.push("--gc-sections".into());
    if settings.optimize_builds {
        link.push("-O1".into());
    }
    link.extend(["--strip-all".into(), "-o".into(), file(&output)?]);
    let _ = run(&linker, &link, &stage, &toolchain, target, end)
        .context("native Windows linker stage")?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "downloads the reviewed Windows C toolchain and checks confined compilation"]
    fn reviewed_c_compiler_preserves_target_and_denies_host_include() {
        assert!(gamepkg_takedawn_windows_runner::supports_amd64_tools());
        let root = tempfile::Builder::new()
            .prefix("gamepkg C compiler with spaces ")
            .tempdir()
            .unwrap();
        let sdk = crate::takedawn_windows_sdk::prepare(root.path()).unwrap();
        let llvm = sdk.join("llvm-mingw-windows").canonicalize().unwrap();
        let compiler = llvm.join("bin/clang.exe");
        assert!(compiler.is_file(), "reviewed Clang executable missing");
        let stage = root.path().join("private C source");
        std::fs::create_dir_all(&stage).unwrap();
        let stage = stage.canonicalize().unwrap();
        let (target, machine, pointer_bytes) = if cfg!(target_arch = "x86") {
            ("i686-w64-mingw32", 0x14cu16, 4)
        } else {
            ("x86_64-w64-mingw32", 0x8664u16, 8)
        };
        std::fs::write(stage.join("probe.c"), format!(
            "#include <stdint.h>\n#include <stddef.h>\n_Static_assert(sizeof(void*)=={pointer_bytes},\"wrong pointer width\");\n_Static_assert(sizeof(long)==4,\"wrong Windows ABI\");\nuint64_t probe(uint64_t x){{return x^UINT64_C(0x1122334455667788);}}\n"
        )).unwrap();
        let canary = root.path().join("host-canary.h");
        std::fs::write(&canary, "#define GAMEPKG_HOST_CANARY 0x5137\n").unwrap();
        let canary_path = crate::takedawn_windows_sdk::compiler_path(&canary)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        std::fs::write(
            stage.join("host-include.c"),
            format!(
            "#include \"{canary_path}\"\nint host_canary(void){{return GAMEPKG_HOST_CANARY;}}\n"
        ),
        )
        .unwrap();
        let identity = crate::verify::digest(stage.as_os_str().as_encoded_bytes());
        let sandbox = Sandbox::open(&format!("GamePKG.CCompiler.{}", &identity[..32])).unwrap();
        sandbox.grant_runtime(&llvm).unwrap();
        sandbox.grant_saves(&stage).unwrap();
        let (directory, mut values) = environment(&stage, &llvm, target).unwrap();
        let path = crate::takedawn_windows_sdk::compiler_path(&llvm.join("bin")).unwrap();
        values
            .iter_mut()
            .find(|(name, _)| name == "PATH")
            .unwrap()
            .1 = path.into_os_string();
        let execute = |arguments: &[OsString]| {
            crate::windows_compiler::run(
                &compiler,
                arguments,
                &directory,
                &values,
                &sandbox,
                Instant::now() + Duration::from_secs(60),
            )
        };
        match execute(&["--version".into()]) {
            Ok(output) => println!(
                "Confined Clang default-path control: {}",
                String::from_utf8_lossy(&output)
            ),
            Err(error) => println!("Confined Clang default-path control failed: {error:#}"),
        }
        let version = execute(&["-no-canonical-prefixes".into(), "--version".into()])
            .expect("confined Clang startup with explicit executable paths");
        assert!(String::from_utf8_lossy(&version).contains("clang version"));
        let arguments = |input: &str, output: &str| -> Vec<OsString> {
            [
                "-no-canonical-prefixes",
                "--target",
                target,
                "-std=c11",
                "-O3",
                "-fno-strict-aliasing",
                "-fno-builtin",
                "-D__USE_MINGW_ANSI_STDIO=0",
                "-D_CRTIMP=",
                "-c",
                input,
                "-o",
                output,
            ]
            .into_iter()
            .map(Into::into)
            .collect()
        };
        execute(&arguments("probe.c", "probe.o")).expect("confined target-sized C compilation");
        let verify_object = |name: &str| {
            let object = crate::files::read_regular(&stage.join(name), 2 * 1048576, false).unwrap();
            assert!(object.len() > 20);
            assert_eq!(
                u16::from_le_bytes([object[0], object[1]]),
                machine,
                "wrong COFF target"
            );
        };
        verify_object("probe.o");
        let failure = execute(&arguments("host-include.c", "denied.o"))
            .expect_err("confined Clang read an ungranted host include");
        let diagnostic = format!("{failure:#}");
        assert!(
            diagnostic.contains("host-canary.h")
                && (diagnostic.contains("not found") || diagnostic.contains("Permission denied")),
            "wrong host-include rejection: {diagnostic}"
        );
        assert!(!stage.join("denied.o").exists());
        // Deliberate test control with the same compiler, environment and file.
        // Production compilation always uses the restricted process above.
        let mut control = std::process::Command::new(&compiler);
        control
            .args(arguments("host-include.c", "control.o"))
            .current_dir(&directory)
            .env_clear()
            .envs(values.iter().map(|(k, v)| (k, v)));
        let control = crate::compiler::run_timed(&mut control, Duration::from_secs(20)).unwrap();
        assert!(control.status.success(), "{}", control.stderr);
        verify_object("control.o");
        println!("Verified confined reviewed C compiler: {target}; correct COFF/ABI and denied host include with same-file positive control; no engine admission or game media.");
    }

    #[test]
    #[ignore = "downloads and executes the official standalone Windows Rust compiler"]
    fn managed_compiler_and_native_worker_without_wsl() {
        let root = tempfile::Builder::new()
            .prefix("gamepkg source compiler with spaces ")
            .tempdir()
            .unwrap();
        let stage = root.path().join("source with spaces");
        std::fs::create_dir_all(stage.join("src")).unwrap();
        let main = stage.join("src/main.rs");
        std::fs::write(
            &main,
            r#"
use std::io::{Read, Write};
fn main() {
    if let Some(mode) = std::env::args().nth(1) {
        if mode == "--child" { println!("CAPTURED_CHILD"); return; }
        assert_eq!(mode, "--probe");
        let executable = std::env::current_exe().unwrap();
        assert!(std::process::Command::new(&executable).arg("--child").status().unwrap().success());
        println!("INHERITED_CHILD_OK");
        match std::process::Command::new(&executable).arg("--child").output() {
            Ok(output) => {
                assert!(output.status.success());
                assert!(String::from_utf8_lossy(&output.stdout).contains("CAPTURED_CHILD"));
                println!("CAPTURED_CHILD_OK");
            }
            Err(error) => { println!("CAPTURED_CHILD_ERROR={:?}", error.raw_os_error()); }
        }
        return;
    }
    assert_eq!(u64::from_le_bytes([1,2,3,4,5,6,7,8]), 0x0807060504030201);
    std::io::stdout().write_all(b"nativeIO").unwrap();
    std::io::stdout().flush().unwrap();
    let mut bytes = [0u8;8];
    std::io::stdin().read_exact(&mut bytes).unwrap();
    std::io::stdout().write_all(&bytes).unwrap();
    std::io::stdout().flush().unwrap();
}
"#,
        )
        .unwrap();
        let manifest = crate::manifest::Manifest::parse(include_bytes!(
            "../../examples/minimal/manifest.toml"
        ))
        .unwrap();
        let settings = crate::settings::Settings::default();
        let (toolchain, target) = crate::takedawn_windows_sdk::prepare_source(root.path()).unwrap();
        let toolchain = toolchain.canonicalize().unwrap();
        let linker = toolchain.join(format!("lib/rustlib/{target}/bin/self-contained/ld.exe"));
        let version = run(
            &linker,
            &["--version".into()],
            &stage.canonicalize().unwrap(),
            &toolchain,
            target,
            Instant::now() + Duration::from_secs(10),
        )
        .context("confined linker version probe before reading game inputs")
        .unwrap();
        println!(
            "Confined linker startup: {}",
            String::from_utf8_lossy(&version)
        );
        let binary = compile(root.path(), &stage, &manifest, &settings).unwrap();
        let bytes = std::fs::read(&binary).unwrap();
        super::super::validate_executable(&bytes).unwrap();
        let (toolchain, target) = crate::takedawn_windows_sdk::prepare_source(root.path()).unwrap();
        let diagnostic = run(
            &binary.canonicalize().unwrap(),
            &["--probe".into()],
            &stage.canonicalize().unwrap(),
            &toolchain.canonicalize().unwrap(),
            target,
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        let diagnostic = String::from_utf8(diagnostic).unwrap();
        assert!(diagnostic.contains("INHERITED_CHILD_OK"), "{diagnostic}");
        assert!(
            diagnostic.contains("CAPTURED_CHILD_OK")
                || diagnostic.contains("CAPTURED_CHILD_ERROR=Some(5)"),
            "{diagnostic}"
        );
        println!("Actual confined inherited/captured stdio probe: {diagnostic}");
        let runtime = root.path().join("verified worker");
        std::fs::create_dir(&runtime).unwrap();
        std::fs::write(runtime.join("game.exe"), &bytes).unwrap();
        let mut worker = super::super::transport::Worker::spawn(
            &runtime,
            128 * 1048576,
            &crate::verify::digest(&bytes),
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let mut response = [0; 8];
        worker.read_exact(&mut response, end).unwrap();
        assert_eq!(&response, b"nativeIO");
        worker.write_all(b"\x00\xff\x80\x7fRust", end).unwrap();
        worker.read_exact(&mut response, end).unwrap();
        assert_eq!(&response, b"\x00\xff\x80\x7fRust");
        worker.stop();
        drop(worker);

        // Exercise the compiler's OS boundary independently of the source
        // policy: even a direct include cannot read a non-granted host file.
        let secret = root.path().join("ungranted compiler secret");
        std::fs::write(&secret, b"private compiler marker").unwrap();
        std::fs::write(
            &main,
            format!("fn main(){{let _=include_bytes!({:?});}}", secret),
        )
        .unwrap();
        let error = compile(root.path(), &stage, &manifest, &settings).unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("native Rust static-library stage"),
            "{message}"
        );
        assert!(message.contains("isolated build failed"), "{message}");
        assert!(message.contains("os error 5"), "{message}");
        assert!(!message.contains("private compiler marker"));
        assert_eq!(std::fs::read(&secret).unwrap(), b"private compiler marker");
        println!("Verified pinned standalone Rust compile, PE identity, confined binary worker exchange and denied host-file include; no WSL or Python process is used.");
        super::super::signed_fixture::verify_signed_sdk(root.path(), &settings);
    }
}
