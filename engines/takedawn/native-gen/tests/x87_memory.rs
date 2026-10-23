use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete memory fixture"
    );
    instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = include_str!("x87_memory_oracle.c").to_string();
    let mut forms = vec![];
    for (kind, first, width) in [
        (0u32, 0xd8u8, 4u32),
        (1, 0xdc, 8),
        (2, 0xde, 2),
        (3, 0xda, 4),
    ] {
        for field in [0u8, 1, 4, 5, 6, 7] {
            for mode in 0..6 {
                let bytes = match mode {
                    0 => vec![first, field << 3 | 6],
                    1 => vec![0x64, first, field << 3 | 6],
                    2 => vec![0x67, first, field << 3],
                    3 => vec![first, 0x84 | (field << 3), 0xfe, 0x20, 0, 0, 0],
                    4 => vec![first, 0x45 | (field << 3), 0],
                    _ => vec![first, 0x05 | (field << 3), 1, 4, 0, 0],
                };
                let code = emit(&bytes).unwrap();
                let i = forms.len();
                let start = usize::from(mode == 1 || mode == 2);
                let opcode = ((u32::from(bytes[start]) & 7) << 8) | u32::from(bytes[start + 1]);
                c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp={};{code}++completed;}}\nstatic void physical_{i}(const unsigned char *ptr){{__asm__ volatile(\".byte {first},{}\"::\"a\"(ptr):\"memory\");}}\n",if mode==4{"1025u"}else{"0xabcdef12u"},field<<3));
                forms.push((opcode, width, mode, kind));
            }
        }
    }
    c.push_str("static void (*const generated[144])(void)={");
    for i in 0..144 {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic void (*const physical[144])(const unsigned char*)={");
    for i in 0..144 {
        c.push_str(&format!("physical_{i},"));
    }
    c.push_str("};\n");
    for (which, name) in ["opcodes", "widths", "modes", "kinds"].iter().enumerate() {
        c.push_str(&format!("static const unsigned {name}[144]={{"));
        for &(a, b, d, e) in &forms {
            let v = [a, b, d, e][which];
            c.push_str(&format!("{v},"));
        }
        c.push_str("};\n");
    }
    c
}
#[test]
fn export_x87_memory_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_MEMORY_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn rejects_invalid_memory_forms() {
    for bytes in [
        &[0x66, 0xd8, 6][..],
        &[0xf0, 0xd8, 6],
        &[0x65, 0xd8, 6],
        &[0xdf, 0x08], // FISTTP is outside the current CPU profile.
    ] {
        assert!(emit(bytes).is_err(), "{bytes:x?}");
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xdc, 6], 0x10000, &[]).unwrap();
    let original = &x.instructions()[0];
    for width in [1, 2, 4, 10, 16] {
        let mut bad = original.clone();
        bad.operands[0].mem_size = width;
        assert!(instruction(&bad).is_err());
    }
    let mut bad = original.clone();
    bad.operands[0].expr_src = Some("unsafe()".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.operands[0].mem_seg = Some("gs".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.mnemonic = "fsub".into();
    assert!(instruction(&bad).is_err());
    let mut cosmetic = original.clone();
    cosmetic.op_str = "unsafe()".into();
    assert_eq!(
        instruction(original).unwrap(),
        instruction(&cosmetic).unwrap()
    );
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_memory_arithmetic_matches_hardware() {
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
        "gamepkg_x86_memory.c",
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
