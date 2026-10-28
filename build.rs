// Optional maintainer-built audio embedding. Runtime never opens these paths.
use std::{env, fs, io::Read, path::PathBuf};
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
    fs::write(output, bytes).expect("write bundled audio companion");
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
}
