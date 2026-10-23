use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
const OPS: [(&str, u8, u8, u8); 8] = [
    ("psllw", 0xf1, 0x71, 6),
    ("pslld", 0xf2, 0x72, 6),
    ("psllq", 0xf3, 0x73, 6),
    ("psrlw", 0xd1, 0x71, 2),
    ("psrld", 0xd2, 0x72, 2),
    ("psrlq", 0xd3, 0x73, 2),
    ("psraw", 0xe1, 0x71, 4),
    ("psrad", 0xe2, 0x72, 4),
];
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete shift"
    );
    instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = include_str!("mmx_shift_oracle.c").to_string();
    let mut forms = vec![];
    for (name, opcode, _, _) in OPS {
        for dst in 0..8u8 {
            for mode in 0..5u8 {
                let src = if mode == 1 { dst } else { (dst + 1) & 7 };
                let bytes = match mode {
                    0 | 1 => vec![0x0f, opcode, 0xc0 | (dst << 3) | src],
                    2 => vec![0x0f, opcode, (dst << 3) | 6],
                    3 => vec![0x64, 0x0f, opcode, (dst << 3) | 6],
                    _ => vec![0x67, 0x0f, opcode, dst << 3],
                };
                let i = forms.len();
                let source = if mode < 2 {
                    format!("%%mm{src}")
                } else {
                    "(%0)".into()
                };
                c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp=0xabcdef12;{}++completed;}}\nstatic void hardware_{i}(const void *ptr){{(void)ptr;__asm__ volatile(\"{name} {source},%%mm{dst}\"::\"r\"(ptr):\"mm{dst}\",\"memory\");}}\n",emit(&bytes).unwrap()));
                forms.push((mode, dst, src));
            }
        }
    }
    for (name, _, opcode, field) in OPS {
        for count in 0..256u32 {
            let dst = (count & 7) as u8;
            let bytes = [0x0f, opcode, 0xc0 | (field << 3) | dst, count as u8];
            let i = forms.len();
            c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp=0xabcdef12;{}++completed;}}\nstatic void hardware_{i}(const void *ptr){{(void)ptr;__asm__ volatile(\"{name} ${count},%%mm{dst}\"::: \"mm{dst}\",\"memory\");}}\n",emit(&bytes).unwrap()));
            forms.push((5, dst, 0));
        }
    }
    assert_eq!(forms.len(), 2368);
    c.push_str("static void (*const generated[@FORMS@])(void)={");
    for i in 0..forms.len() {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic void (*const physical[@FORMS@])(const void*)={");
    for i in 0..forms.len() {
        c.push_str(&format!("hardware_{i},"));
    }
    c.push_str("};\n");
    for (name, field) in [("modes", 0), ("destinations", 1), ("sources", 2)] {
        c.push_str(&format!("static const unsigned {name}[@FORMS@]={{"));
        for &(m, d, s) in &forms {
            c.push_str(&format!("{},", [m, d, s][field]));
        }
        c.push_str("};\n");
    }
    c.replace("@FORMS@", &forms.len().to_string())
}
#[test]
fn shift_rejects_invalid_forms_prefixes_and_metadata() {
    for (_, variable, immediate, field) in OPS {
        for bytes in [
            vec![0x66, 0x0f, variable, 0xc1],
            vec![0xf0, 0x0f, variable, 6],
            vec![0xf3, 0x0f, variable, 0xc1],
            vec![0x65, 0x0f, variable, 6],
        ] {
            assert!(emit(&bytes).is_err());
        }
        let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
        let bytes = [0x0f, immediate, 0xc0 | (field << 3), 17];
        let x = d.linear(&bytes, 0x10000, &[]).unwrap();
        let original = &x.instructions()[0];
        for count in [0, 16, 256, u32::MAX] {
            let mut bad = original.clone();
            bad.operands[1].imm = Some(count);
            assert!(instruction(&bad).is_err());
        }
        for reg in ["mm1", "xmm0", "mm0;untrusted()"] {
            let mut bad = original.clone();
            bad.operands[0].reg = Some(reg.into());
            assert!(instruction(&bad).is_err());
        }
        let mut bad = original.clone();
        bad.operands[1].expr = Some("untrusted()".into());
        assert!(instruction(&bad).is_err());
        let x = d.linear(&[0x0f, variable, 6], 0x10000, &[]).unwrap();
        for width in [1, 2, 4, 16] {
            let mut bad = x.instructions()[0].clone();
            bad.operands[1].mem_size = width;
            assert!(instruction(&bad).is_err());
        }
    }
}
#[test]
fn export_mmx_shift_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_MMX_SHIFT_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_shifts_match_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("shifts.c");
    let binary = dir.path().join("shifts");
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
