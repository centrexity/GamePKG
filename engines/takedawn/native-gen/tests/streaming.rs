use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete streaming instruction"
    );
    instruction(&x.instructions()[0])
}
fn memory(opcode: u8, field: u8, mode: u8) -> Vec<u8> {
    let mut b = vec![0x0f, opcode];
    if mode == 1 {
        b.insert(0, 0x64);
    }
    if mode == 2 {
        b.insert(0, 0x67);
    }
    match mode {
        0 | 1 => b.push((field << 3) | 6),
        2 => b.push(field << 3),
        3 => b.extend_from_slice(&[0x45 | (field << 3), 0]),
        4 => b.extend_from_slice(&[0x05 | (field << 3), 0, 4, 0, 0]),
        _ => b.extend_from_slice(&[0x84 | (field << 3), 0xfe, 0x20, 0, 0, 0]),
    }
    b
}
fn fixture() -> String {
    let mut c = include_str!("streaming_oracle.c").to_string();
    let mut forms = vec![];
    let mut add = |b: Vec<u8>,
                   asm: String,
                   kind: u8,
                   mode: u8,
                   src: u8,
                   mask: u8,
                   implicit: bool| {
        let i = forms.len();
        let input = if implicit { "D" } else { "r" };
        c.push_str(&format!("static void emitted_{i}(void){{uint32_t ebp=local_ebp;(void)ebp;{}++completed;}}\nstatic void hardware_{i}(unsigned char*ptr){{(void)ptr;__asm__ volatile(\"{asm}\"::\"{input}\"(ptr):\"memory\");}}\n",emit(&b).unwrap_or_else(|e|panic!("{b:x?}: {e}"))));
        forms.push((kind, mode, src, mask));
    };
    for (kind, (name, opcode)) in [("movntq", 0xe7), ("movntps", 0x2b)].iter().enumerate() {
        for src in 0..8u8 {
            for mode in 0..6u8 {
                let reg = if kind == 0 { "mm" } else { "xmm" };
                add(
                    memory(*opcode, src, mode),
                    format!("{name} %%{reg}{src},(%0);sfence"),
                    kind as u8,
                    mode,
                    src,
                    0,
                    false,
                );
            }
        }
    }
    for src in 0..8u8 {
        for mask in 0..8u8 {
            for mode in 0..3u8 {
                let mut b = vec![0x0f, 0xf7, 0xc0 | (src << 3) | mask];
                if mode == 1 {
                    b.insert(0, 0x64);
                }
                if mode == 2 {
                    b.insert(0, 0x67);
                }
                add(
                    b,
                    format!("maskmovq %%mm{mask},%%mm{src};sfence"),
                    2,
                    mode,
                    src,
                    mask,
                    true,
                );
            }
        }
    }
    for byte in 0xf8..=0xff {
        add(
            vec![0x0f, 0xae, byte],
            format!(".byte 15,174,{byte}"),
            3,
            0,
            0,
            0,
            false,
        );
    }
    for (hint, name) in ["prefetchnta", "prefetcht0", "prefetcht1", "prefetcht2"]
        .iter()
        .enumerate()
    {
        for mode in 0..6u8 {
            add(
                memory(0x18, hint as u8, mode),
                format!("{name} (%0)"),
                4,
                mode,
                0,
                0,
                false,
            );
        }
    }
    assert_eq!(forms.len(), 320);
    c.push_str("static void (*const generated[@FORMS@])(void)={");
    for i in 0..forms.len() {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic void (*const physical[@FORMS@])(unsigned char*)={");
    for i in 0..forms.len() {
        c.push_str(&format!("hardware_{i},"));
    }
    c.push_str("};\n");
    for (name, index) in [("kinds", 0), ("modes", 1), ("sources", 2), ("masks", 3)] {
        c.push_str(&format!("static const unsigned {name}[@FORMS@]={{"));
        for &(k, m, s, a) in &forms {
            c.push_str(&format!("{},", [k, m, s, a][index]));
        }
        c.push_str("};\n");
    }
    c.replace("@FORMS@", &forms.len().to_string())
}
#[test]
fn streaming_rejects_wrong_widths_prefixes_registers_and_metadata() {
    for b in [
        vec![0x66, 0x0f, 0xe7, 6],
        vec![0x65, 0x0f, 0xe7, 6],
        vec![0xf0, 0x0f, 0xae, 0xf8],
        vec![0xf3, 0x0f, 0x2b, 6],
        vec![0x0f, 0xe7, 0xc0],
        vec![0x0f, 0x2b, 0xc0],
    ] {
        assert!(emit(&b).is_err());
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for b in [vec![0x0f, 0xe7, 6], vec![0x0f, 0x2b, 6]] {
        let x = d.linear(&b, 0x10000, &[]).unwrap();
        let original = &x.instructions()[0];
        for w in [1, 2, 4, 8, 16] {
            if w == original.operands[0].mem_size {
                continue;
            }
            let mut bad = original.clone();
            bad.operands[0].mem_size = w;
            assert!(instruction(&bad).is_err());
        }
        for idx in 0..2 {
            let mut bad = original.clone();
            bad.operands[idx].expr = Some("untrusted()".into());
            assert!(instruction(&bad).is_err());
        }
        let mut bad = original.clone();
        bad.operands[1].reg = Some("mm8;untrusted()".into());
        assert!(instruction(&bad).is_err());
    }
    for b in [vec![0x0f, 0xf7, 0xc1], vec![0x0f, 0x18, 6]] {
        let x = d.linear(&b, 0x10000, &[]).unwrap();
        let mut bad = x.instructions()[0].clone();
        bad.operands[0].expr_src = Some("untrusted()".into());
        assert!(instruction(&bad).is_err());
    }
}
#[test]
fn export_streaming_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_STREAMING_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_streaming_matches_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("streaming.c");
    let exe = dir.path().join("streaming");
    std::fs::write(&src, fixture()).unwrap();
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
        .arg(src)
        .arg(r.join("gamepkg_x86_memory.c"))
        .arg(r.join("gamepkg_x86_x87.c"))
        .arg(r.join("gamepkg_x86_fault.c"))
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr[..out.stderr.len().min(4096)])
    );
    let out = std::process::Command::new(exe).output().unwrap();
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr[..out.stderr.len().min(4096)])
    );
    print!("{}", String::from_utf8_lossy(&out.stdout));
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn concrete_ram_transactions_lifetime_and_threads() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("ram");
    let r = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let out = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O2",
            "-pthread",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg("-I")
        .arg(&r)
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/memory_runtime.c"))
        .arg(r.join("gamepkg_x86_memory.c"))
        .arg(r.join("gamepkg_x86_fault.c"))
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr[..out.stderr.len().min(4096)])
    );
    let out = std::process::Command::new(exe)
        .env("ASAN_OPTIONS", "detect_leaks=1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr[..out.stderr.len().min(4096)])
    );
    print!("{}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn sfence_aliases_preserve_original_bytes_and_cfg_boundaries() {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for tail in 0xf8..=0xff {
        for prefixes in [vec![], vec![0x64], vec![0x67], vec![0x64, 0x67]] {
            let mut bytes = prefixes;
            bytes.extend_from_slice(&[0x0f, 0xae, tail]);
            let size = bytes.len();
            bytes.extend_from_slice(&[0x90, 0xc3]);
            let linear = d.linear(&bytes, 0x10000, &[]).unwrap();
            let cfg = d
                .cfg(
                    &bytes,
                    0x10000,
                    &[0x10000],
                    &std::collections::BTreeSet::new(),
                    &std::collections::BTreeSet::new(),
                )
                .unwrap();
            assert_eq!(linear.instructions(), cfg.instructions());
            assert_eq!(linear.instructions().len(), 3);
            let first = &linear.instructions()[0];
            assert_eq!(usize::from(first.size), size);
            assert_eq!(first.bytes_hex, hex::encode(&bytes[..size]));
            assert_eq!(first.mnemonic, "sfence");
            assert!(instruction(first).is_ok());
        }
    }
}
