use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::dispatch::controlled_block};
fn fixture() -> String {
    let b = Budget::new(32 * 1024 * 1024);
    let d = Decoder::new(&b).unwrap();
    let mut program = String::new();
    let mut entries = String::new();
    for i in 0..4u32 {
        let pc = 0x400000 + i * 0x40;
        let code = if i == 3 {
            vec![0xa1, 0, 0xc0, 0xad, 0xde, 0xc2, 4, 0]
        } else {
            let mut v = vec![0x8b, 0x44, 0x24, 4, 0xc7, 0x40, 4];
            v.extend((0xaabb0000 + i).to_le_bytes());
            v.extend([0xc2, 4, 0]);
            v
        };
        let decoded = d.linear(&code, pc, &[]).unwrap();
        let p = controlled_block(&decoded, pc, pc + code.len() as u32, &b).unwrap();
        program.push_str(p.source());
        entries.push_str(&format!("{{{pc}u,gamepkg_cpu_block_{pc:08x}}},\n"));
    }
    include_str!("shutdown_fixture.c")
        .replace("/* PROGRAM */", &program)
        .replace("/* ENTRIES */", &entries)
}
#[test]
fn export_shutdown_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_SHUTDOWN_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn shutdown_registration_and_checked_callbacks_execute() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("shutdown.c");
    let bin = dir.path().join("shutdown");
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
        "shutdown",
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
