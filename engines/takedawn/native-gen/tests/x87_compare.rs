use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete comparison fixture"
    );
    instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = include_str!("x87_compare_oracle.c").to_string();
    let mut forms = vec![];
    for (kind, first, width) in [
        (2u32, 0xd8u8, 4u32),
        (3, 0xdc, 8),
        (4, 0xde, 2),
        (5, 0xda, 4),
    ] {
        for field in [2u8, 3] {
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
                let second = bytes[start + 1];
                forms.push((
                    bytes,
                    first,
                    field << 3,
                    ((u32::from(first) & 7) << 8) | u32::from(second),
                    width,
                    mode,
                    kind,
                    0,
                    0,
                ));
            }
        }
    }
    for (first, base, flags) in [
        (0xd8u8, 0xd0u8, 0u32),
        (0xd8, 0xd8, 0),
        (0xdd, 0xe0, 0),
        (0xdd, 0xe8, 0),
        (0xdb, 0xf0, 1),
        (0xdf, 0xf0, 1),
        (0xdb, 0xe8, 1),
        (0xdf, 0xe8, 1),
    ] {
        for index in 0..8 {
            let second = base + index;
            forms.push((
                vec![first, second],
                first,
                second,
                ((u32::from(first) & 7) << 8) | u32::from(second),
                0,
                0,
                0,
                u32::from(index),
                flags,
            ));
        }
    }
    for (first, second, kind, src) in [
        (0xdeu8, 0xd9u8, 0u32, 1u32),
        (0xda, 0xe9, 0, 1),
        (0xd9, 0xe4, 1, 0),
    ] {
        forms.push((
            vec![first, second],
            first,
            second,
            ((u32::from(first) & 7) << 8) | u32::from(second),
            0,
            0,
            kind,
            src,
            0,
        ));
    }
    assert_eq!(forms.len(), 115);
    for (i, (bytes, first, second, _, _, mode, _, _, _)) in forms.iter().enumerate() {
        let code = emit(bytes).unwrap();
        c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp={};{code}++completed;}}\nstatic uint32_t physical_{i}(const unsigned char *ptr,uint32_t flags){{uintptr_t out;__asm__ volatile(\"push %2; popf; .byte {first},{second}; pushf; pop %0\":\"=r\"(out):\"a\"(ptr),\"r\"((uintptr_t)flags):\"memory\",\"cc\");return (uint32_t)out;}}\n",if *mode==4{"1025u"}else{"0xabcdef12u"}));
    }
    c.push_str("static void (*const generated[115])(void)={");
    for i in 0..115 {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic uint32_t (*const physical[115])(const unsigned char*,uint32_t)={");
    for i in 0..115 {
        c.push_str(&format!("physical_{i},"));
    }
    c.push_str("};\n");
    for (which, name) in [
        "opcodes",
        "widths",
        "modes",
        "kinds",
        "sources",
        "integer_flags",
    ]
    .iter()
    .enumerate()
    {
        c.push_str(&format!("static const unsigned {name}[115]={{"));
        for (_, _, _, op, width, mode, kind, src, flags) in &forms {
            let v = [op, width, mode, kind, src, flags][which];
            c.push_str(&format!("{v},"));
        }
        c.push_str("};\n");
    }
    c
}
#[test]
fn export_comparison_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_COMPARE_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn rejects_unqualified_comparisons() {
    for b in [
        &[0x66, 0xd8, 0xd1][..],
        &[0x67, 0xd8, 0xd1],
        &[0x64, 0xd8, 0xd1],
        &[0xf0, 0xdb, 0xf1],
        &[0x65, 0xdc, 0x16],
    ] {
        assert!(emit(b).is_err(), "{b:x?}");
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xdb, 0xf1], 0x10000, &[]).unwrap();
    let original = &x.instructions()[0];
    let mut bad = original.clone();
    bad.operands[0].reg = Some("st(7)".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.operands[0].expr = Some("unsafe()".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.mnemonic = "fucomi".into();
    assert!(instruction(&bad).is_err());
    let x = d.linear(&[0xdc, 0x16], 0x10000, &[]).unwrap();
    let original = &x.instructions()[0];
    for width in [1, 2, 4, 10, 16] {
        let mut bad = original.clone();
        bad.operands[0].mem_size = width;
        assert!(instruction(&bad).is_err());
    }
    let mut bad = original.clone();
    bad.operands[0].mem_seg = Some("gs".into());
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
fn generated_comparisons_match_hardware() {
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
        "gamepkg_x86_x87_compare.c",
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
