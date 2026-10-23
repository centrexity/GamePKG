use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete x87 load"
    );
    instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = include_str!("x87_load_oracle.c").to_string();
    let memories = [
        (0xd9, 0, 4, "flds"),
        (0xdd, 0, 8, "fldl"),
        (0xdb, 5, 10, "fldt"),
        (0xdf, 0, 2, "filds"),
        (0xdb, 0, 4, "fildl"),
        (0xdf, 5, 8, "fildll"),
    ];
    let mut forms = vec![];
    for (kind, (first, field, width, name)) in memories.iter().enumerate() {
        for mode in 0..6 {
            let mut bytes = match mode {
                0 => vec![*first, field << 3 | 6],
                1 => vec![0x64, *first, field << 3 | 6],
                2 => vec![0x67, *first, field << 3],
                3 => vec![*first, 0x84 | (field << 3), 0xfe, 0x20, 0, 0, 0],
                4 => vec![*first, 0x45 | (field << 3), 0],
                _ => vec![*first, 0x05 | (field << 3), 1, 4, 0, 0],
            };
            let code = emit(&bytes).unwrap();
            let i = forms.len();
            let start = if mode == 1 || mode == 2 { 1 } else { 0 };
            let opcode = ((u32::from(bytes[start]) & 7) << 8) | u32::from(bytes[start + 1]);
            c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp={};{code}++completed;}}\nstatic void physical_{i}(const unsigned char *ptr){{__asm__ volatile(\"{name} (%0)\"::\"r\"(ptr):\"memory\");}}\n",if mode==4{"1025u"}else{"0xabcdef12u"}));
            forms.push((opcode, *width, mode, kind));
            bytes.clear();
        }
    }
    for index in 0..8u8 {
        let i = forms.len();
        let bytes = [0xd9, 0xc0 + index];
        let code = emit(&bytes).unwrap();
        c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp=0xabcdef12;{code}++completed;}}\nstatic void physical_{i}(const unsigned char *ptr){{(void)ptr;__asm__ volatile(\".byte 217,{}\"::: \"memory\");}}\n",bytes[1]));
        forms.push((0x1c0 + u32::from(index), 0, 0, 6));
    }
    for index in 0..7u8 {
        let i = forms.len();
        let bytes = [0xd9, 0xe8 + index];
        let code = emit(&bytes).unwrap();
        c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp=0xabcdef12;{code}++completed;}}\nstatic void physical_{i}(const unsigned char *ptr){{(void)ptr;__asm__ volatile(\".byte 217,{}\"::: \"memory\");}}\n",bytes[1]));
        forms.push((0x1e8 + u32::from(index), 0, 0, 7));
    }
    c.push_str("static void (*const generated[51])(void)={");
    for i in 0..51 {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic void (*const physical[51])(const unsigned char*)={");
    for i in 0..51 {
        c.push_str(&format!("physical_{i},"));
    }
    c.push_str("};\nstatic const unsigned opcodes[51]={");
    for (op, _, _, _) in &forms {
        c.push_str(&format!("{op},"));
    }
    c.push_str("};\nstatic const unsigned widths[51]={");
    for (_, w, _, _) in &forms {
        c.push_str(&format!("{w},"));
    }
    c.push_str("};\nstatic const unsigned modes[51]={");
    for (_, _, m, _) in &forms {
        c.push_str(&format!("{m},"));
    }
    c.push_str("};\nstatic const unsigned kinds[51]={");
    for (_, _, _, k) in &forms {
        c.push_str(&format!("{k},"));
    }
    c.push_str("};\n");
    c
}
#[test]
fn export_x87_load_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_LOAD_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn load_rejects_wrong_forms_and_untrusted_metadata() {
    for bytes in [
        &[0x66, 0xd9, 0x06][..],
        &[0xf0, 0xd9, 0x06],
        &[0x65, 0xd9, 0x06],
        &[0xdf, 0x08],
        &[0xdb, 0x36],
    ] {
        assert!(emit(bytes).is_err(), "{bytes:x?}");
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xdd, 0x06], 0x10000, &[]).unwrap();
    let original = &x.instructions()[0];
    for width in [1, 2, 4, 10, 16] {
        let mut bad = original.clone();
        bad.operands[0].mem_size = width;
        assert!(instruction(&bad).is_err());
    }
    let mut bad = original.clone();
    bad.operands[0].expr = Some("unsafe()".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.mnemonic = "fild".into();
    assert!(instruction(&bad).is_err());
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn generated_x87_loads_match_hardware_stack_and_exceptions() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("loads.c");
    let binary = d.path().join("loads");
    std::fs::write(&source, fixture()).unwrap();
    let r = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let out = std::process::Command::new("cc")
        .args([
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
        .arg(&r)
        .arg(source)
        .arg(r.join("gamepkg_x86_x87_load.c"))
        .arg(r.join("gamepkg_x86_x87.c"))
        .arg(r.join("gamepkg_x86_fault.c"))
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
