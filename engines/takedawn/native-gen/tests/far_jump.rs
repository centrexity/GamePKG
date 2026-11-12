#[test]
#[cfg(target_os = "linux")]
fn direct_far_code_plans_preserve_fault_priority_and_fetch_boundaries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let corpus = work.path().join("vectors.bin");
    let binary = work.path().join("far");
    let rows: [[u32; 18]; 5] = [
        [
            0, 24, 0, 0x1000, 0xf000ffff, 0xcf9a1f, 127, 15, 1, 0, 0, 24, 0xf000ffff, 0xcf9b1f, 2,
            2, 0, 0,
        ],
        [
            0, 24, 0, 0x1000, 0xf000ffff, 0xcf1a1f, 127, 15, 1, 11, 24, 8, 0xf000ffff, 0xcf1a1f, 2,
            2, 1, 0xffff0000,
        ],
        [
            3, 24, 0, 0x1000, 0xf000ffff, 0xcf1a1f, 127, 15, 1, 13, 24, 35, 0xf000ffff, 0xcf1a1f,
            2, 2, 1, 0xffff0030,
        ],
        [
            0, 24, 0, 0x1000, 0xf0000fff, 0x409a1f, 127, 15, 1, 13, 0, 8, 0xf0000fff, 0x409a1f, 2,
            2, 1, 0xffff0000,
        ],
        // A valid transfer followed by a separate target-fetch fault.
        [
            0, 24, 0, 0x1000, 0xf0001000, 0x409a1f, 127, 15, 1, 13, 0, 24, 0xf0001000, 0x409b1f, 2,
            0x10002, 2, 0x1000,
        ],
    ];
    let mut bytes = b"GPKFAR02".to_vec();
    bytes.extend((rows.len() as u32).to_le_bytes());
    for row in rows {
        for value in row {
            bytes.extend(value.to_le_bytes());
        }
    }
    std::fs::write(&corpus, bytes).unwrap();
    let mut cmd = std::process::Command::new("cc");
    cmd.args([
        "-std=c11",
        "-O3",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-fsanitize=address,undefined",
        "-fno-sanitize-recover=all",
    ])
    .arg("-I")
    .arg(&runtime)
    .arg(root.join("tests/portable_far_jump.c"))
    .arg(runtime.join("gamepkg_x86_far_jump.c"))
    .arg(runtime.join("gamepkg_x86_fault.c"));
    let built = cmd.arg("-o").arg(&binary).output().unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
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
