use gamepkg_takedawn_native_gen::kernel_data::{HardwareInfo, KernelVersion};
fn fixture() -> String {
    let mut flags = vec![0, u32::MAX];
    flags.extend((0..32).map(|bit| 1u32 << bit));
    let mut rows = String::new();
    for flags in flags {
        for (gpu_revision, mcp_revision) in [
            (0, 0),
            (0xa1, 0xb1),
            (255, 255),
            (0x12, 0x34),
            (0x80, 0x7f),
            (0x7f, 0x80),
        ] {
            let profile = HardwareInfo {
                flags,
                gpu_revision,
                mcp_revision,
            };
            let bytes = profile
                .to_le_bytes()
                .iter()
                .map(|byte| format!("0x{byte:02x}"))
                .collect::<Vec<_>>()
                .join(",");
            let version = KernelVersion {
                major: (flags >> 16) as u16,
                minor: flags as u16,
                build: ((gpu_revision as u16) << 8) | mcp_revision as u16,
                qfe: ((mcp_revision as u16) << 8) | gpu_revision as u16,
            };
            let version_bytes = version
                .to_le_bytes()
                .iter()
                .map(|byte| format!("0x{byte:02x}"))
                .collect::<Vec<_>>()
                .join(",");
            let (major, minor, build, qfe) =
                (version.major, version.minor, version.build, version.qfe);
            rows.push_str(&format!(
                "{{0x{flags:08x}u,0x{gpu_revision:02x},0x{mcp_revision:02x},{{{bytes}}},{{{major}u,{minor}u,{build}u,{qfe}u}},{{{version_bytes}}}}},\n"
            ));
        }
    }
    include_str!("hardware_info_fixture.c").replace("/* ROWS */", &rows)
}
#[test]
fn export_hardware_info_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_HARDWARE_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[cfg(target_os = "linux")]
#[test]
fn hardware_info_matches_public_c_abi_and_checked_readonly_mapping() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("hardware.c");
    let binary = directory.path().join("hardware");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let build = std::process::Command::new("cc")
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
        .arg(source)
        .arg(runtime.join("gamepkg_x86_fault.c"))
        .arg(runtime.join("gamepkg_x86_memory.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = std::process::Command::new(binary).output().unwrap();
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}
