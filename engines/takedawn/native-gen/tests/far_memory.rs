#[test]
#[cfg(target_os = "linux")]
fn far_descriptor_resolution_preserves_fault_priority_and_atomic_updates() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let binary = work.path().join("far-memory");
    let mut compile = std::process::Command::new("cc");
    compile
        .args([
            "-std=c11",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pthread",
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg("-I")
        .arg(&runtime)
        .arg(root.join("tests/portable_far_memory.c"));
    for name in ["far_memory", "far_jump", "memory", "fault"] {
        compile.arg(runtime.join(format!("gamepkg_x86_{name}.c")));
    }
    let built = compile.arg("-o").arg(&binary).output().unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = std::process::Command::new(binary).output().unwrap();
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    print!("{}", String::from_utf8_lossy(&run.stdout));
}
