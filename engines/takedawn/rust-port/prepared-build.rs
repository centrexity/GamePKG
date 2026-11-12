// Copied into prepared source kits as build.rs. Players need stable Rust and a
// native archiver, never Python, C2Rust, or the maintainer normalizer.
use std::{env, fs, path::PathBuf, process::Command};
fn run(command: &mut Command) {
    let status = command.status().expect("could not start native build tool");
    assert!(status.success(), "native build failed: {command:?}");
}
fn main() {
    println!("cargo:rerun-if-changed=units.txt");
    println!("cargo:rerun-if-changed=src");
    for key in ["RUSTC", "AR", "GAMEPKG_TARGET_CPU"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    let host = env::var("HOST").unwrap();
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let target_cpu = env::var("GAMEPKG_TARGET_CPU")
        .unwrap_or_else(|_| if target == host { "native" } else { "generic" }.into());
    assert!(!target.contains("msvc"), "the prepared archive builder currently supports GNU/LLVM archivers; MSVC integration still needs qualification");
    let units = fs::read_to_string(root.join("units.txt")).unwrap();
    let mut objects = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for unit in units.lines() {
        assert!(
            !unit.is_empty() && unit.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'),
            "invalid prepared Rust unit"
        );
        assert!(names.insert(unit), "duplicate prepared Rust unit");
        let object = out.join(format!("{unit}.o"));
        run(Command::new(&rustc)
            .arg(root.join("src").join(format!("{unit}.rs")))
            .args([
                "--edition=2021",
                "--crate-type=lib",
                "--emit=obj",
                "-Awarnings",
                "--target",
                &target,
            ])
            .arg("-C")
            .arg(format!("opt-level={}", env::var("OPT_LEVEL").unwrap()))
            .arg("-C")
            .arg(format!("target-cpu={target_cpu}"))
            .args(["-C", "panic=abort", "-C", "overflow-checks=off", "-o"])
            .arg(&object));
        objects.push(object);
    }
    assert!(!objects.is_empty(), "empty prepared Rust inventory");
    let archive = out.join("libgamepkg_takedawn_units.a");
    if archive.exists() {
        fs::remove_file(&archive).unwrap();
    }
    let ar = env::var_os("AR").unwrap_or_else(|| "ar".into());
    // Batch arguments for large kits without relying on shell expansion.
    for chunk in objects.chunks(32) {
        run(Command::new(&ar).arg("q").arg(&archive).args(chunk));
    }
    run(Command::new(&ar).arg("s").arg(&archive));
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=gamepkg_takedawn_units");
}
