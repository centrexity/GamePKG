#[test]
#[cfg(target_os = "linux")]
fn owned_code_caches_preserve_transfer_faults_fetch_and_context() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let binary = work.path().join("cpu-code");
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
        .arg(root.join("tests/portable_cpu_code.c"));
    for name in [
        "cpu_code",
        "cpu",
        "interrupt",
        "far_memory",
        "far_jump",
        "memory",
        "fault",
    ] {
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
