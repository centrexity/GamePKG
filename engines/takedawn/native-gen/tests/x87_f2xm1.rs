use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn fixture() -> String {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xd9, 0xf0], 0x10000, &[]).unwrap();
    let code = instruction(&x.instructions()[0]).unwrap();
    format!(
        "{}\nstatic void generated(void){{uint32_t ebp=0xabcdef12;{code}}}\n",
        include_str!("x87_f2xm1_oracle.c")
    )
}
#[test]
fn export_f2xm1_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_F2XM1_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn f2xm1_rejects_unqualified_inputs() {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for bytes in [
        &[0x66, 0xd9, 0xf0][..],
        &[0x64, 0xd9, 0xf0],
        &[0xf3, 0xd9, 0xf0],
    ] {
        let x = d.linear(bytes, 0x10000, &[]).unwrap();
        assert!(x.instructions().is_empty() || instruction(&x.instructions()[0]).is_err());
    }
    let x = d.linear(&[0xd9, 0xf0], 0x10000, &[]).unwrap();
    let mut bad = x.instructions()[0].clone();
    bad.bytes_hex = "d9fc".into();
    assert!(instruction(&bad).is_err());
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_f2xm1_matches_reference_accuracy_and_hardware_state() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("f2xm1.c");
    let binary = dir.path().join("f2xm1");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let sf = runtime.join("softfloat");
    let trans = runtime.join("transcendentals");
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut reference = vec![tests.join("f2xm1_reference/bridge.cc")];
    for file in std::fs::read_to_string(trans.join("reference-sources.txt"))
        .unwrap()
        .lines()
    {
        reference.push(trans.join(file));
    }
    let mut objects = Vec::new();
    for (i, file) in reference.iter().enumerate() {
        let object = dir.path().join(format!("reference-{i}.o"));
        let out = std::process::Command::new("g++")
            .args(["-std=c++17", "-O3", "-fno-exceptions", "-fno-rtti"])
            .arg("-I")
            .arg(tests.join("f2xm1_reference"))
            .arg("-I")
            .arg(trans.join("reference/bochs/cpu/softfloat3e/include"))
            .arg("-I")
            .arg(trans.join("reference/bochs/cpu/fpu"))
            .arg("-I")
            .arg(trans.join("reference/bochs/cpu"))
            .arg("-c")
            .arg(file)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        objects.push(object);
    }
    let mut cmd = std::process::Command::new("cc");
    cmd.arg("-DGAMEPKG_F2XM1_MPFR=1")
        .arg("-I")
        .arg(&tests)
        .arg("-I")
        .arg(std::env::var_os("GAMEPKG_MPFR_INCLUDE").unwrap_or_else(|| "/usr/include".into()));
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
        "gamepkg_x86_x87_trans.c",
        "gamepkg_x86_f128.c",
        "transcendentals/gamepkg_f2xm1.c",
        "transcendentals/gamepkg_fsincos.c",
        "transcendentals/gamepkg_fyl2x.c",
        "transcendentals/gamepkg_fpatan.c",
        "gamepkg_x86_ext80.c",
        "gamepkg_x86_fault.c",
    ] {
        cmd.arg(runtime.join(s));
    }
    for list in ["sources.txt", "ext80-sources.txt", "wide-sources.txt"] {
        for s in std::fs::read_to_string(sf.join(list)).unwrap().lines() {
            cmd.arg(sf.join(s));
        }
    }
    let out = cmd
        .args(objects)
        .args(["-Wl,-l:libmpfr.so.6", "-lgmp"])
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
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

#[test]
fn transcendental_sources_match_reviewed_pins() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/transcendentals");
    let lock: serde_json::Value = serde_json::from_str(include_str!(
        "../runtime/transcendentals/upstream-lock.json"
    ))
    .unwrap();
    for file in lock["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert!(!path.contains("..") && !path.starts_with('/'));
        assert_eq!(
            hex::encode(Sha256::digest(std::fs::read(root.join(path)).unwrap())),
            file["sha256"].as_str().unwrap(),
            "{path}"
        );
    }
}
