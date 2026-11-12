use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn fixture() -> String {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let mut code = String::new();
    let mut tables = String::new();
    let mut operations = Vec::new();
    for op in 0..2u8 {
        let field = if op == 0 { 4 } else { 6 };
        let mut opcodes = Vec::new();
        for mode in 0..6 {
            let bytes = match mode {
                0 => vec![0xdf, field << 3 | 6],
                1 => vec![0x64, 0xdf, field << 3 | 6],
                2 => vec![0x67, 0xdf, field << 3],
                3 => vec![0xdf, 0x84 | (field << 3), 0xfe, 0x20, 0, 0, 0],
                4 => vec![0xdf, 0x45 | (field << 3), 0],
                _ => vec![0xdf, 0x05 | (field << 3), 1, 4, 0, 0],
            };
            let decoded = decoder.linear(&bytes, 0x10000, &[]).unwrap();
            assert_eq!(decoded.instructions().len(), 1);
            let insn = &decoded.instructions()[0];
            assert_eq!(usize::from(insn.size), bytes.len());
            assert_eq!(insn.mnemonic, if op == 0 { "fbld" } else { "fbstp" });
            let emitted = instruction(insn).unwrap();
            code.push_str(&format!(
                "static void generated_{op}_{mode}(void){{uint32_t ebp=1025;{emitted}}}\n"
            ));
            let start = usize::from(mode == 1 || mode == 2);
            opcodes.push(format!("{}u", 0x700 | u32::from(bytes[start + 1])));
        }
        tables.push_str(&format!(
            "{{{}}},",
            (0..6)
                .map(|mode| format!("generated_{op}_{mode}"))
                .collect::<Vec<_>>()
                .join(",")
        ));
        operations.push(format!("{{{}}}", opcodes.join(",")));
    }
    code.push_str(&format!("static void (*const generated[2][6])(void)={{{tables}}};\nstatic const unsigned opcodes[2][6]={{{}}};\n",operations.join(",")));
    include_str!("x87_bcd_oracle.c").replace("/* GENERATED */", &code)
}
#[test]
fn export_bcd_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_X87_BCD_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[test]
fn packed_bcd_rejects_wrong_width_prefix_and_metadata() {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for bytes in [&[0xdf, 0x26][..], &[0xdf, 0x36][..]] {
        let decoded = decoder.linear(bytes, 0x10000, &[]).unwrap();
        let original = &decoded.instructions()[0];
        assert!(instruction(original).is_ok());
        for width in [1, 2, 4, 8, 16] {
            let mut bad = original.clone();
            bad.operands[0].mem_size = width;
            assert!(instruction(&bad).is_err());
        }
        for segment in ["cs", "ds", "es", "ss", "gs"] {
            let mut bad = original.clone();
            bad.operands[0].mem_seg = Some(segment.into());
            assert!(instruction(&bad).is_err());
        }
        let mut bad = original.clone();
        bad.operands[0].expr_src = Some("unsafe()".into());
        assert!(instruction(&bad).is_err());
        let mut bad = original.clone();
        bad.mnemonic = if original.mnemonic == "fbld" {
            "fbstp"
        } else {
            "fbld"
        }
        .into();
        assert!(instruction(&bad).is_err());
    }
    for bytes in [
        &[0x66, 0xdf, 0x26][..],
        &[0xf0, 0xdf, 0x36][..],
        &[0x65, 0xdf, 0x26][..],
    ] {
        let decoded = decoder.linear(bytes, 0x10000, &[]).unwrap();
        assert!(decoded
            .instructions()
            .iter()
            .all(|i| instruction(i).is_err()));
    }
}
#[test]
#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "x86")))]
fn packed_bcd_matches_native_x87_state_and_fault_priority() {
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("bcd");
    let source = directory.path().join("bcd.c");
    std::fs::write(&source, fixture()).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let mut command = std::process::Command::new("cc");
    command
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
        .arg(&runtime)
        .arg("-I")
        .arg(runtime.join("softfloat"))
        .arg("-I")
        .arg(runtime.join("softfloat/source/include"))
        .arg("-I")
        .arg(runtime.join("softfloat/source/8086-SSE"))
        .arg(&source);
    for name in ["x87", "x87_load", "x87_store", "fault", "memory"] {
        command.arg(runtime.join(format!("gamepkg_x86_{name}.c")));
    }
    let build = command.arg("-o").arg(&binary).output().unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = std::process::Command::new(binary).output().unwrap();
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    print!("{}", String::from_utf8_lossy(&run.stdout));
}
