use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn decoded(bytes: &[u8]) -> gamepkg_takedawn_native_gen::decode::Instruction {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(bytes, 0x10000, &[]).unwrap();
    assert_eq!(x.instructions().len(), 1);
    x.instructions()[0].clone()
}
fn fixture() -> String {
    let mut c = include_str!("x87_environment_oracle.c").to_string();
    for word16 in 0..2 {
        for (op, first, second) in [
            (0, 0xd9, 0x2e),
            (1, 0xd9, 0x26),
            (2, 0xdd, 0x26),
            (3, 0xd9, 0x36),
            (4, 0xdd, 0x36),
        ] {
            let mut bytes = vec![];
            if word16 != 0 {
                bytes.push(0x66);
            }
            bytes.extend([first, second]);
            let x = decoded(&bytes);
            let code = instruction(&x).unwrap_or_else(|e| panic!("{bytes:x?} {x:?}: {e}"));
            c.push_str(&format!(
                "static void emitted_{}(void){{uint32_t ebp=0xabcdef12;{code}}}\n",
                word16 * 5 + op
            ));
            c.push_str(&format!("static void physical_{}(unsigned char *p){{__asm__ volatile(\"{} .byte {},{}\"::\"S\"(p):\"memory\");}}\n",word16*5+op,if word16!=0{".byte 0x66;"}else{""},first,second));
        }
    }
    c.push_str("static void (*const generated[10])(void)={");
    for i in 0..10 {
        c.push_str(&format!("emitted_{i},"));
    }
    c.push_str("};\nstatic void (*const physical[10])(unsigned char *)={");
    for i in 0..10 {
        c.push_str(&format!("physical_{i},"));
    }
    c.push_str("};\n");
    c
}
#[test]
fn export_environment_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_X87_ENVIRONMENT_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
fn environment_operand_validation() {
    for bytes in [
        &[0xd9, 0x26][..],
        &[0x66, 0xdd, 0x36],
        &[0x64, 0xd9, 0x2e],
        &[0x67, 0xd9, 0x24],
    ] {
        let x = decoded(bytes);
        assert!(instruction(&x).is_ok(), "{x:?}");
        for change in 0..4 {
            let mut bad = x.clone();
            match change {
                0 => bad.operands[0].mem_size = 1,
                1 => bad.operands[0].expr_src = Some("bad()".into()),
                2 => bad.mnemonic = "fldcw".into(),
                _ => bad.operands[0].mem_seg = Some("gs".into()),
            };
            if change != 2 || x.mnemonic != "fldcw" {
                assert!(instruction(&bad).is_err());
            }
        }
    }
    for bytes in [
        &[0xf0, 0xd9, 0x26][..],
        &[0xf3, 0xdd, 0x36],
        &[0x65, 0xd9, 0x2e],
    ] {
        let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
        let x = d.linear(bytes, 0x10000, &[]).unwrap();
        assert!(x.instructions().is_empty() || instruction(&x.instructions()[0]).is_err());
    }
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn generated_environment_matches_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("env.c");
    let binary = dir.path().join("env");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let mut cmd = std::process::Command::new("cc");
    cmd.args([
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
    .arg(&runtime)
    .arg(source);
    for s in [
        "gamepkg_x86_x87_environment.c",
        "gamepkg_x86_x87.c",
        "gamepkg_x86_memory.c",
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
