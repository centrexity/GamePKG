// Optional maintainer-built decoder embedding. Runtime never opens this path.
use std::{env, fs, io::Read, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GAMEPKG_BUNDLED_AUDIO_DECODER");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"))
        .join("audio-decoder.wasm");
    let bytes = match env::var_os("GAMEPKG_BUNDLED_AUDIO_DECODER") {
        None => Vec::new(),
        Some(path) => {
            let path = PathBuf::from(path);
            assert!(
                !path.to_string_lossy().contains(['\n', '\r']),
                "invalid bundled decoder build path"
            );
            println!("cargo:rerun-if-changed={}", path.display());
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .expect("open bundled audio decoder")
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .expect("read bundled audio decoder");
            assert!(
                bytes.len() <= 4 * 1024 * 1024 && bytes.starts_with(b"\0asm\x01\0\0\0"),
                "bundled audio decoder must be a core Wasm module no larger than 4 MiB"
            );
            bytes
        }
    };
    fs::write(output, bytes).expect("write bundled audio decoder");
}
