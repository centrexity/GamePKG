use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete MMX fixture"
    );
    instruction(&x.instructions()[0])
}
#[test]
fn mmx_x87_rejects_wrong_width_prefix_register_and_expressions() {
    for opcode in [
        0xec, 0xed, 0xdc, 0xdd, 0xe8, 0xe9, 0xd8, 0xd9, 0xd5, 0xe5, 0xe4, 0xf5, 0x63, 0x6b, 0x67,
        0x60, 0x61, 0x62, 0x68, 0x69, 0x6a,
    ] {
        assert!(emit(&[0x66, 0x0f, opcode, 0xc1]).is_err());
        let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
        let decoded = d.linear(&[0x0f, opcode, 0x06], 0x10000, &[]).unwrap();
        let original = &decoded.instructions()[0];
        for width in [1, 2, 4, 8, 16] {
            if width == original.operands[1].mem_size {
                continue;
            }
            let mut bad = original.clone();
            bad.operands[1].mem_size = width;
            assert!(instruction(&bad).is_err());
        }
        let mut bad = original.clone();
        bad.operands[1].expr = Some("untrusted()".into());
        assert!(instruction(&bad).is_err());
    }
    for bytes in [
        &[0x66, 0x0f, 0xfc, 0xc1][..],
        &[0xf3, 0x0f, 0x6f, 0xc1],
        &[0xf0, 0x0f, 0xfc, 0x06],
        &[0x65, 0x0f, 0x6f, 0x06],
        &[0xdf, 0x0e], // FISTTP needs a separately qualified SSE3 profile.
    ] {
        assert!(emit(bytes).is_err(), "{bytes:x?}");
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let decoded = d.linear(&[0x0f, 0x6f, 0x06], 0x10000, &[]).unwrap();
    let original = &decoded.instructions()[0];
    for width in [1, 2, 4, 16] {
        let mut bad = original.clone();
        bad.operands[1].mem_size = width;
        assert!(instruction(&bad).is_err());
    }
    for value in ["mm8", "xmm0", "mm0; unsafe()"] {
        let mut bad = original.clone();
        bad.operands[0].reg = Some(value.into());
        assert!(instruction(&bad).is_err());
    }
    let mut bad = original.clone();
    bad.operands[1].expr_src = Some("unsafe()".into());
    assert!(instruction(&bad).is_err());
}
fn fixture() -> String {
    let mut c = include_str!("mmx_x87_oracle.c").to_string();
    let ops = [
        (0xdb, "pand"),
        (0xdf, "pandn"),
        (0xeb, "por"),
        (0xef, "pxor"),
        (0xfc, "paddb"),
        (0xfd, "paddw"),
        (0xfe, "paddd"),
        (0xf8, "psubb"),
        (0xf9, "psubw"),
        (0xfa, "psubd"),
        (0x74, "pcmpeqb"),
        (0x75, "pcmpeqw"),
        (0x76, "pcmpeqd"),
        (0x64, "pcmpgtb"),
        (0x65, "pcmpgtw"),
        (0x66, "pcmpgtd"),
        (0xec, "paddsb"),
        (0xed, "paddsw"),
        (0xdc, "paddusb"),
        (0xdd, "paddusw"),
        (0xe8, "psubsb"),
        (0xe9, "psubsw"),
        (0xd8, "psubusb"),
        (0xd9, "psubusw"),
        (0xd5, "pmullw"),
        (0xe5, "pmulhw"),
        (0xe4, "pmulhuw"),
        (0xf5, "pmaddwd"),
        (0x63, "packsswb"),
        (0x6b, "packssdw"),
        (0x67, "packuswb"),
        (0x60, "punpcklbw"),
        (0x61, "punpcklwd"),
        (0x62, "punpckldq"),
        (0x68, "punpckhbw"),
        (0x69, "punpckhwd"),
        (0x6a, "punpckhdq"),
    ];
    for (op, (opcode, name)) in ops.iter().enumerate() {
        for mode in 0..3 {
            let bytes = [
                0x0f,
                *opcode,
                if mode == 0 {
                    0xc1
                } else if mode == 1 {
                    0xc0
                } else {
                    0x06
                },
            ];
            c.push_str(&format!(
                "static void emitted_{op}_{mode}(void){{uint32_t ebp=0xabcdef12;{}++completed;}}\n",
                emit(&bytes).unwrap_or_else(|e| panic!("{name} mode{mode}: {e}; {bytes:x?}"))
            ));
            let source = if mode == 0 {
                "%%mm1"
            } else if mode == 1 {
                "%%mm0"
            } else {
                "(%[ptr])"
            };
            c.push_str(&format!("static void hardware_{op}_{mode}(const void *ptr){{(void)ptr;__asm__ volatile(\"{name} {source},%%mm0\"::[ptr]\"r\"(ptr):\"mm0\",\"memory\");}}\n"));
        }
    }
    let mut extras: Vec<(Vec<u8>, String)> = vec![
        (vec![0x0f, 0x77], "emms".into()),
        (vec![0xdb, 0xe3], "fninit".into()),
        (vec![0xdb, 0xe2], "fnclex".into()),
        (vec![0x0f, 0x6e, 0xc0], "movd %[value],%%mm0".into()),
        (vec![0x0f, 0x7e, 0xc0], "movd %%mm0,%[value]".into()),
        (vec![0x0f, 0x6f, 0x06], "movq (%[ptr]),%%mm0".into()),
        (vec![0x0f, 0x7f, 0x06], "movq %%mm0,(%[ptr])".into()),
    ];
    for i in 0..8u8 {
        extras.push((
            vec![0x0f, 0x6f, 0xc0 | (i << 3) | (7 - i)],
            format!("movq %%mm{},%%mm{i}", 7 - i),
        ));
    }
    for (i, (bytes, asm)) in extras.iter().enumerate() {
        c.push_str(&format!(
            "static void extra_{i}(void){{uint32_t ebp=0xabcdef12;(void)ebp;{}++completed;}}\n",
            emit(bytes).unwrap()
        ));
        c.push_str(&format!("static void real_extra_{i}(uint64_t *ptr,uint32_t *value){{uint32_t v=*value;__asm__ volatile(\"{asm}\":[value]\"+r\"(v):[ptr]\"r\"(ptr):\"mm0\",\"mm1\",\"mm2\",\"mm3\",\"mm4\",\"mm5\",\"mm6\",\"mm7\",\"memory\");*value=v;}}\n"));
    }
    for (name, bytes) in [
        ("cw", vec![0xd9, 0x3e]),
        ("sw", vec![0xdd, 0x3e]),
        ("ax", vec![0xdf, 0xe0]),
        ("wait", vec![0x9b]),
    ] {
        c.push_str(&format!("static void control_{name}(void){{uint32_t ebp=0xabcdef12;(void)ebp;{}++completed;}}\n",emit(&bytes).unwrap()));
    }
    c.push_str("static void (*const generated[37][3])(void)={");
    for op in 0..ops.len() {
        c.push_str(&format!(
            "{{emitted_{op}_0,emitted_{op}_1,emitted_{op}_2}},"
        ));
    }
    c.push_str("};\nstatic void (*const physical[37][3])(const void*)={");
    for op in 0..ops.len() {
        c.push_str(&format!(
            "{{hardware_{op}_0,hardware_{op}_1,hardware_{op}_2}},"
        ));
    }
    c.push_str("};\nstatic void (*const extras[])(void)={");
    for i in 0..extras.len() {
        c.push_str(&format!("extra_{i},"));
    }
    c.push_str("};\nstatic void (*const real_extras[])(uint64_t*,uint32_t*)={");
    for i in 0..extras.len() {
        c.push_str(&format!("real_extra_{i},"));
    }
    c.push_str("};\n");
    c
}
#[test]
fn export_mmx_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_MMX_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_mmx_and_shared_x87_state_match_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mmx.c");
    let exe = dir.path().join("mmx");
    std::fs::write(&src, fixture()).unwrap();
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
        .arg(src)
        .arg(r.join("gamepkg_x86_fault.c"))
        .arg(r.join("gamepkg_x86_x87.c"))
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = std::process::Command::new(exe).output().unwrap();
    assert!(
        out.status.success(),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    print!("{}", String::from_utf8_lossy(&out.stdout));
}
