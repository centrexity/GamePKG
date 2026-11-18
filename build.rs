// Optional maintainer-built audio embedding. Runtime never opens these paths.
use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
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
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Native games must match this app, even when rustup itself is a different
    // width (for example a 32-bit app on a 64-bit Linux installation).
    println!(
        "cargo:rustc-env=GAMEPKG_BUILD_TARGET={}",
        env::var("TARGET").expect("Cargo target")
    );
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
        text.push_str(&format!("    include_str!({:?}),\n", manifest.to_string_lossy()));
    }
    text.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"))
        .join("external_engine_records.rs");
    generated(&out, text.as_bytes());
}
