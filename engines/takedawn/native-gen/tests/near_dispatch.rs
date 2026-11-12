use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::dispatch::controlled_block};
fn opcode(kind: usize, target: u32) -> Vec<u8> {
    let rel32 = |op: u8| {
        let mut b = vec![op];
        b.extend(target.wrapping_sub(5).to_le_bytes());
        b
    };
    match kind {
        0 => rel32(0xe8),
        1 => vec![0xff, 0xd7],
        2 => vec![0xff, 0x16],
        3 => vec![0xff, 0x14, 0x24],
        4 => vec![0xff, 0x54, 0x24, 0xfc],
        5 => vec![0xff, 0xd4],
        6 => rel32(0xe9),
        7 => vec![0xff, 0xe7],
        8 => vec![0xff, 0x26],
        9 => vec![0xeb, target.wrapping_sub(2) as u8],
        10 => vec![0x74, target.wrapping_sub(2) as u8],
        11 => vec![0x75, target.wrapping_sub(2) as u8],
        12 => {
            let mut b = vec![0x0f, 0x84];
            b.extend(target.wrapping_sub(6).to_le_bytes());
            b
        }
        13 => vec![0xe3, target.wrapping_sub(2) as u8],
        14 => vec![0x67, 0xe3, target.wrapping_sub(3) as u8],
        15..=17 => vec![
            if kind == 15 {
                0xe2
            } else if kind == 16 {
                0xe1
            } else {
                0xe0
            },
            target.wrapping_sub(2) as u8,
        ],
        18 => vec![0x67, 0xe2, target.wrapping_sub(3) as u8],
        19 => vec![0xc3],
        20 => vec![0xc2, 8, 0],
        21 => vec![0x66, 0xc3],
        _ => {
            assert_eq!(kind, 22);
            vec![0x66, 0xc2, 8, 0]
        }
    }
}
fn fixture() -> String {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let mut source = String::new();
    let mut functions = String::new();
    let mut encodings = String::new();
    for kind in 0..23 {
        for (bad, target) in [64u32, 128, 127].into_iter().enumerate() {
            let n = kind * 3 + bad;
            let bytes = opcode(kind, target);
            let decoded = decoder.linear(&bytes, 0, &[]).unwrap();
            let block = controlled_block(&decoded, 0, bytes.len() as u32, &budget).unwrap();
            source.push_str(
                &block
                    .source()
                    .replace("gamepkg_cpu_block_00000000", &format!("near_case_{n}")),
            );
            functions.push_str(&format!("near_case_{n},\n"));
            encodings.push_str(&format!(
                "{{{}u,{{{}}}}},\n",
                bytes.len(),
                (0..8)
                    .map(|i| bytes.get(i).copied().unwrap_or(0).to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    }
    include_str!("near_dispatch_fixture.c")
        .replace("/* PROGRAM */", &source)
        .replace("/* FUNCTIONS */", &functions)
        .replace("/* ENCODINGS */", &encodings)
}
#[test]
fn export_near_dispatch_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_NEAR_DISPATCH_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn near_transfers_validate_targets_before_stack_counter_and_retirement_commit() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let path = work.path().join("near.c");
    let bin = work.path().join("near");
    std::fs::write(&path, fixture()).unwrap();
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
        .arg(path);
    for name in [
        "dispatch",
        "cpu",
        "interrupt",
        "cpu_dispatch",
        "table_store",
        "far_memory",
        "far_jump",
        "memory",
        "fault",
        "x87",
    ] {
        build.arg(runtime.join(format!("gamepkg_x86_{name}.c")));
    }
    let out = build.arg("-o").arg(&bin).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = std::process::Command::new(bin).output().unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    print!("{}", String::from_utf8_lossy(&out.stdout));
}
