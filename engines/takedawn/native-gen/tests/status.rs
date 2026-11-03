use sha2::{Digest, Sha256};
#[test]
fn pinned_status_sources_have_complete_locked_provenance() {
    let lock: serde_json::Value =
        serde_json::from_str(include_str!("../runtime/status/upstream-lock.json")).unwrap();
    assert_eq!(lock["revision"], "fd1153552d779e1ac14b0b1c48fbd1cde894eb0a");
    let inputs: [(&str, &[u8]); 5] = [
        (
            "upstream/error.c",
            include_bytes!("../runtime/status/upstream/error.c"),
        ),
        (
            "upstream/error.h",
            include_bytes!("../runtime/status/upstream/error.h"),
        ),
        (
            "upstream/winerror.h",
            include_bytes!("../runtime/status/upstream/winerror.h"),
        ),
        (
            "upstream/ntstatus.h",
            include_bytes!("../runtime/status/upstream/ntstatus.h"),
        ),
        (
            "upstream/COPYING.LIB",
            include_bytes!("../runtime/status/upstream/COPYING.LIB"),
        ),
    ];
    assert_eq!(lock["files"].as_array().unwrap().len(), inputs.len());
    for (path, bytes) in inputs {
        let record = lock["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["path"] == path)
            .unwrap();
        assert_eq!(record["sha256"], format!("{:x}", Sha256::digest(bytes)));
    }
}
#[test]
fn export_status_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_STATUS_FIXTURE") {
        std::fs::write(path, include_str!("status_fixture.c")).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn status_conversion_matches_pinned_original_and_preserves_guest_banks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let binary = work.path().join("status");
    let mut cc = std::process::Command::new("cc");
    cc.args([
        "-std=c11",
        "-O3",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-fsanitize=address,undefined",
        "-fno-sanitize-recover=all",
    ])
    .arg("-I")
    .arg(&runtime)
    .arg("-I")
    .arg(runtime.join("status/reference"))
    .arg(root.join("tests/status_fixture.c"))
    .arg(runtime.join("status/reference/reference.c"));
    for n in ["status", "fault", "x87"] {
        cc.arg(runtime.join(format!("gamepkg_x86_{n}.c")));
    }
    let output = cc.arg("-o").arg(&binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}
