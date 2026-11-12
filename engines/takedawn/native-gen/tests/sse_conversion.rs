use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let decoded = decoder.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        decoded.instructions().len() == 1
            && usize::from(decoded.instructions()[0].size) == bytes.len(),
        "incomplete conversion"
    );
    instruction(&decoded.instructions()[0])
}
#[test]
fn conversion_rejects_invalid_prefixes_widths_and_synthetic_operands() {
    for bytes in [
        &[0x66, 0x0f, 0x2a, 0xc0][..],
        &[0x66, 0xf3, 0x0f, 0x2a, 0xc0],
        &[0xf2, 0x0f, 0x2a, 0xc0],
        &[0xf0, 0xf3, 0x0f, 0x2a, 0],
        &[0x65, 0xf3, 0x0f, 0x2a, 0],
    ] {
        assert!(emit(bytes).is_err(), "{bytes:x?}");
    }
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for bytes in [
        &[0xf3, 0x0f, 0x2a, 0xc0][..],
        &[0xf3, 0x0f, 0x2d, 0xc1],
        &[0xf3, 0x0f, 0x2c, 0x06],
    ] {
        let decoded = decoder.linear(bytes, 0x10000, &[]).unwrap();
        let original = decoded.instructions()[0].clone();
        for index in 0..2 {
            let mut bad = original.clone();
            bad.operands[index].expr = Some("unsafe()".into());
            assert!(instruction(&bad).is_err());
        }
        let mut bad = original.clone();
        bad.operands[0].reg = Some("xmm8".into());
        assert!(instruction(&bad).is_err());
        let mut bad = original.clone();
        bad.operands[1].mem_size = 8;
        if bad.operands[1].kind == "mem" {
            assert!(instruction(&bad).is_err());
        }
        let mut bad = original;
        bad.operands[1].reg = Some("ax".into());
        bad.operands[1].kind = "reg";
        assert!(instruction(&bad).is_err());
    }
}
fn fixture() -> String {
    let mut c = include_str!("conversion_oracle.c").to_owned();
    for op in 0..3 {
        for mode in 0..6 {
            let opcode = [0x2a, 0x2d, 0x2c][op];
            let mut bytes = vec![0xf3, 0x0f, opcode];
            match mode {
                0 => bytes.push(if op == 0 { 0xc0 } else { 0xc1 }),
                1 => bytes.push(0x06),
                2 => bytes.push(if op == 0 { 0xc5 } else { 0xe9 }),
                3 => bytes.push(if op == 0 { 0xc4 } else { 0xe1 }),
                4 => {
                    bytes.insert(0, 0x64);
                    bytes.push(0x06);
                }
                _ => {
                    bytes.insert(0, 0x67);
                    bytes.push(0x00);
                }
            }
            c.push_str(&format!("static void emitted_{op}_{mode}(void) {{uint32_t ebp=local_ebp; {} observed_ebp=ebp;++completed;}}\n",emit(&bytes).unwrap()));
        }
    }
    c.push_str("static void (*selected(unsigned op,unsigned mode))(void) {\n");
    for op in 0..3 {
        for mode in 0..6 {
            c.push_str(&format!(
                "if(op=={op} && mode=={mode})return emitted_{op}_{mode};\n"
            ));
        }
    }
    c.push_str("abort();}\n");
    c
}
#[test]
fn export_conversion_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_CONVERSION_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn conversion_matches_hardware_and_staged_faults() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("conversion.c");
    let binary = dir.path().join("conversion");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let out = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O3",
            "-mno-red-zone",
            "-msse2",
            "-pthread",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-fsanitize=undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg("-I")
        .arg(&runtime)
        .arg(&source)
        .arg(runtime.join("gamepkg_x86_fault.c"))
        .arg(runtime.join("gamepkg_x86_sse.c"))
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
