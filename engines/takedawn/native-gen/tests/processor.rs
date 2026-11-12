use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{
    decode::Decoder,
    lower::dispatch::controlled_block,
    processor::{CodeLimitInitialization, Processor, Stack, PCR_BYTES, THREAD_BYTES},
};
fn stack() -> Stack {
    Stack {
        low: 0x10000,
        high: 0x12000,
        tls: 0x11fc0,
        tls_bytes: 32,
        esp: 0x11fa0,
    }
}
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
#[test]
fn explicit_initial_fields_are_disjoint_little_endian_and_sparse() {
    let p = Processor::new(0x2000, CodeLimitInitialization::RunTitlePath).unwrap();
    let t = p.thread(0x3000, stack()).unwrap();
    assert_eq!(
        (p.address(), t.address(), t.stack()),
        (0x2000, 0x3000, stack())
    );
    let cpu = p.initial_fields();
    let thread = p.thread_fields(&t).unwrap();
    assert_eq!(
        cpu.map(|f| (f.address, f.bytes.len(), f.writable)),
        [
            (0x201c, 12, true),
            (0x202c, 8, true),
            (0x2050, 12, true),
            (0x2278, 4, false)
        ]
    );
    assert_eq!(
        thread.map(|f| (f.address, f.bytes.len(), f.writable)),
        [(0x2000, 28, true), (0x2028, 4, true), (0x301c, 16, true)]
    );
    assert_eq!(
        (
            word(cpu[0].bytes, 0),
            word(cpu[0].bytes, 4),
            cpu[0].bytes[8]
        ),
        (0x2000, 0x2028, 0)
    );
    assert_eq!(cpu[1].bytes, [0; 8]);
    assert_eq!(
        (
            word(cpu[2].bytes, 0),
            word(cpu[2].bytes, 4),
            word(cpu[2].bytes, 8)
        ),
        (0x2050, 0x2050, 0)
    );
    assert_eq!(cpu[3].bytes, [0; 4]);
    assert_eq!(
        (
            word(thread[0].bytes, 0),
            word(thread[0].bytes, 4),
            word(thread[0].bytes, 8),
            word(thread[0].bytes, 24)
        ),
        (u32::MAX, 0x11fe0, 0x10000, 0x2000)
    );
    assert_eq!(word(thread[1].bytes, 0), 0x3000);
    assert_eq!(
        (0..4)
            .map(|i| word(thread[2].bytes, i * 4))
            .collect::<Vec<_>>(),
        vec![0x12000, 0x10000, 0x11fa0, 0x11fc0]
    );
    let mut ram = vec![0xa5; 0x4000];
    p.install_initial(&t, 0, &mut ram).unwrap();
    for (a, n) in [
        (0x2034, 0x1c),
        (0x205c, 0x21c),
        (0x227c, 4),
        (0x3000, 0x1c),
        (0x302c, 0x114),
    ] {
        assert!(ram[a..a + n].iter().all(|&b| b == 0xa5));
    }
    let p2 = Processor::new(0x4000, CodeLimitInitialization::RunTitlePath).unwrap();
    assert!(p2.thread_fields(&t).is_err());
}
#[test]
fn invalid_overlapping_or_unbacked_contexts_fail_without_writes() {
    for base in [0, 1, 0x1001, u32::MAX, 0xffff_fffc] {
        assert!(Processor::new(base, CodeLimitInitialization::RunTitlePath).is_err());
    }
    let p = Processor::new(0x2000, CodeLimitInitialization::RunTitlePath).unwrap();
    for address in [0, 0x2000, 0x2200, 0x10000, 0x11f80, 0xfffffffc, 0x3001] {
        assert!(p.thread(address, stack()).is_err());
    }
    for s in [
        Stack { low: 0, ..stack() },
        Stack {
            low: 0x12000,
            ..stack()
        },
        Stack {
            high: 0x11fd0,
            ..stack()
        },
        Stack {
            tls: 0x11fc1,
            ..stack()
        },
        Stack {
            tls: u32::MAX - 15,
            tls_bytes: 32,
            ..stack()
        },
        Stack {
            esp: 0x11fbc,
            ..stack()
        },
        Stack {
            esp: 0x11fa1,
            ..stack()
        },
        Stack {
            low: 0x2000,
            high: 0x12000,
            ..stack()
        },
    ] {
        assert!(p.thread(0x3000, s).is_err());
    }
    let t = p.thread(0x3000, stack()).unwrap();
    for (base, len) in [
        (0, 0x302b),
        (0x2001, 0x2000),
        (0x301c, 16),
        (0xffff_fffc, 8),
    ] {
        let mut ram = vec![0xa5; len];
        let saved = ram.clone();
        assert!(p.install_initial(&t, base, &mut ram).is_err());
        assert_eq!(ram, saved);
    }
    let high = Processor::new(
        u32::MAX - (PCR_BYTES - 1),
        CodeLimitInitialization::RunTitlePath,
    )
    .unwrap();
    let ht = high
        .thread(u32::MAX - (THREAD_BYTES - 1) - 0x1000, stack())
        .unwrap();
    assert!(high
        .initial_fields()
        .iter()
        .all(|f| u64::from(f.address) + f.bytes.len() as u64 <= u64::from(u32::MAX) + 1));
    assert_eq!(ht.stack(), stack());
}
fn fixture() -> String {
    let p = Processor::new(0x2000, CodeLimitInitialization::RunTitlePath).unwrap();
    let t = p.thread(0x3000, stack()).unwrap();
    let cpu = p.initial_fields();
    let thread = p.thread_fields(&t).unwrap();
    let fields = cpu
        .iter()
        .chain(thread.iter())
        .map(|f| {
            format!(
                "{{{}u,{}u,{}u,{{{}}}}}",
                f.address,
                f.bytes.len(),
                if f.writable { 3 } else { 1 },
                (0..32)
                    .map(|i| f.bytes.get(i).copied().unwrap_or(0).to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let code = [
        0x64, 0x8b, 0x1d, 0x18, 0, 0, 0, 0x64, 0x8b, 0x3d, 0x1c, 0, 0, 0, 0x64, 0xa1, 0x20, 0, 0,
        0, 0x8b, 0x10, 0x8b, 0x4a, 0x28, 0x8b, 0x70, 0x28, 0x8b, 0x80, 0x50, 0x02, 0, 0, 0xc3,
    ];
    let budget = Budget::new(32 * 1024 * 1024);
    let decoded = Decoder::new(&budget)
        .unwrap()
        .linear(&code, 0x400000, &[])
        .unwrap();
    let program =
        controlled_block(&decoded, 0x400000, 0x400000 + code.len() as u32, &budget).unwrap();
    include_str!("processor_fixture.c")
        .replace("/* FIELDS */", &fields)
        .replace("/* PROGRAM */", program.source())
}
#[test]
fn export_processor_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_PROCESSOR_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn checked_processor_traversal_preserves_unknown_gaps_and_fp() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("processor.c");
    let bin = dir.path().join("processor");
    std::fs::write(&src, fixture()).unwrap();
    let mut cc = std::process::Command::new("cc");
    cc.args([
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
    .arg(src);
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
        cc.arg(runtime.join(format!("gamepkg_x86_{name}.c")));
    }
    let out = cc.arg("-o").arg(&bin).output().unwrap();
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
