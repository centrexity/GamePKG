use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{
    decode::Decoder,
    kernel_data::{HardwareInfo, KernelVersion},
    kernel_image::{self as kernel, ExportKind, KernelImage},
    lower::dispatch::controlled_block,
    xbe::Image,
};
fn profile(budget: &Budget) -> KernelImage {
    KernelImage::build(
        HardwareInfo {
            flags: 0x11223344,
            gpu_revision: 0xa1,
            mcp_revision: 0xb1,
        },
        KernelVersion {
            major: 1,
            minor: 0,
            build: 5838,
            qfe: 1,
        },
        budget,
    )
    .unwrap()
}
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn short(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}
#[test]
fn backed_pe_sections_exports_and_actual_init_code_agree() {
    let budget = Budget::new(32 * 1024 * 1024);
    let k = profile(&budget);
    assert_eq!(k.export(279).unwrap().address, kernel::BASE + 0x10e0);
    assert_eq!(k.export(211).unwrap().address, kernel::BASE + 0x10e8);
    assert_eq!(k.export(236).unwrap().address, kernel::BASE + 0x10f0);
    assert_eq!(k.export(219).unwrap().address, kernel::BASE + 0x10f8);
    assert_eq!(
        k.host_ordinal(kernel::BASE + kernel::CALLBACK_RETURN_RVA),
        None
    );
    assert_eq!(k.host_ordinal(kernel::BASE + kernel::DPC_RETURN_RVA), None);
    let b = k.bytes();
    assert_eq!(b.len(), kernel::IMAGE_BYTES);
    assert_eq!(&b[..2], b"MZ");
    let nt = word(b, 0x3c) as usize;
    assert_eq!(&b[nt..nt + 4], b"PE\0\0");
    assert_eq!((short(b, nt + 4), short(b, nt + 6)), (0x14c, 4));
    let o = nt + 24;
    assert_eq!((short(b, o), word(b, o + 28)), (0x10b, kernel::BASE));
    assert_eq!(
        (word(b, o + 16), word(b, o + 56), word(b, o + 60)),
        (kernel::INIT_RVA, kernel::IMAGE_BYTES as u32, 0x1000)
    );
    let sections = o + short(b, nt + 20) as usize;
    let mut end = 0x1000;
    for i in 0..4 {
        let a = sections + i * 40;
        let rva = word(b, a + 12);
        let n = word(b, a + 8);
        assert_eq!(rva, end);
        assert_eq!((word(b, a + 16), word(b, a + 20)), (n, rva));
        end = rva + n;
        assert!(end as usize <= b.len());
    }
    assert_eq!(end, kernel::IMAGE_BYTES as u32);
    assert_eq!(word(b, 0x2000 + 24), 0);
    assert_eq!(word(b, 0x2000 + 32), 0x2590);
    assert_eq!(word(b, 0x2000 + 36), 0x2590);
    assert_eq!(&b[sections + 120..sections + 128], b"INIT\0\0\0\0");
    assert_eq!(
        &b[kernel::HARDWARE_RVA as usize..kernel::HARDWARE_RVA as usize + 8],
        &[0x44, 0x33, 0x22, 0x11, 0xa1, 0xb1, 0, 0]
    );
    assert_eq!(
        &b[kernel::VERSION_RVA as usize..kernel::VERSION_RVA as usize + 8],
        &[1, 0, 0, 0, 0xce, 0x16, 1, 0]
    );
    let decoder = Decoder::new(&budget).unwrap();
    let decoded = decoder
        .linear(k.init_code(), kernel::BASE + kernel::INIT_RVA, &[])
        .unwrap();
    let program = controlled_block(
        &decoded,
        kernel::BASE + kernel::INIT_RVA,
        kernel::BASE + kernel::INIT_RVA + 41,
        &budget,
    )
    .unwrap();
    assert!(program.source().contains("gamepkg_x86_dispatch_ret("));
    for ordinal in 1..=324 {
        let rva = word(b, 0x2080 + (ordinal as usize - 1) * 4);
        match k.export(ordinal) {
            None => assert_eq!(rva, 0),
            Some(e) => {
                assert_eq!(kernel::BASE + rva, e.address);
                match e.kind {
                    ExportKind::HostTrap => {
                        assert_eq!(
                            &b[rva as usize..rva as usize + 4],
                            &[0x0f, 0x0b, 0x47, 0x50]
                        );
                        assert_eq!(word(b, rva as usize + 4), ordinal);
                        assert_eq!(k.host_ordinal(e.address), Some(ordinal));
                        for n in 1..8 {
                            assert_eq!(k.host_ordinal(e.address + n), None);
                        }
                    }
                    ExportKind::Data => assert_eq!(k.host_ordinal(e.address), None),
                }
            }
        }
    }
    for address in [
        0,
        kernel::BASE,
        kernel::BASE + kernel::CALLBACK_RETURN_RVA,
        kernel::BASE + kernel::DPC_RETURN_RVA,
        kernel::BASE + kernel::TIMER_HEAD_RVA,
        kernel::BASE + kernel::INIT_RVA,
        0x800000ff,
        u32::MAX,
    ] {
        assert_eq!(k.host_ordinal(address), None);
    }
}
fn title() -> Vec<u8> {
    let mut b = vec![0; 0x3000];
    b[..4].copy_from_slice(b"XBEH");
    for (a, n) in [
        (0x104, 0x10000),
        (0x108, 0x1000),
        (0x10c, 0x4000),
        (0x11c, 2),
        (0x120, 0x10200),
        (0x128, 0x11000 ^ 0xa8fc57ab),
        (0x158, 0x12000 ^ 0x5b6d40b6),
    ] {
        put(&mut b, a, n);
    }
    for (i, va, raw, name) in [(0, 0x11000, 0x1000, 0x10300), (1, 0x12000, 0x2000, 0x10310)] {
        for (o, n) in [
            (0, 4),
            (4, va),
            (8, 0x100),
            (12, raw),
            (16, 0x100),
            (20, name),
        ] {
            put(&mut b, 0x200 + i * 56 + o, n);
        }
    }
    b[0x300..0x306].copy_from_slice(b".text\0");
    b[0x310..0x317].copy_from_slice(b".rdata\0");
    b[0x1000] = 0xc3;
    for (i, n) in [255, 322, 277, 291, 294, 324, 99, 255]
        .into_iter()
        .enumerate()
    {
        put(&mut b, 0x2000 + i * 4, 0x80000000 | n);
    }
    b
}
fn put(b: &mut [u8], a: usize, n: u32) {
    b[a..a + 4].copy_from_slice(&n.to_le_bytes());
}
#[test]
fn import_binding_is_atomic_typed_and_leaves_unknown_services_unresolved() {
    let budget = Budget::new(32 * 1024 * 1024);
    let k = profile(&budget);
    let b = title();
    let image = Image::parse(&b).unwrap();
    for i in [0, 1, 2, 3, 4, 5, 7] {
        let mut bad = b.clone();
        put(&mut bad, 0x2000 + i * 4, 0);
        let saved = bad.clone();
        assert!(k.bind_imports(&image, 0x10000, &mut bad).is_err());
        assert_eq!(bad, saved);
    }
    let mut ram = b.clone();
    assert_eq!(k.bind_imports(&image, 0x10000, &mut ram).unwrap(), 7);
    for (i, n) in [255, 322, 277, 291, 294, 324, 99, 255]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            word(&ram, 0x2000 + i * 4),
            k.export(n).map(|e| e.address).unwrap_or(0x80000000 | n)
        );
    }
    let saved = ram.clone();
    assert!(k.bind_imports(&image, 0x10000, &mut ram).is_err());
    assert_eq!(ram, saved);
    for (base, len) in [(0u32, 0x3000), (0x10000, 0x2004), (0xfffffffe, 4)] {
        let mut bad = vec![0xa5; len];
        let saved = bad.clone();
        assert!(k.bind_imports(&image, base, &mut bad).is_err());
        assert_eq!(bad, saved);
    }
    assert!(KernelImage::build(
        HardwareInfo {
            flags: 0,
            gpu_revision: 0,
            mcp_revision: 0
        },
        KernelVersion {
            major: 0,
            minor: 0,
            build: 0,
            qfe: 0
        },
        &Budget::new((kernel::IMAGE_BYTES - 1) as u64)
    )
    .is_err());
}
fn fixture() -> String {
    let budget = Budget::new(32 * 1024 * 1024);
    let k = profile(&budget);
    let decoder = Decoder::new(&budget).unwrap();
    let pc = kernel::BASE + kernel::INIT_RVA;
    let decoded = decoder.linear(k.init_code(), pc, &[]).unwrap();
    let program = controlled_block(&decoded, pc, pc + 41, &budget).unwrap();
    include_str!("kernel_image_fixture.c")
        .replace("/* PROGRAM */", program.source())
        .replace(
            "/* IMAGE */",
            &k.bytes()
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(","),
        )
}
#[test]
fn export_kernel_image_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_KERNEL_IMAGE_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn virtual_kernel_init_executes_with_checked_sections_and_cpu() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let src = work.path().join("kernel.c");
    let bin = work.path().join("kernel");
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
    for n in [
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
        cc.arg(runtime.join(format!("gamepkg_x86_{n}.c")));
    }
    let built = cc.arg("-o").arg(&bin).output().unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
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
