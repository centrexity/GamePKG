use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
const BINARY: [(&str, u8); 7] = [
    ("pavgb", 0xe0),
    ("pavgw", 0xe3),
    ("pminub", 0xda),
    ("pmaxub", 0xde),
    ("pminsw", 0xea),
    ("pmaxsw", 0xee),
    ("psadbw", 0xf6),
];
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete lane instruction"
    );
    instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = include_str!("mmx_lanes_oracle.c").to_string();
    let mut forms = vec![];
    let mut add = |bytes: Vec<u8>,
                   assembly: String,
                   op: u8,
                   mode: u8,
                   dst: u8,
                   src: u8,
                   control: u8,
                   out: bool| {
        let i = forms.len();
        let suffix = if out {
            "return result;"
        } else {
            "return value;"
        };
        let asm = if out {
            format!("uint32_t result;__asm__ volatile(\"{assembly}\":\"=r\"(result)::\"memory\");")
        } else {
            format!(
                "__asm__ volatile(\"{assembly}\"::\"r\"(ptr),\"r\"(value):\"mm{dst}\",\"memory\");"
            )
        };
        c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp=0xabcdef12;{}observed_ebp=ebp;++completed;}}\nstatic uint32_t hardware_{i}(const void *ptr,uint32_t value){{(void)ptr;(void)value;{asm}{suffix}}}\n",emit(&bytes).unwrap_or_else(|e|panic!("{bytes:x?}: {e}"))));
        forms.push((op, mode, dst, src, control));
    };
    for (op, (name, opcode)) in BINARY.iter().enumerate() {
        for dst in 0..8u8 {
            for mode in 0..3u8 {
                let src = if mode == 1 { dst } else { (dst + 1) & 7 };
                let byte = if mode == 2 {
                    (dst << 3) | 6
                } else {
                    0xc0 | (dst << 3) | src
                };
                let source = if mode == 2 {
                    "(%0)".into()
                } else {
                    format!("%%mm{src}")
                };
                add(
                    vec![0x0f, *opcode, byte],
                    format!("{name} {source},%%mm{dst}"),
                    op as u8,
                    mode,
                    dst,
                    src,
                    0,
                    false,
                );
            }
        }
    }
    for control in 0..256u32 {
        let dst = (control & 7) as u8;
        for mode in 0..3u8 {
            let src = if mode == 1 { dst } else { (dst + 1) & 7 };
            let byte = if mode == 2 {
                (dst << 3) | 6
            } else {
                0xc0 | (dst << 3) | src
            };
            let source = if mode == 2 {
                "(%0)".into()
            } else {
                format!("%%mm{src}")
            };
            add(
                vec![0x0f, 0x70, byte, control as u8],
                format!("pshufw ${control},{source},%%mm{dst}"),
                7,
                mode,
                dst,
                src,
                control as u8,
                false,
            );
        }
    }
    for control in 0..256u32 {
        let dst = (control & 7) as u8;
        let src = ((control >> 3) & 7) as u8;
        for mode in [0, 2] {
            let byte = if mode == 2 {
                (dst << 3) | 6
            } else {
                0xc0 | (dst << 3) | src
            };
            add(
                vec![0x0f, 0xc4, byte, control as u8],
                format!(
                    "pinsrw ${control},{},%%mm{dst}",
                    if mode == 2 { "(%0)" } else { "%1" }
                ),
                8,
                mode,
                dst,
                src,
                control as u8,
                false,
            );
        }
    }
    for control in 0..256u32 {
        let dst = (control & 7) as u8;
        for src in 0..8u8 {
            add(
                vec![0x0f, 0xc5, 0xc0 | (dst << 3) | src, control as u8],
                format!("pextrw ${control},%%mm{src},%0"),
                9,
                0,
                dst,
                src,
                control as u8,
                true,
            );
        }
    }
    for dst in 0..8u8 {
        for src in 0..8u8 {
            add(
                vec![0x0f, 0xd7, 0xc0 | (dst << 3) | src],
                format!("pmovmskb %%mm{src},%0"),
                10,
                0,
                dst,
                src,
                0,
                true,
            );
        }
    }
    // Cross destination/source fields with lane selection independently; the
    // all-imm8 rows above intentionally rotate destinations to bound C size.
    for dst in 0..8u8 {
        for control in [0x1b, 0xb1, 0x4e, 0xe4] {
            for mode in 0..3u8 {
                let src = if mode == 1 { dst } else { (dst + 1) & 7 };
                let byte = if mode == 2 {
                    (dst << 3) | 6
                } else {
                    0xc0 | (dst << 3) | src
                };
                let source = if mode == 2 {
                    "(%0)".into()
                } else {
                    format!("%%mm{src}")
                };
                add(
                    vec![0x0f, 0x70, byte, control],
                    format!("pshufw ${control},{source},%%mm{dst}"),
                    7,
                    mode,
                    dst,
                    src,
                    control,
                    false,
                );
            }
        }
        for control in 0..4u8 {
            for src in 0..9u8 {
                let memory = src == 8;
                let byte = if memory {
                    (dst << 3) | 6
                } else {
                    0xc0 | (dst << 3) | src
                };
                add(
                    vec![0x0f, 0xc4, byte, control],
                    format!(
                        "pinsrw ${control},{},%%mm{dst}",
                        if memory { "(%0)" } else { "%1" }
                    ),
                    8,
                    if memory { 2 } else { 0 },
                    dst,
                    if memory { 0 } else { src },
                    control,
                    false,
                );
            }
            for src in 0..8u8 {
                add(
                    vec![0x0f, 0xc5, 0xc0 | (dst << 3) | src, control],
                    format!("pextrw ${control},%%mm{src},%0"),
                    9,
                    0,
                    dst,
                    src,
                    control,
                    true,
                );
            }
        }
    }
    assert_eq!(forms.len(), 4200);
    c.push_str("static void (*const generated[@FORMS@])(void)={");
    for i in 0..forms.len() {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic uint32_t (*const physical[@FORMS@])(const void*,uint32_t)={");
    for i in 0..forms.len() {
        c.push_str(&format!("hardware_{i},"));
    }
    c.push_str("};\n");
    for (name, field) in [
        ("kinds", 0),
        ("modes", 1),
        ("destinations", 2),
        ("sources", 3),
        ("controls", 4),
    ] {
        c.push_str(&format!("static const unsigned {name}[@FORMS@]={{"));
        for &(o, m, d, s, k) in &forms {
            c.push_str(&format!("{},", [o, m, d, s, k][field]));
        }
        c.push_str("};\n");
    }
    c.replace("@FORMS@", &forms.len().to_string())
}
#[test]
fn lanes_reject_invalid_forms_and_metadata() {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for (_, opcode) in BINARY {
        assert!(emit(&[0x66, 0x0f, opcode, 0xc1]).is_err());
        let x = d.linear(&[0x0f, opcode, 6], 0x10000, &[]).unwrap();
        for width in [1, 2, 4, 16] {
            let mut bad = x.instructions()[0].clone();
            bad.operands[1].mem_size = width;
            assert!(instruction(&bad).is_err());
        }
    }
    for bytes in [
        vec![0x0f, 0x70, 0xc1, 255],
        vec![0x0f, 0xc4, 0xc1, 255],
        vec![0x0f, 0xc5, 0xc1, 255],
        vec![0x0f, 0xd7, 0xc1],
    ] {
        let mut prefix = vec![0x66];
        prefix.extend_from_slice(&bytes);
        assert!(emit(&prefix).is_err());
        let x = d.linear(&bytes, 0x10000, &[]).unwrap();
        let original = &x.instructions()[0];
        for operand in 0..2 {
            let mut bad = original.clone();
            bad.operands[operand].reg = Some("bad;untrusted()".into());
            assert!(instruction(&bad).is_err());
            let mut bad = original.clone();
            bad.operands[operand].expr = Some("untrusted()".into());
            assert!(instruction(&bad).is_err());
        }
        if original.operands.len() == 3 {
            for count in [0, 3, 256, u32::MAX] {
                let mut bad = original.clone();
                bad.operands[2].imm = Some(count);
                assert!(instruction(&bad).is_err());
            }
        }
    }
    let x = d.linear(&[0x0f, 0xc4, 6, 0], 0x10000, &[]).unwrap();
    for width in [1, 4, 8, 16] {
        let mut bad = x.instructions()[0].clone();
        bad.operands[1].mem_size = width;
        assert!(instruction(&bad).is_err());
    }
}
#[test]
fn export_mmx_lanes_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_MMX_LANES_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_lanes_match_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("lanes.c");
    let binary = dir.path().join("lanes");
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
        .arg(r.join("gamepkg_x86_mmx_lanes.c"))
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
