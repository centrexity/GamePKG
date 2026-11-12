use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete x87 fixture"
    );
    instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = include_str!("x87_arithmetic_oracle.c").replace(
        "#include \"ext80_oracle.c\"",
        include_str!("ext80_oracle.c"),
    );
    let mut forms = vec![];
    for first in [0xd8u8, 0xdc, 0xde] {
        for index in [0u8, 1, 7] {
            for field in [0u8, 1, 4, 5, 6, 7] {
                forms.push([first, 0xc0 | (field << 3) | index]);
            }
        }
    }
    forms.push([0xd9, 0xfa]);
    for (i, bytes) in forms.iter().enumerate() {
        let code = emit(bytes).unwrap();
        c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp=0xabcdef12;{code}++completed;}}\nstatic void physical_{i}(void){{__asm__ volatile(\".byte {},{}\"::: \"memory\");}}\n",bytes[0],bytes[1]));
    }
    c.push_str("static void (*const generated[55])(void)={");
    for i in 0..forms.len() {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic void (*const physical[55])(void)={");
    for i in 0..forms.len() {
        c.push_str(&format!("physical_{i},"));
    }
    c.push_str("};\nstatic const unsigned opcodes[55]={");
    for b in &forms {
        c.push_str(&format!(
            "{},",
            ((u32::from(b[0]) & 7) << 8) | u32::from(b[1])
        ));
    }
    c.push_str("};\n");
    c
}
#[test]
fn export_x87_register_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_register_arithmetic_matches_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("x87.c");
    let binary = dir.path().join("x87");
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
        "gamepkg_x86_ext80.c",
        "gamepkg_x86_x87.c",
        "gamepkg_x86_x87_arithmetic.c",
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

#[test]
fn x87_register_lowering_rejects_unqualified_forms_and_synthetic_metadata() {
    for bytes in [
        &[0x66, 0xd8, 0xc1][..],
        &[0x67, 0xd8, 0xc1],
        &[0xf0, 0xd8, 0xc1],
        &[0xdf, 0x08], // FISTTP is outside the current CPU profile.
    ] {
        assert!(emit(bytes).is_err(), "{bytes:x?}");
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xd8, 0xc1], 0x10000, &[]).unwrap();
    let original = x.instructions()[0].clone();
    let mut bad = original.clone();
    bad.operands[0].expr_src = Some("unsafe()".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.operands[0].reg = Some("st(9)".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.mnemonic = "fsub".into();
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.operands.push(bad.operands[0].clone());
    bad.operands.push(bad.operands[0].clone());
    assert!(instruction(&bad).is_err());
    let mut cosmetic = original.clone();
    cosmetic.op_str = "unsafe()".into();
    assert_eq!(
        instruction(&original).unwrap(),
        instruction(&cosmetic).unwrap()
    );
}
