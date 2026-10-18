//! Loading state must distinguish cold resources from ordinary cached frames.
#![cfg(target_os = "linux")]
#[test]
fn reviewed_engine_loading_state_is_bounded_and_resets_with_resources() {
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("loading-test");
    let compiler = std::process::Command::new("c++")
        .args(["-std=c++11", "-Wall", "-Wextra", "-Werror"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/cube2_loading_bridge.cpp"
        ))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("C++ compiler required for reviewed engine bridge tests");
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    assert!(std::process::Command::new(binary)
        .status()
        .unwrap()
        .success());
}
