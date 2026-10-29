use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::dispatch::controlled_block};
fn fixture() -> String {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let mut code = vec![0x8b, 0x01, 0x8b, 0x32, 0x8b, 0x3b, 0xe9];
    code.extend(
        0x700000u32
            .wrapping_sub(0x400000 + code.len() as u32 + 4)
            .to_le_bytes(),
    );
    let decoded = decoder.linear(&code, 0x400000, &[]).unwrap();
    let block =
        controlled_block(&decoded, 0x400000, 0x400000 + code.len() as u32, &budget).unwrap();
    include_str!("nv_fixture.c").replace("/* PROGRAM */", block.source())
}
#[test]
fn export_nv_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_NV_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn immutable_virtual_console_settings_and_checked_queries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let source = work.path().join("nv.c");
    let binary = work.path().join("nv");
    std::fs::write(&source, fixture()).unwrap();
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
    .arg(source);
    for n in [
        "nv",
        "irql",
        "cpu",
        "interrupt",
        "cpu_dispatch",
        "dispatch",
        "far_memory",
        "far_jump",
        "memory",
        "fault",
        "table_store",
        "x87",
    ] {
        cc.arg(runtime.join(format!("gamepkg_x86_{n}.c")));
    }
    let output = cc.arg("-o").arg(&binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}
