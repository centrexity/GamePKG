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

fn run(
    compiler: &Path,
    args: &[OsString],
    stage: &Path,
    toolchain: &Path,
    target: &str,
) -> Result<()> {
    let identity = crate::verify::digest(stage.as_os_str().as_encoded_bytes());
    let sandbox = Sandbox::open(&format!("GamePKG.Compiler.{}", &identity[..32]))?;
    sandbox.grant_runtime(toolchain)?;
    sandbox.grant_saves(stage)?;
    let temporary = stage.join("tmp");
    std::fs::create_dir_all(&temporary)?;
    let binary = crate::takedawn_windows_sdk::compiler_path(&toolchain.join("bin"))?;
    let linker = crate::takedawn_windows_sdk::compiler_path(
        &toolchain.join(format!("lib/rustlib/{target}/bin/self-contained")),
    )?;
    let values = [
        (
            "SystemRoot".into(),
            std::env::var_os("SystemRoot").context("Windows system directory unavailable")?,
        ),
        (
            "LOCALAPPDATA".into(),
            std::env::var_os("LOCALAPPDATA").context("Windows profile directory unavailable")?,
        ),
        ("PATH".into(), std::env::join_paths([binary, linker])?),
        ("TEMP".into(), temporary.as_os_str().to_owned()),
        ("TMP".into(), temporary.as_os_str().to_owned()),
    ];
    let end = Instant::now() + Duration::from_secs(60);
    let worker = WorkerProcess::spawn_tool(
        compiler,
        args,
        stage,
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
        status == 0,
        "native build failed (Windows exit {status}): {}",
        String::from_utf8_lossy(&diagnostics)
    );
    Ok(())
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
    let mut args: Vec<OsString> = [
        "--edition=2021",
        "-Awarnings",
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
        "link-self-contained=yes",
        "-C",
        "linker-flavor=ld.lld",
        "--target",
        target,
        "-C",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.push(format!("linker={}", linker.display()).into());
    args.extend([
        "-C".into(),
        format!("opt-level={}", if settings.optimize_builds { 3 } else { 0 }).into(),
    ]);
    if settings.optimize_builds {
        args.extend(["-C", "lto=thin", "-C", "codegen-units=1"].map(Into::into));
    }
    args.push("-o".into());
    args.push(crate::takedawn_windows_sdk::compiler_path(&output)?.into_os_string());
    run(
        &toolchain.join("bin/rustc.exe"),
        &args,
        &stage,
        &toolchain,
        target,
    )?;
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
        let binary = compile(root.path(), &stage, &manifest, &settings, false).unwrap();
        let bytes = std::fs::read(&binary).unwrap();
        super::super::validate_executable(&bytes).unwrap();
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
