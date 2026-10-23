use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::dispatch::controlled_block};
fn fixture() -> String {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let mut program = String::new();
    let mut entries = String::new();
    for (pc, mut code) in [
        (0x400000u32, vec![0x0f, 0xb6, 0x05, 0, 0x10, 0, 0]),
        (0x400100u32, vec![0xc6, 0x05, 0, 0x10, 0, 0, 7, 0x90]),
    ] {
        let branch = pc + code.len() as u32;
        code.push(0xe9);
        code.extend(0x700000u32.wrapping_sub(branch + 5).to_le_bytes());
        let decoded = decoder.linear(&code, pc, &[]).unwrap();
        let block = controlled_block(&decoded, pc, pc + code.len() as u32, &budget).unwrap();
        program.push_str(block.source());
        entries.push_str(&format!("{{{pc}u,gamepkg_cpu_block_{pc:08x}}},\n"));
    }
    include_str!("kernel_priority_fixture.c")
        .replace("/* PROGRAM */", &program)
        .replace("/* ENTRIES */", &entries)
}
#[test]
fn export_kernel_priority_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_KERNEL_PRIORITY_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn checked_kernel_priority_and_dpc_initialization() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let source = work.path().join("priority.c");
    let binary = work.path().join("priority");
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
    for name in [
        "irql",
        "dpc",
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
        cc.arg(runtime.join(format!("gamepkg_x86_{name}.c")));
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
