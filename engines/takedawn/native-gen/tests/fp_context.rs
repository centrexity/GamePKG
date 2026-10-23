use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};

fn fixture() -> String {
    let mut c = String::from("#include <stdint.h>\n");
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['\"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\n#define RECOMP_XMM_DEFINED\ntypedef union RecompXmm {float f[4]; double d[2]; uint32_t u[4]; int32_t i[4]; uint64_t q[2];} RecompXmm;\nextern RECOMP_TLS RecompXmm g_xmm0,g_xmm1,g_xmm2,g_xmm3,g_xmm4,g_xmm5,g_xmm6,g_xmm7;\n");
    }
    c.push_str("#include \"gamepkg_x86_pinned_context.h\"\nRECOMP_TLS uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;\nRECOMP_TLS int g_df;\nRECOMP_TLS RecompXmm g_xmm0,g_xmm1,g_xmm2,g_xmm3,g_xmm4,g_xmm5,g_xmm6,g_xmm7;\n");
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for (name, bytes) in [
        ("divide", &[0xf7, 0xf1][..]),
        ("wait", &[0x9b][..]),
        ("mmx", &[0x0f, 0x6e, 0xc0][..]),
        ("empty", &[0x0f, 0x77][..]),
    ] {
        let decoded = decoder.linear(bytes, 0x10000, &[]).unwrap();
        assert_eq!(decoded.instructions().len(), 1);
        c.push_str(&format!(
            "static void generated_{name}(void) {{uint32_t ebp=0x12345678;(void)ebp;\n{}\n}}\n",
            instruction(&decoded.instructions()[0]).unwrap()
        ));
    }
    let capture = include_str!("fp_context_fixture.c");
    assert_eq!(capture.matches("/* RESTORE_CASES */").count(), 1);
    c.push_str(&capture.replace(
        "/* RESTORE_CASES */",
        &format!(
            "{}\n{}",
            include_str!("fp_restore_fixture.c"),
            include_str!("register_capture_fixture.c")
        ),
    ));
    c
}

#[test]
fn floating_fault_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_FP_CONTEXT_FIXTURE") {
        std::fs::write(path, c).unwrap();
    }
}

#[cfg(target_os = "linux")]
#[test]
fn floating_fault_captures_raw_state_and_preserves_boundary_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("context.c");
    let binary = dir.path().join("context");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let result = std::process::Command::new("cc")
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
        .arg(&runtime)
        .arg(source)
        .arg(runtime.join("gamepkg_x86_fault.c"))
        .arg(runtime.join("gamepkg_x86_x87.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = std::process::Command::new(binary).output().unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    print!("{}", String::from_utf8_lossy(&result.stdout));
}
