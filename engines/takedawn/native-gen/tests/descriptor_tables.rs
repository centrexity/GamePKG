#[test]
#[cfg(target_os = "linux")]
fn virtual_tables_require_configuration_ownership_and_checked_memory() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let directory = tempfile::tempdir().unwrap();
    let corpus = directory.path().join("vectors.bin");
    let binary = directory.path().join("tables");
    let mut bytes = b"GPKTBL01".to_vec();
    bytes.extend(4u32.to_le_bytes());
    for (op, operand16, base, limit, expected) in [
        (
            0u32,
            0u32,
            0xc3d4e5f6u32,
            0xa1b2u32,
            [0xb2, 0xa1, 0xf6, 0xe5, 0xd4, 0xc3],
        ),
        (0, 1, 0xc3d4e5f6, 0xa1b2, [0xb2, 0xa1, 0xf6, 0xe5, 0xd4, 0]),
        (
            1,
            0,
            0x01234567,
            0x89ab,
            [0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
        ),
        (1, 1, 0xffffffff, 0xffff, [0xff, 0xff, 0xff, 0xff, 0xff, 0]),
    ] {
        for word in [op, operand16, base, limit] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend(expected);
        bytes.extend([0x5a, 0x5a]);
    }
    std::fs::write(&corpus, bytes).unwrap();
    let mut build = std::process::Command::new("cc");
    build
        .args([
            "-std=c11",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pthread",
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg("-I")
        .arg(&runtime)
        .arg(root.join("tests/portable_tables.c"));
    for name in ["cpu", "interrupt", "table_store", "memory", "fault"] {
        build.arg(runtime.join(format!("gamepkg_x86_{name}.c")));
    }
    let result = build.arg("-o").arg(&binary).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = std::process::Command::new(binary)
        .arg(corpus)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    print!("{}", String::from_utf8_lossy(&result.stdout));
}
