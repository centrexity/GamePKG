//! Native maintainer build entry: Rust/Cargo only, including bundled audio.
//! Run from the repository root after installing wasm32-unknown-unknown.
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn run(command: &mut Command) -> io::Result<()> {
    if command.stdin(Stdio::null()).status()?.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            "build command failed; see the diagnostics above",
        ))
    }
}
fn cargo(root: &Path, offline: bool) -> Command {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["build", "--locked", "--release"]);
    if offline {
        command.arg("--offline");
    }
    command
}
fn windows_msvc_target() -> io::Result<Option<&'static str>> {
    if !cfg!(all(windows, target_env = "msvc")) {
        return Ok(None);
    }
    match env::consts::ARCH {
        "x86_64" => Ok(Some("x86_64-pc-windows-msvc")),
        "x86" => Ok(Some("i686-pc-windows-msvc")),
        "aarch64" => Ok(Some("aarch64-pc-windows-msvc")),
        _ => Err(io::Error::other(
            "unsupported native Windows build architecture",
        )),
    }
}
fn static_crt_flags(triple: &str) -> std::ffi::OsString {
    // Cargo's encoded form preserves existing flag boundaries. An explicit
    // --target keeps these target flags out of host procedural-macro builds.
    let mut flags = env::var_os("CARGO_ENCODED_RUSTFLAGS").unwrap_or_else(|| {
        let target_flags = format!(
            "CARGO_TARGET_{}_RUSTFLAGS",
            triple.to_uppercase().replace('-', "_")
        );
        env::var("RUSTFLAGS")
            .or_else(|_| env::var(target_flags))
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("\x1f")
            .into()
    });
    if !flags.is_empty() {
        flags.push("\x1f");
    }
    flags.push("-C\x1ftarget-feature=+crt-static");
    flags
}
fn companion(root: &Path, target: &Path, offline: bool, kind: &str) -> io::Result<PathBuf> {
    let mut command = cargo(root, offline);
    command
        .args(["--target", "wasm32-unknown-unknown", "--manifest-path"])
        .arg(root.join(format!("tools/audio-{kind}/Cargo.toml")))
        .env("CARGO_TARGET_DIR", target);
    if kind == "decoder" {
        command.env("GAMEPKG_AUDIO_MAX_MEMORY_BYTES", "6291456");
    }
    run(&mut command)?;
    let wasm = target
        .join("wasm32-unknown-unknown/release")
        .join(format!("gamepkg_host_audio_{kind}.wasm"));
    let bytes = fs::read(&wasm)?;
    if bytes.len() > 4 * 1024 * 1024 || !bytes.starts_with(b"\0asm\x01\0\0\0") {
        return Err(io::Error::other(
            "the audio companion is not a supported core Wasm module",
        ));
    }
    Ok(wasm)
}
fn verify_relocated(binaries: &Path) -> io::Result<()> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let directory = env::temp_dir().join(format!(
        "gamepkg-native-release-check-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory)?;
    let mut previous = None;
    for (name, option) in [
        ("gamepkg", "check-bundled-audio"),
        ("gamepkg-gui", "--check-bundled-audio"),
    ] {
        let name = format!("{name}{}", env::consts::EXE_SUFFIX);
        let executable = directory.join(&name);
        fs::copy(binaries.join(name), &executable)?;
        let output = Command::new(executable)
            .arg(option)
            .current_dir(&directory)
            .env("GAMEPKG_HOME", directory.join("library"))
            .env("GAMEPKG_CONFIG", directory.join("settings.toml"))
            .env("GAMEPKG_AUDIO_DECODER_WASM", directory.join("missing.wasm"))
            .env(
                "GAMEPKG_BUNDLED_AUDIO_DECODER",
                directory.join("missing.wasm"),
            )
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .stdin(Stdio::null())
            .output()?;
        let record = String::from_utf8(output.stdout).map_err(io::Error::other)?;
        if !output.status.success()
            || !record.contains("\"sha256\":")
            || !record.contains("\"maximum_memory_bytes\":")
        {
            return Err(io::Error::other(format!(
                "relocated audio validation failed: {}; evidence: {}",
                String::from_utf8_lossy(&output.stderr),
                directory.display()
            )));
        }
        if previous
            .as_ref()
            .is_some_and(|old: &String| old.trim() != record.trim())
        {
            return Err(io::Error::other(
                "CLI and GUI contain different bundled audio decoders",
            ));
        }
        previous = Some(record);
    }
    fs::remove_dir_all(directory)?;
    Ok(())
}
fn build() -> io::Result<()> {
    let mut offline = false;
    let mut github_env = false;
    for arg in env::args().skip(1) {
        if arg == "--offline" {
            offline = true;
        } else if arg == "--github-env" {
            github_env = true;
        } else {
            return Err(io::Error::other(
                "usage: gamepkg-build-release [--offline] [--github-env] (from the repository root)",
            ));
        }
    }
    let root = env::current_dir()?.canonicalize()?;
    if !root.join("tools/audio-decoder/Cargo.toml").is_file()
        || !root.join("src/bin/gui.rs").is_file()
    {
        return Err(io::Error::other(
            "run this builder from the GamePKG repository root",
        ));
    }
    let target = root.join(
        env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| "target".into()),
    );
    fs::create_dir_all(&target)?;
    let target = target.canonicalize()?;
    let audio = target.join("audio-companions");
    println!("Building bundled audio companions with Rust/Cargo.");
    let decoder = companion(&root, &audio, offline, "decoder")?;
    let converter = companion(&root, &audio, offline, "converter")?;
    if github_env {
        let destination = env::var_os("GITHUB_ENV")
            .ok_or_else(|| io::Error::other("--github-env requires GITHUB_ENV"))?;
        let mut output = fs::OpenOptions::new().append(true).open(destination)?;
        for (name, path) in [
            ("GAMEPKG_BUNDLED_AUDIO_DECODER", &decoder),
            ("GAMEPKG_BUNDLED_AUDIO_CONVERTER", &converter),
        ] {
            let text = path
                .to_str()
                .ok_or_else(|| io::Error::other("audio build path is not Unicode"))?;
            if text.contains(['\n', '\r']) {
                return Err(io::Error::other("audio build path contains a newline"));
            }
            writeln!(output, "{name}={text}")?;
        }
    }
    println!("Building CLI and GUI with their embedded audio.");
    let mut command = cargo(&root, offline);
    command
        .args(["--all-features", "--bins"])
        .env("CARGO_TARGET_DIR", &target)
        .env("GAMEPKG_BUNDLED_AUDIO_DECODER", decoder)
        .env("GAMEPKG_BUNDLED_AUDIO_CONVERTER", converter);
    let triple = windows_msvc_target()?;
    if let Some(triple) = triple {
        command
            .args(["--target", triple])
            .env_remove("RUSTFLAGS")
            .env("CARGO_ENCODED_RUSTFLAGS", static_crt_flags(triple));
    }
    run(&mut command)?;
    let binaries = triple.map_or_else(
        || target.join("release"),
        |t| target.join(t).join("release"),
    );
    verify_relocated(&binaries)?;
    if triple.is_some() {
        // Keep the documented packaging paths while using Cargo's separate
        // target directory for the standalone Windows executables.
        fs::create_dir_all(target.join("release"))?;
        for name in ["gamepkg.exe", "gamepkg-gui.exe"] {
            let destination = target.join("release").join(name);
            // Cargo may have hard-linked an earlier flat output into its deps
            // cache. Replace that link rather than truncating the other entry.
            match fs::remove_file(&destination) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            fs::copy(binaries.join(name), destination)?;
        }
    }
    println!(
        "CLI and GUI ready in {}. Relocated audio checks passed.",
        target.join("release").display()
    );
    Ok(())
}
fn main() {
    if let Err(error) = build() {
        eprintln!("GamePKG release build: {error}");
        std::process::exit(1);
    }
}
