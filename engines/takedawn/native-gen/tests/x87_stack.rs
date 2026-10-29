use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete stack fixture"
    );
    instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = include_str!("x87_stack_oracle.c").to_string();
    let mut forms = vec![];
    for (op, second) in [
        (0u32, 0xe1u8),
        (1, 0xe0),
        (4, 0xf7),
        (5, 0xf6),
        (6, 0xe5),
        (7, 0xd0),
        (8, 0xfc),
        (9, 0xf4),
    ] {
        forms.push((0xd9u8, second, op, 0u32));
    }
    for (op, first, base) in [(2, 0xd9, 0xc8), (3, 0xdd, 0xc0)] {
        for index in 0..8u8 {
            forms.push((first, base + index, op, u32::from(index)));
        }
    }
    for selector in 0..8u8 {
        for index in 0..8u8 {
            forms.push((
                if selector < 4 { 0xda } else { 0xdb },
                0xc0 + (selector & 3) * 8 + index,
                10 + u32::from(selector),
                u32::from(index),
            ));
        }
    }
    assert_eq!(forms.len(), 88);
    for (i, (first, second, _, _)) in forms.iter().enumerate() {
        let code = emit(&[*first, *second]).unwrap();
        c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp=0xabcdef12;{code}++completed;}}\nstatic uint32_t physical_{i}(uint32_t flags){{uintptr_t out;__asm__ volatile(\"push %1; popf; .byte {first},{second}; pushf; pop %0\":\"=r\"(out):\"r\"((uintptr_t)flags):\"memory\",\"cc\");return (uint32_t)out;}}\n"));
    }
    c.push_str("static void (*const generated[88])(void)={");
    for i in 0..88 {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic uint32_t (*const physical[88])(uint32_t)={");
    for i in 0..88 {
        c.push_str(&format!("physical_{i},"));
    }
    c.push_str("};\n");
    for (which, name) in ["opcodes", "operations", "indices"].iter().enumerate() {
        c.push_str(&format!("static const unsigned {name}[88]={{"));
        for (first, second, op, index) in &forms {
            let v = [
                ((u32::from(*first) & 7) << 8) | u32::from(*second),
                *op,
                *index,
            ][which];
            c.push_str(&format!("{v},"));
        }
        c.push_str("};\n");
    }
    c
}
#[test]
fn export_stack_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_STACK_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn stack_rejects_unqualified_operands() {
    for b in [
        &[0x66, 0xd9, 0xe1][..],
        &[0x67, 0xd9, 0xc9],
        &[0x64, 0xdd, 0xc1],
        &[0xf0, 0xda, 0xc1],
    ] {
        assert!(emit(b).is_err(), "{b:x?}");
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for bytes in [[0xda, 0xc1], [0xd9, 0xc9]] {
        let x = d.linear(&bytes, 0x10000, &[]).unwrap();
        let original = &x.instructions()[0];
        let mut bad = original.clone();
        bad.operands[0].reg = Some("st(2)".into());
        assert!(instruction(&bad).is_err());
        let mut bad = original.clone();
        bad.operands[1].reg = Some("st(7)".into());
        assert!(instruction(&bad).is_err());
        let mut bad = original.clone();
        bad.operands[0].expr_src = Some("unsafe()".into());
        assert!(instruction(&bad).is_err());
        let mut bad = original.clone();
        bad.mnemonic = "ffree".into();
        assert!(instruction(&bad).is_err());
        let mut cosmetic = original.clone();
        cosmetic.op_str = "unsafe()".into();
        assert_eq!(
            instruction(original).unwrap(),
            instruction(&cosmetic).unwrap()
        );
    }
    let x = d.linear(&[0xdd, 0xc1], 0x10000, &[]).unwrap();
    let mut bad = x.instructions()[0].clone();
    bad.operands[0].kind = "mem";
    assert!(instruction(&bad).is_err());
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_stack_operations_match_hardware() {
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
        "gamepkg_x86_x87_stack.c",
        "gamepkg_x86_x87.c",
        "gamepkg_x86_fault.c",
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
