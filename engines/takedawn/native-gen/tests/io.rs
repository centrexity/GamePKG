#[test]
#[cfg(target_os = "linux")]
fn io_calls_preserve_handles_and_fault_semantics() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let work = tempfile::tempdir().unwrap();
    let c = work.path().join("nt.c");
    let binary = work.path().join("nt");
    std::fs::write(&c, include_str!("io_fixture.c")).unwrap();
    let mut cc = std::process::Command::new("cc");
    cc.args([
        "-DGAMEPKG_IO_ALLOCATOR_TEST=1",
        "-DGAMEPKG_IO_COPY_FAULT_TEST=1",
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
    .arg(root.join("runtime"))
    .arg(c);
    for name in [
        "io",
        "rtl_string",
        "handles",
        "irql",
        "memory",
        "fault",
        "x87",
        "cpu",
        "interrupt",
        "cpu_dispatch",
        "cpu_code",
        "dispatch",
        "far_memory",
        "far_jump",
        "table_store",
    ] {
        cc.arg(root.join(format!("runtime/gamepkg_x86_{name}.c")));
    }
    let o = cc.arg("-o").arg(&binary).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = std::process::Command::new(binary).output().unwrap();
    assert!(
        o.status.success(),
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    print!("{}", String::from_utf8_lossy(&o.stdout));
}
