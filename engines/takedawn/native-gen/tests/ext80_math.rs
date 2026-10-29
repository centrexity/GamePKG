#[test]
fn export_ext80_oracle() {
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_EXT80_FIXTURE") {
        std::fs::write(path, include_str!("ext80_oracle.c")).unwrap();
    }
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn extended_precision_math_matches_x87_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("ext80.c");
    let binary = dir.path().join("ext80");
    std::fs::write(&source, include_str!("ext80_oracle.c")).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let sf = runtime.join("softfloat");
    let mut cmd = std::process::Command::new("cc");
    cmd.args([
        "-std=c11",
        "-O3",
        "-msse2",
        "-pthread",
        "-mfpmath=sse",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-fsanitize=undefined",
        "-fno-sanitize-recover=all",
    ])
    .arg("-I")
    .arg(&runtime)
    .arg("-I")
    .arg(&sf)
    .arg("-I")
    .arg(sf.join("source/include"))
    .arg("-I")
    .arg(sf.join("source/8086-SSE"))
    .arg(source)
    .arg(runtime.join("gamepkg_x86_ext80.c"));
    for list in ["sources.txt", "ext80-sources.txt"] {
        for source in std::fs::read_to_string(sf.join(list)).unwrap().lines() {
            cmd.arg(sf.join(source));
        }
    }
    let out = cmd.arg("-o").arg(&binary).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = std::process::Command::new(binary).output().unwrap();
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    print!("{}", String::from_utf8_lossy(&out.stdout));
}
