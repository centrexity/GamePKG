use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete x87 store"
    );
    instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = include_str!("x87_store_oracle.c").to_string();
    let mut forms = vec![];
    for (kind, first, field, width) in [
        (0u32, 0xd9u8, 2u8, 4u32),
        (0, 0xd9, 3, 4),
        (1, 0xdd, 2, 8),
        (1, 0xdd, 3, 8),
        (2, 0xdb, 7, 10),
        (3, 0xdf, 2, 2),
        (3, 0xdf, 3, 2),
        (4, 0xdb, 2, 4),
        (4, 0xdb, 3, 4),
        (5, 0xdf, 7, 8),
    ] {
        for mode in 0..6 {
            let bytes = match mode {
                0 => vec![first, field << 3 | 6],
                1 => vec![0x64, first, field << 3 | 6],
                2 => vec![0x67, first, field << 3],
                3 => vec![first, 0x84 | (field << 3), 0xfe, 0x20, 0, 0, 0],
                4 => vec![first, 0x45 | (field << 3), 0],
                _ => vec![first, 0x05 | (field << 3), 1, 4, 0, 0],
            };
            let start = usize::from(mode == 1 || mode == 2);
            let opcode = ((u32::from(first) & 7) << 8) | u32::from(bytes[start + 1]);
            forms.push((bytes, first, field << 3, opcode, width, mode, kind, 0));
        }
    }
    for pop in 0..2u8 {
        for dst in 0..8 {
            let second = 0xd0 + pop * 8 + dst;
            forms.push((
                vec![0xdd, second],
                0xdd,
                second,
                0x500 | u32::from(second),
                0,
                0,
                6,
                u32::from(dst),
            ));
        }
    }
    assert_eq!(forms.len(), 76);
    for (i, (bytes, first, second, _, _, mode, _, _)) in forms.iter().enumerate() {
        let code = emit(bytes).unwrap();
        c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp={};{code}++completed;}}\nstatic void physical_{i}(unsigned char *ptr){{__asm__ volatile(\".byte {first},{second}\"::\"a\"(ptr):\"memory\");}}\n",if *mode==4{"1025u"}else{"0xabcdef12u"}));
    }
    c.push_str("static void (*const generated[76])(void)={");
    for i in 0..76 {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic void (*const physical[76])(unsigned char*)={");
    for i in 0..76 {
        c.push_str(&format!("physical_{i},"));
    }
    c.push_str("};\n");
    for (which, name) in ["opcodes", "widths", "modes", "kinds", "destinations"]
        .iter()
        .enumerate()
    {
        c.push_str(&format!("static const unsigned {name}[76]={{"));
        for (_, _, _, op, width, mode, kind, dst) in &forms {
            let v = [op, width, mode, kind, dst][which];
            c.push_str(&format!("{v},"));
        }
        c.push_str("};\n");
    }
    c
}
#[test]
fn export_store_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_STORE_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn stores_reject_unqualified_forms() {
    for b in [
        &[0x66, 0xd9, 0x16][..],
        &[0x67, 0xdd, 0xd1],
        &[0xf0, 0xdb, 0x3e],
        &[0x65, 0xdd, 0x16],
        &[0xdb, 0x0e],
    ] {
        assert!(emit(b).is_err(), "{b:x?}");
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xdd, 0x16], 0x10000, &[]).unwrap();
    let original = &x.instructions()[0];
    for width in [1, 2, 4, 10, 16] {
        let mut bad = original.clone();
        bad.operands[0].mem_size = width;
        assert!(instruction(&bad).is_err());
    }
    let mut bad = original.clone();
    bad.operands[0].mem_seg = Some("gs".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.operands[0].expr_src = Some("unsafe()".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.mnemonic = "fstp".into();
    assert!(instruction(&bad).is_err());
    let mut cosmetic = original.clone();
    cosmetic.op_str = "unsafe()".into();
    assert_eq!(
        instruction(original).unwrap(),
        instruction(&cosmetic).unwrap()
    );
    let x = d.linear(&[0xdd, 0xd1], 0x10000, &[]).unwrap();
    let mut bad = x.instructions()[0].clone();
    bad.operands[0].reg = Some("st(7)".into());
    assert!(instruction(&bad).is_err());
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_stores_match_hardware() {
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
        "gamepkg_x86_x87_store.c",
        "gamepkg_x86_x87.c",
        "gamepkg_x86_fault.c",
        "gamepkg_x86_memory.c",
    ] {
        cmd.arg(runtime.join(s));
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
