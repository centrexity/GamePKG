// Maintainer-only optimized-code check: a dead local buffer must still be wiped.
use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    let app = PathBuf::from(env::args_os().nth(1).expect("app path"))
        .canonicalize()
        .unwrap();
    let root = env::temp_dir().join(format!("gamepkg-zeroize-codegen-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let source = root.join("probe.rs");
    fs::write(&source,format!(r#"
#![allow(dead_code)]
#[path={:?}]mod client;
#[unsafe(no_mangle)]pub fn wipe_probe()->i32{{let mut key=[13u8;64];match client::zeroize(&mut key){{Ok(())=>0,Err(code)=>code}}}}
"#,app.join("engines/native-services/client.rs"))).unwrap();
    for target in [
        "x86_64-unknown-linux-gnu",
        "i686-unknown-linux-gnu",
        "x86_64-pc-windows-gnu",
        "i686-pc-windows-gnu",
    ] {
        let out = root.join(format!("{target}.ll"));
        assert!(Command::new("rustc")
            .args([
                "--edition=2021",
                "-D",
                "warnings",
                "--crate-type=lib",
                "-C",
                "opt-level=3",
                "--emit=llvm-ir",
                "--target",
                target
            ])
            .arg(&source)
            .arg("-o")
            .arg(&out)
            .status()
            .unwrap()
            .success());
        let ir = fs::read_to_string(out).unwrap();
        assert!(ir.contains("@wipe_probe"), "missing probe {target}");
        assert_eq!(
            ir.matches("store volatile i8 0").count(),
            64,
            "dead-buffer zero stores removed/replaced on {target}"
        );
        println!(
            "PASS {target}: all 64 volatile zero stores survive optimized dead-buffer compilation"
        );
    }
    fs::remove_dir_all(root).unwrap();
}
