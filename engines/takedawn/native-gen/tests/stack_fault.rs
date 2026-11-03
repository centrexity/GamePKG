use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::data_instruction};

fn fixture() -> String {
    let mut c = String::from("#define _GNU_SOURCE 1\n#include <stdint.h>\n");
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['\"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    }
    c.push_str(include_str!("stack_fault_fixture.c"));
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let mut wrappers = String::new();
    let mut names = vec![];
    let mut descriptions = vec![];
    for (kind, bytes) in [
        (0u32, &[0x54][..]),            // PUSH ESP
        (1, &[0xff, 0x34, 0x24]),       // PUSH [ESP]
        (2, &[0x58]),                   // POP EAX
        (3, &[0x5c]),                   // POP ESP
        (4, &[0x8f, 0x04, 0x24]),       // POP [ESP] uses advanced ESP for address only
        (5, &[0x8f, 0x44, 0x84, 0xfc]), // POP [ESP+EAX*4-4]
        (6, &[0x64, 0x8f, 0x04, 0x24]), // FS:POP [ESP]
        (7, &[0xc9]),                   // LEAVE
    ] {
        for word in [false, true] {
            let mut bytes = bytes.to_vec();
            if word {
                bytes.insert(0, 0x66);
            }
            let decoded = decoder.linear(&bytes, 0x10000, &[]).unwrap();
            assert_eq!(decoded.instructions().len(), 1);
            let code = data_instruction(&decoded.instructions()[0]).unwrap();
            let name = format!("instruction_{}", names.len());
            wrappers.push_str(&format!(
                "static void {name}(void){{uint32_t ebp=work_bp;{code}work_bp=ebp;++completed;}}\n"
            ));
            names.push(name);
            descriptions.push(format!("{{{kind}, {}}}", if word { 2 } else { 4 }));
        }
    }
    c=c.replace("/* WRAPPERS */",&format!("{wrappers}\nstatic void (*instructions[])(void)={{{}}};\nstatic const unsigned forms[][2]={{{}}};",names.join(","),descriptions.join(",")));
    c
}
#[test]
fn stack_fault_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_STACK_FAULT_FIXTURE") {
        std::fs::write(path, c).unwrap();
    }
}
#[cfg(target_os = "linux")]
#[test]
fn generated_stack_updates_commit_only_after_memory_success() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("stack.c");
    let binary = dir.path().join("stack");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let result = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pthread",
            "-fsanitize=undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg("-I")
        .arg(&runtime)
        .arg(source)
        .arg(runtime.join("gamepkg_x86_fault.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = std::process::Command::new(binary).output().unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    print!("{}", String::from_utf8_lossy(&result.stdout));
}
