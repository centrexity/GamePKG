use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn fixture() -> String {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xd9, 0xf3], 0x10000, &[]).unwrap();
    let code = instruction(&x.instructions()[0]).unwrap();
    format!(
        "{}\nstatic void generated(void){{uint32_t ebp=0xabcdef12;{code}}}\n",
        include_str!("x87_fpatan_oracle.c")
    )
}
#[test]
fn export_fpatan_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_FPATAN_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn fpatan_rejects_unqualified_inputs() {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for bytes in [
        &[0x66, 0xd9, 0xf3][..],
        &[0x64, 0xd9, 0xf3],
        &[0xf3, 0xd9, 0xf3],
    ] {
        let x = d.linear(bytes, 0x10000, &[]).unwrap();
        assert!(x.instructions().is_empty() || instruction(&x.instructions()[0]).is_err());
    }
    let x = d.linear(&[0xd9, 0xf3], 0x10000, &[]).unwrap();
    let mut bad = x.instructions()[0].clone();
    bad.bytes_hex = "d9f0".into();
    assert!(instruction(&bad).is_err());
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn run_fixture(source_text: String, export_corpus: bool) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("fpatan.c");
    let binary = dir.path().join("fpatan");
    std::fs::write(&source, source_text).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let sf = runtime.join("softfloat");
    let trans = runtime.join("transcendentals");
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut reference = vec![tests.join("fpatan_reference/bridge.cc")];
    for file in std::fs::read_to_string(trans.join("fpatan-reference-sources.txt"))
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
    cmd.arg("-DGAMEPKG_FPATAN_MPFR=1")
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
        "transcendentals/gamepkg_fpatan.c",
        "transcendentals/gamepkg_fyl2x.c",
        "transcendentals/gamepkg_f2xm1.c",
        "transcendentals/gamepkg_fsincos.c",
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
    let mut command = std::process::Command::new(binary);
    if !export_corpus {
        // Probe variants must not overwrite or race the canonical replay corpus.
        command.env_remove("GAMEPKG_TAKEDAWN_X87_FPATAN_CORPUS");
    }
    command.output().unwrap()
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn assert_fixture_matches(source_text: String, export_corpus: bool) {
    let out = run_fixture(source_text, export_corpus);
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
fn generated_fpatan_matches_reference_accuracy_and_hardware_state() {
    assert_fixture_matches(fixture(), true);
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_fpatan_tiny_rounding_does_not_depend_on_host_c1() {
    let source = fixture();
    let marker = "unsigned sw = rd(after.b + 2), valid = 0;";
    assert_eq!(source.matches(marker).count(), 1);
    // The hosted CPU returned a different approximation C1 for the same tiny
    // dyadic ratio. Exercise the opposite captured flag without altering any
    // result bytes, exceptions, stack/tags or the generated implementation.
    let altered = source.replace(marker, &format!(
        "{marker}\nif (!empty && exact_tiny(a, b) && ((sw >> 11) & 7) == ((top + 1) & 7)) {{ sw ^= 0x200u; wr(after.b + 2, sw); }}"
    ));
    assert_fixture_matches(altered, false);
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_fpatan_rejects_corrupted_tiny_rounding_flag() {
    let mut source = fixture();
    // Damage the implementation's flag after execution, preserving its value.
    // The independent reference must catch the exact case reported by CI even
    // though it accepts either captured approximation flag from the host CPU.
    let end = source.rfind("}\n").unwrap();
    source.insert_str(end, "GamePkgX87State *bad=gamepkg_x86_x87_state();unsigned t=(bad->status>>11)&7; if(bad->sign_exponent[t]==1 && bad->significand[t]==UINT64_C(0x8000000000000000))bad->status^=0x200u;");
    let out = run_fixture(source, false);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("FPATAN case85 cw40"));
    assert!(String::from_utf8_lossy(&out.stderr)
        .contains("value got0001:8000000000000000 expected0001:8000000000000000"));
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_fpatan_tiny_underflow_does_not_depend_on_host_profile() {
    let source = fixture();
    let marker = "unsigned sw = rd(after.b + 2), valid = 0;";
    assert_eq!(source.matches(marker).count(), 1);
    // Reproduce the Intel capture for the boundary where AMD returns a wrapped
    // UE result. The independent mathematical oracle must check the same
    // implementation bits regardless of this host observation.
    let altered = source.replace(marker, &format!(
        "{marker}\nif (!empty && a.sign_exponent==0x3fff && a.significand==UINT64_C(0x8000000000000000) && b.sign_exponent==1 && b.significand==UINT64_C(0x8000000000000000) && ((cw>>10)&1) && !(cw&16) && ((sw>>11)&7)==((top+1)&7)) {{ put80(after.b+32,b); sw=(sw&~16u)|0x200u; wr(after.b+2,sw); }}"
    ));
    assert_fixture_matches(altered, false);
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_fpatan_rejects_corrupted_tiny_underflow() {
    for damage in ["bad->status^=16u;", "bad->significand[t]^=1;"] {
        let mut source = fixture();
        let end = source.rfind("}\n").unwrap();
        source.insert_str(end, &format!("GamePkgX87State *bad=gamepkg_x86_x87_state();unsigned t=(bad->status>>11)&7; if(bad->sign_exponent[t]==0x6000 && bad->significand[t]==UINT64_MAX){{{damage}}}"));
        let out = run_fixture(source, false);
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("FPATAN case87701 cw440"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
