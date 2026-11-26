use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn fixture() -> String {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let mut code = String::from(include_str!("x87_trig_push_oracle.c"));
    for (name, byte) in [("pair", 0xfb), ("tan", 0xf2)] {
        let x = d.linear(&[0xd9, byte], 0x10000, &[]).unwrap();
        let emitted = instruction(&x.instructions()[0]).unwrap();
        code.push_str(&format!(
            "\nstatic void generated_{name}(void){{uint32_t ebp=0xabcdef12;{emitted}}}\n"
        ));
    }
    code.push_str("static void generated(void){if(which)generated_tan();else generated_pair();}\n");
    code
}

#[test]
fn export_trig_push_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_TRIG_PUSH_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn trig_push_rejects_unqualified_inputs() {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for opcode in [0xfb, 0xf2] {
        for prefix in [0x66, 0x64, 0xf3] {
            let x = d.linear(&[prefix, 0xd9, opcode], 0x10000, &[]).unwrap();
            assert!(x.instructions().is_empty() || instruction(&x.instructions()[0]).is_err());
        }
        let x = d.linear(&[0xd9, opcode], 0x10000, &[]).unwrap();
        let mut bad = x.instructions()[0].clone();
        bad.bytes_hex = "d9f0".into();
        assert!(instruction(&bad).is_err());
        let mut bad = x.instructions()[0].clone();
        bad.mnemonic = if opcode == 0xfb { "fptan" } else { "fsincos" }.into();
        assert!(instruction(&bad).is_err());
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn run_fixture(source: String) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("trig_push.c");
    let binary = dir.path().join("trig_push");
    std::fs::write(&source_path, source).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let sf = runtime.join("softfloat");
    let trans = runtime.join("transcendentals");
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut reference = vec![tests.join("trig_push_reference/bridge.cc")];
    for file in std::fs::read_to_string(trans.join("sincos-reference-sources.txt"))
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
    cmd.arg("-DGAMEPKG_TRIG_PUSH_MPFR=1")
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
    .arg(&source_path);
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
    std::process::Command::new(binary).output().unwrap()
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_trig_push_matches_reference_accuracy_and_hardware_state() {
    let out = run_fixture(fixture());
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    print!("{}", String::from_utf8_lossy(&out.stdout));
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_trig_push_accepts_tiny_sincos_c1_difference() {
    let source = fixture();
    let marker = "if ((z->status & status_mask) != (sw & status_mask)) {";
    assert_eq!(source.matches(marker).count(), 1);
    // Force the captured tiny sine C1 to the opposite generated value without
    // changing any result bytes, exceptions, stack/tags or the implementation.
    let altered = source.replace(marker, &format!(
        "{marker}\n  if (!which && !empty && a.sign_exponent==0x3fba && a.significand==UINT64_C(0x8000000000000000) && ((sw>>11)&7)==((top+7)&7)) {{ sw=(sw&~0x200u)|(((((z->status>>9)&1u)^1u)<<9)); wr(after.b+2,sw); }}"
    ));
    let out = run_fixture(altered);
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn force_tiny_sincos_cos(sig: &str, marker_text: &'static str) -> (String, &'static str) {
    let source = fixture();
    let marker = "unsigned sw = rd(after.b + 2), valid = 0, status_difference = 0;";
    assert_eq!(source.matches(marker).count(), 1);
    // The generated cosine for this tiny input is exactly 1.0. Replace the
    // hardware result before any MPFR check, without touching flags or state.
    let altered = source.replace(marker, &format!(
        "{marker}\n  if (!which && !empty && a.sign_exponent==0x3fba && a.significand==UINT64_C(0x8000000000000000) && ((sw>>11)&7)==((top+7)&7) && z->sign_exponent[(sw>>11)&7]==0x3fff && z->significand[(sw>>11)&7]==UINT64_C(0x8000000000000000)) {{ for (unsigned k=0;k<8;++k) after.b[32+k]=(unsigned char)(({sig})>>(8*k)); wr(after.b+40,0x3ffe); fprintf(stderr,\"{marker_text}\\n\"); }}"
    ));
    (altered, marker_text)
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_trig_push_accepts_tiny_sincos_adjacent_value() {
    let (source, marker) = force_tiny_sincos_cos("UINT64_MAX", "FORCED_TINY_ADJACENT");
    let out = run_fixture(source);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(marker),
        "tiny case was not exercised: {stderr}"
    );
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        stderr
    );
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_trig_push_accepts_verified_tiny_sincos_ue_difference() {
    let source = fixture();
    let marker = "unsigned sw = rd(after.b + 2), valid = 0, status_difference = 0;";
    assert_eq!(source.matches(marker).count(), 1);
    // Flip only the captured UE bit for a tiny denormal input; result values
    // and every other status/state bit stay unchanged.
    let altered = source.replace(marker, &format!(
        "{marker}\n  if (!which && !empty && a.sign_exponent==1 && a.significand==UINT64_C(0x8000000000000000) && ((sw>>11)&7)==((top+7)&7)) {{ sw^=0x10u; wr(after.b+2,sw); fprintf(stderr,\"FORCED_TINY_UE\\n\"); }}"
    ));
    let out = run_fixture(altered);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("FORCED_TINY_UE"),
        "tiny case was not exercised: {stderr}"
    );
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        stderr
    );
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_trig_push_rejects_non_adjacent_tiny_sincos_value() {
    let (source, marker) = force_tiny_sincos_cos("UINT64_MAX - 1u", "FORCED_TINY_NON_ADJACENT");
    let out = run_fixture(source);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(marker),
        "tiny case was not exercised: {stderr}"
    );
    assert!(
        !out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        stderr
    );
}
