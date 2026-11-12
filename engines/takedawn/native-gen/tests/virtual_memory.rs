use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::dispatch::controlled_block};
fn fixture() -> String {
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let d = decoder
        .linear(&[0x8b, 0x01, 0x89, 0x02, 0xc3], 0x400000, &[])
        .unwrap();
    let b = controlled_block(&d, 0x400000, 0x400005, &budget).unwrap();
    include_str!("virtual_memory_fixture.c").replace("/* PROGRAM */", b.source())
}
#[test]
fn export_virtual_memory_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_VIRTUAL_MEMORY_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn nt_memory_calls_preserve_reservation_and_fault_semantics() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let work = tempfile::tempdir().unwrap();
    let c = work.path().join("nt.c");
    let binary = work.path().join("nt");
    std::fs::write(&c, fixture()).unwrap();
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
    .arg(root.join("runtime"))
    .arg(c);
    for name in [
        "virtual_memory",
        "irql",
        "memory",
        "fault",
        "x87",
        "cpu",
        "interrupt",
        "cpu_dispatch",
        "cpu_code",
        "dispatch",
        "far_memory",
        "far_jump",
        "table_store",
    ] {
        cc.arg(root.join(format!("runtime/gamepkg_x86_{name}.c")));
    }
    let o = cc.arg("-o").arg(&binary).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = std::process::Command::new(binary).output().unwrap();
    assert!(
        o.status.success(),
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    print!("{}", String::from_utf8_lossy(&o.stdout));
}
