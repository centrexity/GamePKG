use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::integer_instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete flags fixture"
    );
    integer_instruction(&x.instructions()[0])
}
fn fixture() -> String {
    let mut c = String::from("#include <stdint.h>\n");
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['\"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    }
    c.push_str(include_str!("flags_fixture.c"));
    let mut wrappers = String::new();
    let mut names = vec![];
    let mut forms = vec![];
    for (kind, opcode) in [
        (0, 0x9f),
        (1, 0x9e),
        (2, 0xf8),
        (3, 0xf9),
        (4, 0xf5),
        (5, 0xfc),
        (6, 0xfd),
        (7, 0x9c),
        (8, 0x9d),
    ] {
        for prefix in [None, Some(0x66), Some(0x67)] {
            let mut bytes = vec![opcode];
            if let Some(p) = prefix {
                bytes.insert(0, p);
            }
            let code = emit(&bytes).unwrap();
            let name = format!("instruction_{}", names.len());
            wrappers.push_str(&format!("static void {name}(void){{{code}++completed;}}\n"));
            names.push(name);
            forms.push(format!(
                "{{{kind},{}}}",
                if prefix == Some(0x66) { 2 } else { 4 }
            ));
        }
    }
    c.replace("/* WRAPPERS */",&format!("{wrappers}\nstatic void (*instructions[])(void)={{{}}};\nstatic const unsigned forms[][2]={{{}}};",names.join(","),forms.join(",")))
}
#[test]
fn flags_reject_bad_prefixes_operands_and_encoding() {
    for bytes in [&[0xf0, 0x9c][..], &[0xf3, 0x9f], &[0x64, 0x9e]] {
        assert!(emit(bytes).is_err());
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0x9c], 0x10000, &[]).unwrap();
    let mut bad = x.instructions()[0].clone();
    bad.mnemonic = "popfd".into();
    assert!(integer_instruction(&bad).is_err());
    bad = x.instructions()[0].clone();
    bad.operands.push(Default::default());
    assert!(integer_instruction(&bad).is_err());
}
#[test]
fn export_flags_fixture() {
    let c = fixture();
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_FLAGS_FIXTURE") {
        std::fs::write(p, c).unwrap();
    }
}
#[cfg(target_os = "linux")]
#[test]
fn emitted_flags_preserve_images_and_fault_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("flags.c");
    let binary = dir.path().join("flags");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let out = std::process::Command::new("cc")
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
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = std::process::Command::new(binary).output().unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    print!("{}", String::from_utf8_lossy(&out.stdout));
}
