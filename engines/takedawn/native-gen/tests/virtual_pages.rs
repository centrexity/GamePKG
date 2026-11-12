#[test]
fn export_virtual_pages_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_VIRTUAL_PAGES_FIXTURE") {
        std::fs::write(p, include_str!("virtual_pages_fixture.c")).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn reserved_and_committed_pages_enforce_access_and_rollback() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let binary = work.path().join("virtual");
    let mut cc = std::process::Command::new("cc");
    cc.args([
        "-std=c11",
        "-O3",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-pthread",
        "-fsanitize=address,undefined",
        "-fno-sanitize-recover=all",
        "-DGAMEPKG_VIRTUAL_ALLOCATOR_TEST=1",
    ])
    .arg("-I")
    .arg(&runtime)
    .arg(root.join("tests/virtual_pages_fixture.c"));
    for name in ["memory", "fault", "x87"] {
        cc.arg(runtime.join(format!("gamepkg_x86_{name}.c")));
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
