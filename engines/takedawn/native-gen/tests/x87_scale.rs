use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn fixture() -> String {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xd9, 0xfd], 0x10000, &[]).unwrap();
    let code = instruction(&x.instructions()[0]).unwrap();
    format!(
        "{}\nstatic void generated(void){{uint32_t ebp=0xabcdef12;{code}}}\n",
        include_str!("x87_scale_oracle.c")
    )
}
#[test]
fn export_scale_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_SCALE_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn scale_rejects_unqualified_inputs() {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for bytes in [
        &[0x66, 0xd9, 0xfd][..],
        &[0x64, 0xd9, 0xfd],
        &[0xf3, 0xd9, 0xfd],
    ] {
        let x = d.linear(bytes, 0x10000, &[]).unwrap();
        assert!(x.instructions().is_empty() || instruction(&x.instructions()[0]).is_err());
    }
    let x = d.linear(&[0xd9, 0xfd], 0x10000, &[]).unwrap();
    let mut bad = x.instructions()[0].clone();
    bad.bytes_hex = "d9fc".into();
    assert!(instruction(&bad).is_err());
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_scale_matches_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("scale.c");
    let binary = dir.path().join("scale");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let sf = runtime.join("softfloat");
    let mut cmd = std::process::Command::new("cc");
    cmd.args([
        "-std=c11",
        "-O3",
        "-msse2",
        "-mfpmath=sse",
        "-pthread",
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
    .arg(source);
    for s in [
        "gamepkg_x86_x87.c",
        "gamepkg_x86_x87_arithmetic.c",
        "gamepkg_x86_ext80.c",
        "gamepkg_x86_fault.c",
    ] {
        cmd.arg(runtime.join(s));
    }
    for list in ["sources.txt", "ext80-sources.txt"] {
        for s in std::fs::read_to_string(sf.join(list)).unwrap().lines() {
            cmd.arg(sf.join(s));
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
