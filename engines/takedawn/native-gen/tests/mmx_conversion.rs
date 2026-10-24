use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete mixed conversion"
    );
    instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = include_str!("mmx_conversion_oracle.c").to_string();
    let mut forms = vec![];
    for (op, (name, opcode)) in [("cvtpi2ps", 0x2a), ("cvtps2pi", 0x2d), ("cvttps2pi", 0x2c)]
        .iter()
        .enumerate()
    {
        for dst in 0..8u8 {
            for mode in 0..4u8 {
                let src = (dst + 1) & 7;
                let mut bytes = vec![0x0f, *opcode];
                if mode == 2 {
                    bytes.insert(0, 0x64);
                }
                if mode == 3 {
                    bytes.insert(0, 0x67);
                }
                bytes.push(if mode == 0 {
                    0xc0 | (dst << 3) | src
                } else if mode == 3 {
                    dst << 3
                } else {
                    (dst << 3) | 6
                });
                let source = if mode == 0 {
                    format!("%%{}{}", if op == 0 { "mm" } else { "xmm" }, src)
                } else {
                    "(%0)".into()
                };
                let target = format!("{}{}", if op == 0 { "xmm" } else { "mm" }, dst);
                let i = forms.len();
                c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp=0xabcdef12;{}++completed;}}\nstatic void hardware_{i}(const void*ptr){{(void)ptr;__asm__ volatile(\"{name} {source},%%{target}\"::\"r\"(ptr):\"{target}\",\"memory\");}}\n",emit(&bytes).unwrap_or_else(|e|panic!("{bytes:x?}: {e}"))));
                forms.push((op, mode, dst, src));
            }
        }
    }
    assert_eq!(forms.len(), 96);
    c.push_str("static void (*const generated[96])(void)={");
    for i in 0..96 {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic void (*const physical[96])(const void*)={");
    for i in 0..96 {
        c.push_str(&format!("hardware_{i},"));
    }
    c.push_str("};\n");
    for (name, index) in [
        ("kinds", 0),
        ("modes", 1),
        ("destinations", 2),
        ("sources", 3),
    ] {
        c.push_str(&format!("static const unsigned {name}[96]={{"));
        for &(o, m, d, s) in &forms {
            c.push_str(&format!(
                "{},",
                [o, m as usize, d as usize, s as usize][index]
            ));
        }
        c.push_str("};\n");
    }
    c
}
#[test]
fn mixed_conversion_rejects_wrong_forms_and_metadata() {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for opcode in [0x2a, 0x2d, 0x2c] {
        for prefix in [0x66, 0xf2, 0xf0, 0x65] {
            assert!(emit(&[prefix, 0x0f, opcode, 6]).is_err());
        }
        let x = d.linear(&[0x0f, opcode, 6], 0x10000, &[]).unwrap();
        for width in [1, 2, 4, 10, 16] {
            let mut bad = x.instructions()[0].clone();
            bad.operands[1].mem_size = width;
            assert!(instruction(&bad).is_err());
        }
        let x = d.linear(&[0x0f, opcode, 0xc1], 0x10000, &[]).unwrap();
        for index in 0..2 {
            let mut bad = x.instructions()[0].clone();
            bad.operands[index].reg = Some("mm8;untrusted()".into());
            assert!(instruction(&bad).is_err());
            let mut bad = x.instructions()[0].clone();
            bad.operands[index].expr_src = Some("untrusted()".into());
            assert!(instruction(&bad).is_err());
        }
        let mut bad = x.instructions()[0].clone();
        bad.mnemonic = if opcode == 0x2a {
            "cvtps2pi"
        } else {
            "cvtpi2ps"
        }
        .into();
        assert!(instruction(&bad).is_err());
    }
}
#[test]
fn export_mixed_conversion_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_MMX_CONVERSION_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_mixed_conversions_match_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("mixed.c");
    let binary = dir.path().join("mixed");
    std::fs::write(&source, fixture()).unwrap();
    let r = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let out = std::process::Command::new("cc")
        .args([
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
        .arg(&r)
        .arg(source)
        .arg(r.join("gamepkg_x86_mmx_convert.c"))
        .arg(r.join("gamepkg_x86_sse.c"))
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
