// Optional maintainer-built audio embedding. Runtime never opens these paths.
use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};
fn generated(output: &Path, bytes: &[u8]) {
    // A stopped build must not leave an empty include file behind while Cargo
    // still has an earlier successful build-script fingerprint.
    let temporary = output.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .expect("create generated build input");
    file.write_all(bytes).expect("write generated build input");
    file.sync_all().expect("flush generated build input");
    drop(file);
    fs::rename(&temporary, output).expect("publish generated build input");
}
fn embed(variable: &str, name: &str) {
    println!("cargo:rerun-if-env-changed={variable}");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory")).join(name);
    let bytes = match env::var_os(variable) {
        None => Vec::new(),
        Some(path) => {
            let path = PathBuf::from(path);
            assert!(
                !path.to_string_lossy().contains(['\n', '\r']),
                "invalid bundled audio build path"
            );
            println!("cargo:rerun-if-changed={}", path.display());
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .expect("open bundled audio companion")
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .expect("read bundled audio companion");
            assert!(
                bytes.len() <= 4 * 1024 * 1024 && bytes.starts_with(b"\0asm\x01\0\0\0"),
                "bundled audio companion must be a core Wasm module no larger than 4 MiB"
            );
            bytes
        }
    };
    generated(&output, &bytes);
}
fn find_executable(names: &[String]) -> Option<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    for name in names {
        let candidates = if cfg!(windows) && !name.ends_with(".exe") {
            vec![PathBuf::from(name), PathBuf::from(format!("{name}.exe"))]
        } else {
            vec![PathBuf::from(name)]
        };
        for candidate in candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
            for directory in env::split_paths(&path) {
                let found = directory.join(&candidate);
                if found.is_file() {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn resource_compiler(target: &str, target_env: &str) -> Option<PathBuf> {
    if let Some(configured) = env::var_os("GAMEPKG_RC") {
        let path = PathBuf::from(&configured);
        if path.is_file() {
            return Some(path);
        }
        let name = path.to_string_lossy().into_owned();
        return find_executable(&[name]);
    }
    if target_env == "msvc" {
        if let Some(found) = find_executable(&["rc.exe".into()]) {
            return Some(found);
        }
        let host_arch = match env::consts::ARCH {
            "x86_64" => "x64",
            "x86" => "x86",
            "aarch64" => "arm64",
            other => other,
        };
        for variable in ["ProgramFiles(x86)", "ProgramFiles", "ProgramW6432"] {
            let Some(root) = env::var_os(variable) else {
                continue;
            };
            for kit in ["Windows Kits/10/bin", "Windows Kits/8.1/bin"] {
                let directory = PathBuf::from(&root).join(kit);
                let Ok(entries) = fs::read_dir(&directory) else {
                    continue;
                };
                let mut versions = entries
                    .flatten()
                    .filter(|entry| entry.path().is_dir())
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>();
                versions.sort_by(|left, right| right.cmp(left));
                for version in versions {
                    let candidate = version.join(host_arch).join("rc.exe");
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    } else {
        let arch = target.split('-').next().unwrap_or_default();
        let mut names = Vec::new();
        if !arch.is_empty() {
            names.push(format!("{arch}-w64-mingw32-windres"));
            names.push(format!("{arch}-mingw32-windres"));
        }
        names.push("windres".into());
        find_executable(&names)
    }
}

fn embed_windows_icon(target: &str) {
    if !target.contains("-windows-") {
        return;
    }
    println!("cargo:rerun-if-env-changed=GAMEPKG_RC");
    println!("cargo:rerun-if-env-changed=GAMEPKG_LOCAL_RUNTIME_CONFIG");
    // The compatibility SDK rebuilds this crate for game hosts. Those hosts
    // must not inherit the launcher's artwork; only the app entries embed it.
    if env::var_os("GAMEPKG_LOCAL_RUNTIME_CONFIG").is_some() {
        return;
    }
    // Stand-alone build-script probes (for example the audio embedding check)
    // have no Cargo manifest metadata and do not produce a linkable app.
    let Some(manifest) = env::var_os("CARGO_MANIFEST_DIR") else {
        return;
    };
    let icon = PathBuf::from(manifest).join("html/assets/gamepkg.ico");
    assert!(icon.is_file(), "bundled GamePKG icon must exist");
    println!("cargo:rerun-if-changed={}", icon.display());
    let quoted = icon.to_string_lossy().replace('\\', "/");
    assert!(
        !quoted.contains(['"', '\n', '\r']),
        "invalid GamePKG icon resource path"
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let source = output.join("gamepkg-icon.rc");
    generated(&source, format!("1 ICON \"{quoted}\"\n").as_bytes());
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let compiler = resource_compiler(target, &target_env)
        .expect("Windows resource compiler; set GAMEPKG_RC to rc.exe or windres");
    let resource = output.join("gamepkg-icon.res");
    let status = if target_env == "msvc" {
        Command::new(&compiler)
            .arg("-nologo")
            .arg("/fo")
            .arg(&resource)
            .arg(&source)
            .status()
    } else {
        Command::new(&compiler)
            .arg("-i")
            .arg(&source)
            .arg("-o")
            .arg(&resource)
            .arg("-O")
            .arg("coff")
            .status()
    };
    assert!(
        status.expect("run Windows resource compiler").success(),
        "compile the GamePKG icon resource"
    );
    println!("cargo:rustc-link-arg={}", resource.display());
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Native games must match this app, even when rustup itself is a different
    // width (for example a 32-bit app on a 64-bit Linux installation).
    let target = env::var("TARGET").expect("Cargo target");
    println!("cargo:rustc-env=GAMEPKG_BUILD_TARGET={target}");
    embed_windows_icon(&target);
    embed("GAMEPKG_BUNDLED_AUDIO_DECODER", "audio-decoder.wasm");
    embed("GAMEPKG_BUNDLED_AUDIO_CONVERTER", "audio-converter.wasm");
    println!("cargo:rerun-if-env-changed=GAMEPKG_NATIVE_SDK_SHA256");
    if let Ok(pin) = env::var("GAMEPKG_NATIVE_SDK_SHA256") {
        assert!(
            pin.len() == 64
                && pin
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "invalid native compatibility source hash"
        );
        println!("cargo:rustc-env=GAMEPKG_NATIVE_SDK_PIN={pin}");
    }
    embed_external_engines();
}

fn embed_external_engines() {
    println!("cargo:rerun-if-changed=engines");
    let mut dirs = Vec::new();
    if let Ok(entries) = fs::read_dir("engines") {
        for entry in entries.flatten() {
            if entry.path().join("engine.toml").is_file() {
                dirs.push(entry.path());
            }
        }
    }
    dirs.sort();
    let mut text = String::from("static RECORDS: &[&str] = &[\n");
    for dir in dirs {
        let manifest = dir.join("engine.toml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        let manifest = manifest.canonicalize().expect("engine record");
        text.push_str(&format!(
            "    include_str!({:?}),\n",
            manifest.to_string_lossy()
        ));
    }
    text.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"))
        .join("external_engine_records.rs");
    generated(&out, text.as_bytes());
}
