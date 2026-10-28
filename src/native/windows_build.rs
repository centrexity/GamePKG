//! Ordinary source-game compilation using the pinned portable Rust toolchain.
//! The compiler and linker remain inside an AppContainer and owned job.
use anyhow::{ensure, Context, Result};
use gamepkg_takedawn_windows_runner::{Sandbox, WorkerProcess};
use std::{
    ffi::OsString,
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const DIAGNOSTICS: usize = 1048576;
fn capture(mut read: impl FnMut(&mut [u8]) -> io::Result<usize>) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        match read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                let keep = n.min(DIAGNOSTICS - output.len());
                output.extend_from_slice(&buffer[..keep]);
                truncated |= keep < n;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => break,
            Err(e) => return Err(e),
        }
    }
    if truncated {
        output.extend_from_slice(b"\n[compiler diagnostics truncated at 1 MiB per stream]\n");
    }
    Ok(output)
}

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
    ensure!(
        Instant::now() < end,
        "native compiler exceeded its time limit"
    );
    let worker = WorkerProcess::spawn_tool(
        compiler,
        args,
        &directory,
        &values,
        &sandbox,
        crate::compiler::ADDRESS_SPACE,
    )
    .context("native compilation requires Windows AppContainer isolation")?;
    let (process, mut pipe, mut errors) = worker.into_parts();
    let (send, receive) = std::sync::mpsc::channel();
    let output_send = send.clone();
    std::thread::spawn(move || {
        let _ = output_send.send(capture(|bytes| pipe.read_some(bytes, end)));
    });
    std::thread::spawn(move || {
        let _ = send.send(capture(|bytes| errors.read(bytes)));
    });
    let status = loop {
        if let Some(code) = process.poll()? {
            break code;
        }
        ensure!(
            Instant::now() < end,
            "native compiler exceeded its time limit"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    // Close any inherited pipe writers in a leftover contained child too.
    process.stop()?;
    let mut diagnostics = Vec::new();
    for _ in 0..2 {
        diagnostics.extend(
            receive
                .recv_timeout(end.saturating_duration_since(Instant::now()))
                .context("native compiler output exceeded its time limit")??,
        );
    }
    ensure!(
        Instant::now() < end,
        "native compiler exceeded its time limit"
    );
    ensure!(
        status == 0,
        "native build failed (Windows exit {status}): {}",
        String::from_utf8_lossy(&diagnostics)
    );
    Ok(diagnostics)
}

pub(super) fn compile(
    root: &Path,
    stage: &Path,
    manifest: &crate::manifest::Manifest,
    settings: &crate::settings::Settings,
    needs_shaders: bool,
) -> Result<PathBuf> {
    ensure!(
        manifest.builtin_engine.is_none(),
        "this engine requires its reviewed native builder"
    );
    ensure!(
        !needs_shaders,
        "isolated shader builds currently require Linux"
    );
    crate::source::parse_build_tree(stage)?;
    let (toolchain, target) = crate::takedawn_windows_sdk::prepare_source(root)?;
    let toolchain = toolchain.canonicalize()?;
    let stage = stage.canonicalize()?;
    let output = stage.join("game.exe");
    // This is an executable path, so retain canonical Windows extended-path
    // syntax. Only GNU file/search/output arguments need ordinary syntax.
    // Use the official bundled LLD directly rather than the legacy GCC driver.
    let linker = toolchain.join(format!("lib/rustlib/{target}/bin/rust-lld.exe"));
    // Avoid depending on Rust 1.89's captured-child streams, whose system
    // pipe-filesystem access can fail in an AppContainer. Let Rust create its
    // normal static library (including allocator/runtime support), then launch
    // LLD directly through the host's private pipes in another confined job.
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
        "-flavor",
        "gnu",
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
        let linker = toolchain.join(format!("lib/rustlib/{target}/bin/rust-lld.exe"));
        compare_linker_controls(&stage, &toolchain, target, &linker);
        let version = run(
            &linker,
            &["-flavor".into(), "gnu".into(), "--version".into()],
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
        let binary = compile(root.path(), &stage, &manifest, &settings, false).unwrap();
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
        let error = compile(root.path(), &stage, &manifest, &settings, false).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("native build failed"), "{message}");
        assert!(!message.contains("private compiler marker"));
        assert_eq!(std::fs::read(&secret).unwrap(), b"private compiler marker");
        println!("Verified pinned standalone Rust compile, PE identity, confined binary worker exchange and denied host-file include; no WSL or Python process is used.");
        signed_package_uses_generated_sdk(root.path(), &settings);
    }

    fn compare_linker_controls(stage: &Path, toolchain: &Path, target: &str, linker: &Path) {
        let args: Vec<OsString> = ["-flavor", "gnu", "--version"].map(Into::into).into();
        let (directory, values) = environment(stage, toolchain, target).unwrap();
        let mut command = std::process::Command::new(linker);
        command
            .args(&args)
            .current_dir(&directory)
            .env_clear()
            .envs(values.iter().cloned());
        let control = crate::compiler::run_timed(&mut command, Duration::from_secs(10)).unwrap();
        println!(
            "Unconfined same-tool/environment control: {} stdout={} stderr={}",
            control.status, control.stdout, control.stderr
        );
        assert!(
            control.status.success(),
            "the linker must work in the positive control"
        );

        // The existing reviewed-engine launcher uses regular log files, not
        // private duplex stdio pipes. Compare that boundary independently.
        let sandbox = Sandbox::open(&format!(
            "GamePKG.LinkerFileControl.p{}",
            std::process::id()
        ))
        .unwrap();
        sandbox.grant_runtime(toolchain).unwrap();
        sandbox.grant_saves(stage).unwrap();
        let disc = stage.join("synthetic-read-only-capability");
        std::fs::write(&disc, b"no original media").unwrap();
        let log = stage.join("linker-file-control.log");
        let launch = gamepkg_takedawn_windows_runner::Launch {
            executable: linker.to_owned(),
            args,
            cwd: directory,
            environment: values,
            disc: std::fs::File::open(disc).unwrap(),
            output: std::fs::File::create(&log).unwrap(),
            memory_limit_bytes: Some(crate::compiler::ADDRESS_SPACE),
        };
        let mut session =
            gamepkg_takedawn_windows_runner::Session::start_confined(launch, sandbox).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let code = loop {
            if let Some(event) = session.poll().unwrap() {
                match event {
                    gamepkg_takedawn_windows_runner::Event::Exited(code) => break code,
                    gamepkg_takedawn_windows_runner::Event::Restarted { .. } => {
                        panic!("version probe unexpectedly requested a restart")
                    }
                }
            }
            assert!(Instant::now() < end, "file-log linker control timed out");
            std::thread::sleep(Duration::from_millis(10));
        };
        drop(session);
        println!(
            "Confined regular-file output control: exit={code} log={}",
            std::fs::read_to_string(log).unwrap()
        );
    }

    fn signed_package_uses_generated_sdk(root: &Path, settings: &crate::settings::Settings) {
        let source = root.join("signed source");
        for name in ["manifest.toml", "source/Cargo.toml", "source/Cargo.lock"] {
            let path = source.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("examples/minimal")
                    .join(name),
                path,
            )
            .unwrap();
        }
        std::fs::create_dir_all(source.join("source/src")).unwrap();
        std::fs::write(
            source.join("source/src/lib.rs"),
            "pub fn game_tick(input:u32,state:u64)->u64{state+(u64::from(input)<<32)}",
        )
        .unwrap();
        let package_path = root.join("signed.gamepkg");
        crate::pack::pack(&source, &package_path, &[97; 32]).unwrap();
        let manifest = crate::registry::install(root, &package_path, true).unwrap();
        super::super::build::compile_with_settings(root, &manifest.id, settings).unwrap();
        let digest = crate::verify::digest(&std::fs::read(&package_path).unwrap());
        let built = super::super::path_for_digest(root, &digest);
        let mut bytes = std::fs::read(&built).unwrap();
        super::super::validate_executable(&bytes).unwrap();
        let open = || crate::archive::Package::open(&package_path).unwrap();
        let mut session = super::super::Session::new(
            root,
            &digest,
            open(),
            manifest.clone(),
            Vec::new(),
            settings,
            0,
        )
        .unwrap();
        assert_eq!(session.step(1, 0, &[]).unwrap(), 1u64 << 32);
        assert_eq!(session.step(3, 1u64 << 32, &[]).unwrap(), 4u64 << 32);
        drop(session);
        bytes.extend_from_slice(b"corrupt");
        std::fs::write(&built, bytes).unwrap();
        assert!(super::super::Session::new(
            root,
            &digest,
            open(),
            manifest,
            Vec::new(),
            settings,
            0
        )
        .is_err());
        println!("Verified signed source package, native-only Build, generated SDK ticks and corrupted native-build rejection through the production session.");
    }
}
