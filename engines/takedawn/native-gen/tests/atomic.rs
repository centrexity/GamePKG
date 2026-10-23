use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn decode(bytes: &[u8]) -> gamepkg_takedawn_native_gen::decode::Instruction {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(bytes, 0x10000, &[]).unwrap();
    assert_eq!(x.instructions().len(), 1, "{bytes:x?}");
    assert_eq!(usize::from(x.instructions()[0].size), bytes.len());
    x.instructions()[0].clone()
}
fn forms() -> Vec<(u32, u32, u32, Vec<u8>)> {
    let mut forms = vec![];
    for kind in 0..3 {
        for bytes in [1, 2, 4] {
            for mode in 0..6 {
                let mut b = vec![];
                if bytes == 2 {
                    b.push(0x66);
                }
                if mode == 2 {
                    b.push(0x64);
                }
                if mode == 3 {
                    b.push(0x67);
                }
                if mode == 4 {
                    b.push(0xf0);
                }
                if kind == 0 {
                    b.push(if bytes == 1 { 0x86 } else { 0x87 });
                } else {
                    b.extend_from_slice(&[
                        0x0f,
                        if kind == 1 { 0xc0 } else { 0xb0 } + u8::from(bytes != 1),
                    ]);
                }
                // source CL/CX/ECX. Dest EBX/BX/BL, [EDI], FS:[EDI], [BX+SI],
                // LOCK [EDI], and register source/destination identity.
                b.push(match mode {
                    0 => 0xcb,
                    1 | 2 | 4 => 0x0f,
                    3 => 0x08,
                    _ => 0xc9,
                });
                forms.push((kind, bytes, mode, b));
            }
        }
    }
    // Accumulator short forms, high subregister aliases, SIB and displacements.
    for b in [
        vec![0x91],
        vec![0x66, 0x95],
        vec![0x86, 0xe0],
        vec![0x0f, 0xc0, 0xe0],
        vec![0x0f, 0xb0, 0xe0],
        vec![0x87, 0x0c, 0x24],
        vec![0x87, 0x4c, 0x77, 0xff],
        vec![0x67, 0x87, 0x0e, 0x34, 0x12],
    ] {
        let i = decode(&b);
        assert!(
            instruction(&i).is_ok(),
            "{} {}: {:?}",
            i.mnemonic,
            i.op_str,
            instruction(&i)
        );
    }
    for (kind, bytes, raw) in [
        (0, 1, vec![0x86, 0xe0]),
        (1, 1, vec![0x0f, 0xc0, 0xe0]),
        (2, 1, vec![0x0f, 0xb0, 0xe0]),
        (0, 4, vec![0x91]),
        (0, 2, vec![0x66, 0x91]),
    ] {
        forms.push((kind, bytes, 6, raw));
    }
    forms
}
fn fixture() -> String {
    let mut c = String::from("#include <stdint.h>\n");
    if let Some(h) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let h = h.to_str().unwrap();
        assert!(!h.contains(['\"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{h}\"\n"));
    }
    c.push_str(&format!(
        "#include \"{}/tests/atomic_inputs.h\"\n",
        env!("CARGO_MANIFEST_DIR")
    ));
    c.push_str(include_str!("atomic_fixture.c"));
    let mut w = String::new();
    let mut names = vec![];
    let mut metadata = vec![];
    for (k, b, m, raw) in forms() {
        let i = decode(&raw);
        let code = instruction(&i).unwrap_or_else(|e| panic!("{} {} {e}", i.mnemonic, i.op_str));
        let n = format!("op_{}", names.len());
        w.push_str(&format!(
            "static void {n}(void){{ uint32_t ebp=g_ebp; (void)ebp; {code} g_ebp=ebp; ++completed; }}\n"
        ));
        names.push(n);
        metadata.push(format!("{{{k},{b},{m}}}"));
    }
    c.replace("/* WRAPPERS */",&format!("{w}\nstatic void (*instructions[])(void)={{{}}};\nstatic const unsigned forms[][3]={{{}}};",names.join(","),metadata.join(",")))
}
#[test]
fn rejects_exchange_prefix_encoding_and_operand_mismatches() {
    for b in [&[0x0f, 0xc1, 0x0f][..], &[0x0f, 0xb1, 0xcb], &[0x87, 0x0f]] {
        let mut i = decode(b);
        i.bytes_hex = "0fc90f".into();
        assert!(instruction(&i).is_err());
        let mut i = decode(b);
        i.operands[1].reg = Some("edx".into());
        assert!(instruction(&i).is_err());
        let mut i = decode(b);
        i.mnemonic = format!("lock {}", i.mnemonic);
        assert!(instruction(&i).is_err());
        let mut i = decode(b);
        i.operands[0].expr = Some("unsafe".into());
        assert!(instruction(&i).is_err());
    }
    let mut i = decode(&[0x87, 0x0f]);
    i.operands[0].mem_base = Some("esi".into());
    assert!(instruction(&i).is_err());
    let mut i = decode(&[0x87, 0x0f]);
    i.bytes_hex = "6666870f".into();
    i.size = 4;
    assert!(instruction(&i).is_err());
}
#[test]
fn export_atomic_fixture() {
    let c = fixture();
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_ATOMIC_VARIANTS") {
        let mut v = String::new();
        for (i, (_, _, _, b)) in forms().iter().enumerate() {
            v.push_str(&format!(
                "case {i}: EXECUTE(\".byte {}\");break;\n",
                b.iter()
                    .map(|v| format!("0x{v:02x}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        std::fs::write(p, v).unwrap();
    }
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_ATOMIC_FIXTURE") {
        std::fs::write(p, c).unwrap();
    }
}
#[cfg(target_os = "linux")]
#[test]
fn exchanges_match_results_faults_and_shared_worker_updates() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("atomic.c");
    let binary = d.path().join("atomic");
    std::fs::write(&source, fixture()).unwrap();
    let r = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let out = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pthread",
            "-fsanitize=undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg("-I")
        .arg(&r)
        .arg(source)
        .arg(r.join("gamepkg_x86_memory.c"))
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
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
