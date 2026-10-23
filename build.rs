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
    embed("GAMEPKG_BUNDLED_AUDIO_DECODER", "audio-decoder.wasm");
    embed("GAMEPKG_BUNDLED_AUDIO_CONVERTER", "audio-converter.wasm");
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
    let mut text = String::from("pub static RECORDS: &[catalog::Record] = &[\n");
    for dir in dirs {
        let manifest = dir.join("engine.toml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        let manifest = manifest.canonicalize().expect("engine record");
        let mut patches = Vec::new();
        let patch_dir = dir.join("patches");
        if patch_dir.is_dir() {
            let mut names: Vec<_> = fs::read_dir(&patch_dir)
                .expect("engine patches")
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "patch"))
                .collect();
            names.sort();
            for path in names {
                println!("cargo:rerun-if-changed={}", path.display());
                patches.push(path.canonicalize().expect("engine patch"));
            }
        }
        text.push_str("    catalog::Record {\n        manifest: include_str!(r#\"");
        text.push_str(&manifest.display().to_string());
        text.push_str("\"#),\n        patches: &[\n");
        for path in patches {
            let name = path.file_name().unwrap().to_string_lossy();
            text.push_str("            (\"");
            text.push_str(&name);
            text.push_str("\", include_str!(r#\"");
            text.push_str(&path.display().to_string());
            text.push_str("\"#)),\n");
        }
        text.push_str("        ],\n    },\n");
    }
    text.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"))
        .join("external_engine_records.rs");
    fs::write(out, text).expect("write external engine records");
}
