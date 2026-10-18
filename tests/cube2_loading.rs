//! Exercise production resource-loading, mouse and modifier state without game media.
#![cfg(target_os = "linux")]
fn compile_bridge(source: &str, rejected: &[&str]) {
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("bridge-test");
    let compiler = std::process::Command::new("c++")
        .args(["-std=c++11", "-Wall", "-Wextra", "-Werror"])
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("C++ compiler required for reviewed engine bridge tests");
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    assert!(std::process::Command::new(&binary)
        .status()
        .unwrap()
        .success());
    for argument in rejected {
        let result = std::process::Command::new(&binary)
            .arg(argument)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "{source}: {argument}");
    }
}

#[test]
fn reviewed_engine_loading_state_is_bounded_and_resets_with_resources() {
    compile_bridge("tests/cube2_loading_bridge.cpp", &[]);
}
#[test]
fn reviewed_mouse_sources_preserve_clicks_overlap_and_focus_reset() {
    compile_bridge("tests/cube2_mouse_bridge.cpp", &[]);
}

#[test]
fn reviewed_modifiers_restore_event_time_state_in_dispatch_order() {
    compile_bridge(
        "tests/cube2_modifiers_bridge.cpp",
        &[
            "--queue-full",
            "--filtered",
            "--unknown-modifier",
            "--bad-marker",
            "--negative-marker",
            "--pointer-marker",
        ],
    );
}
