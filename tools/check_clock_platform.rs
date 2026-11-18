//! Maintainer-only native clock-source matrix. Builds the exact host module,
//! with its existing pinned libc on Unix; no game or graphics is executed.
use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Stdio},
};
fn field<'a>(section: &'a str, name: &str) -> &'a str {
    section
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name} = \"")))
        .unwrap()
        .strip_suffix('"')
        .unwrap()
}
fn main() {
    let app = PathBuf::from(env::args_os().nth(1).expect("app directory"))
        .canonicalize()
        .unwrap();
    let wine = env::args().any(|arg| arg == "--wine");
    let lock = fs::read_to_string(app.join("Cargo.lock")).unwrap();
    let libc = lock
        .split("[[package]]")
        .find(|s| s.lines().any(|line| line == "name = \"libc\""))
        .unwrap();
    let root = env::temp_dir().join(format!(
        "gamepkg-clock-platform-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"),format!("[package]\nname=\"gamepkg-clock-probe\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[target.'cfg(unix)'.dependencies]\nlibc=\"={}\"\n",field(libc,"version"))).unwrap();

    fs::create_dir_all(root.join("src/port_host/timing")).unwrap();
    fs::create_dir_all(root.join("engines/native-services")).unwrap();
    for (original, staged) in [
        ("src/port_host/timing.rs", "src/port_host/timing/mod.rs"),
        (
            "src/port_host/timing/platform.rs",
            "src/port_host/timing/platform.rs",
        ),
        (
            "engines/native-services/local_runtime.rs",
            "engines/native-services/local_runtime.rs",
        ),
        (
            "engines/native-services/local_memory.rs",
            "engines/native-services/local_memory.rs",
        ),
        (
            "engines/native-services/performance_clock.rs",
            "engines/native-services/performance_clock.rs",
        ),
        (
            "engines/native-services/local_timing.rs",
            "engines/native-services/local_timing.rs",
        ),
        (
            "engines/native-services/local_timing_windows.rs",
            "engines/native-services/local_timing_windows.rs",
        ),
    ] {
        let bytes = fs::read(app.join(original)).unwrap();
        fs::write(root.join(staged), &bytes).unwrap();
        assert_eq!(bytes, fs::read(root.join(staged)).unwrap());
    }
    fs::write(
        root.join("src/lib.rs"),
        r#"
#![allow(dead_code)]
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum Error{Invalid,Denied,Failed,Busy,Unsupported}
pub type Result<T>=std::result::Result<T,Error>;
#[path="port_host/timing/mod.rs"]pub mod timing;
#[path="../engines/native-services/local_runtime.rs"]pub mod local_runtime;
"#,
    )
    .unwrap();
    for target in [
        "x86_64-unknown-linux-gnu",
        "i686-unknown-linux-gnu",
        "x86_64-pc-windows-gnu",
        "i686-pc-windows-gnu",
    ] {
        let mut build = Command::new("cargo");
        build
            .args(["test", "--offline", "--no-run", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .args([
                "--target",
                target,
                "--config",
                "target.i686-unknown-linux-gnu.linker=\"gcc-11\"",
            ])
            .env("RUSTFLAGS", "-D warnings");
        assert!(build.stdin(Stdio::null()).status().unwrap().success());
        let deps = root.join("target").join(target).join("debug/deps");
        let binary = fs::read_dir(deps)
            .unwrap()
            .map(|r| r.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("gamepkg_clock_probe-")
                    && if target.contains("windows") {
                        p.extension().is_some_and(|e| e == "exe")
                    } else {
                        p.extension().is_none()
                    }
            })
            .unwrap();
        if target.contains("windows") && !wine {
            println!("PASS {target}: trusted clock compilation only; runtime not executed");
            continue;
        }
        let mut run = if target.contains("windows") {
            let mut c = Command::new("wine");
            c.arg(&binary)
                .env("WINEDEBUG", "-all")
                .env("WINEPREFIX", "/tmp/gamepkg-port-replace-wine");
            c
        } else {
            Command::new(&binary)
        };
        assert!(run.stdin(Stdio::null()).status().unwrap().success());
        println!("PASS {target}: exact host clock arithmetic, worker-frame/revocation and real platform-source tests");
    }
    let resolved = fs::read_to_string(root.join("Cargo.lock")).unwrap();
    assert!(
        resolved.contains(field(libc, "checksum")),
        "host libc checksum changed"
    );
    fs::remove_dir_all(root).unwrap();
}
