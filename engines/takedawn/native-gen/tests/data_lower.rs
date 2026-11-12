use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::data_instruction};

fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget)?;
    let decoded = decoder.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        decoded
            .instructions()
            .iter()
            .map(|i| usize::from(i.size))
            .sum::<usize>()
            == bytes.len(),
        "undecoded fixture bytes"
    );
    decoded
        .instructions()
        .iter()
        .map(data_instruction)
        .collect()
}

#[test]
fn rejects_unimplemented_forms_and_untrusted_text_is_never_c() {
    for bytes in [
        &[0x01, 0xc0][..],   // ADD: flags not implemented yet
        &[0xe8, 0, 0, 0, 0], // CALL: requires function/ABI lowering
        &[0x65, 0x87, 0x06], // GS memory XCHG needs a segment model
        &[0x8c, 0xd8],       // segment register read
        &[0x65, 0x8b, 0x00], // GS needs a segment model
        &[0xf3, 0xa4],       // REP needs string-operation lowering
    ] {
        assert!(emit(bytes).is_err(), "{bytes:x?}");
    }
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let decoded = decoder.linear(&[0x89, 0xd8], 0x10000, &[]).unwrap();
    let mut instruction = decoded.instructions()[0].clone();
    let original = data_instruction(&instruction).unwrap();
    instruction.op_str = "system(\"bad\");".into();
    assert_eq!(data_instruction(&instruction).unwrap(), original);
    instruction.operands[0].expr = Some("injected()".into());
    assert!(data_instruction(&instruction).is_err());
    instruction.operands[0].expr = None;
    instruction.operands[0].reg = Some("eax; injected()".into());
    assert!(data_instruction(&instruction).is_err());
    instruction.operands[0].reg = Some("eax".into());
    instruction.bytes_hex = "66".into();
    assert!(data_instruction(&instruction).is_err());
    instruction.size = 1;
    assert!(data_instruction(&instruction).is_err());
    let decoded = decoder.linear(&[0x8b, 0x00], 0x10000, &[]).unwrap();
    let original = decoded.instructions()[0].clone();
    for size in [0, 3, 8, 16] {
        let mut bad = original.clone();
        bad.operands[1].mem_size = size;
        assert!(data_instruction(&bad).is_err());
    }
    for scale in [-1, 0, 3, 16] {
        let mut bad = original.clone();
        bad.operands[1].mem_scale = scale;
        assert!(data_instruction(&bad).is_err());
    }
    for displacement in [i64::MIN, i64::MAX] {
        let mut bad = original.clone();
        bad.operands[1].mem_disp = displacement;
        assert!(data_instruction(&bad).is_err());
    }
    let mut bad = original;
    bad.operands[1].mem_base = Some("eax); injected(); (".into());
    assert!(data_instruction(&bad).is_err());
}

fn fixture() -> String {
    let mut c = String::from("#include <stdint.h>\n#include <assert.h>\n#include <stdio.h>\n");
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\nuint32_t xbox_MmioLoad(uint32_t, int);\nvoid xbox_MmioStore(uint32_t, uint32_t, int);\n");
    }
    c.push_str(r#"
RECOMP_TLS uint32_t g_eax, g_ebx, g_ecx, g_edx, g_esi, g_edi, g_esp, g_fs_base, g_eflags;
static unsigned char memory[131072];
static unsigned reads, writes;
uint32_t xbox_MmioLoad(uint32_t address, int width) {
    assert(width == 1 || width == 2 || width == 4);
    assert(address <= sizeof(memory) - (unsigned)width);
    ++reads;
    uint32_t value = 0;
    for (int i = 0; i < width; ++i) value |= (uint32_t)memory[address + (unsigned)i] << (8u * (unsigned)i);
    return value;
}
void xbox_MmioStore(uint32_t address, uint32_t value, int width) {
    assert(width == 1 || width == 2 || width == 4);
    assert(address <= sizeof(memory) - (unsigned)width);
    ++writes;
    for (int i = 0; i < width; ++i) memory[address + (unsigned)i] = (unsigned char)(value >> (8u * (unsigned)i));
}
int main(void) {
    uint32_t ebp = 0;
    g_eflags = 0x00200ad7u;
"#);
    let cases: &[(&str, &[u8], &str)] = &[
        (
            "g_eax = 0xaabbccddu;",
            &[0xb4, 0x12, 0x86, 0xe0],
            "assert(g_eax == 0xaabbdd12u);",
        ),
        (
            "g_eax = 0x000080ffu; g_ecx = 0;",
            &[0x0f, 0xbe, 0xcc, 0x0f, 0xb7, 0xd0],
            "assert(g_ecx == 0xffffff80u && g_edx == 0x80ffu);",
        ),
        (
            "g_eax = 0x11223344u; g_esi = 100; g_edi = 2;",
            &[0x89, 0x44, 0xbe, 0xfc, 0x8b, 0x4c, 0xbe, 0xfc],
            "assert(g_ecx == g_eax && memory[104] == 0x44 && memory[107] == 0x11);",
        ),
        (
            "g_fs_base = 0x1000; xbox_MmioStore(0x1020, 0x87654321u, 4);",
            &[0x64, 0xa1, 0x20, 0, 0, 0],
            "assert(g_eax == 0x87654321u);",
        ),
        (
            "g_eax = 0xfffffffdu; reads = writes = 0;",
            &[0x64, 0x8d, 0x40, 4],
            "assert(g_eax == 1 && reads == 0 && writes == 0);",
        ),
        (
            "g_ebx = 0xfffffff0u; g_esi = 0x12340030u;",
            &[0x67, 0x8d, 0x40, 0x20],
            "assert(g_eax == 0x40);",
        ),
        (
            "xbox_MmioStore(0x1040, 0x3456789au, 4);",
            &[0x64, 0x67, 0x8b, 0x40, 0x20],
            "assert(g_eax == 0x3456789au);",
        ),
        (
            "g_esp = 0x2000;",
            &[0x54, 0x5c],
            "assert(g_esp == 0x2000 && xbox_MmioLoad(0x1ffc, 4) == 0x2000);",
        ),
        (
            "g_esp = 0x2000; xbox_MmioStore(0x2000, 0x33445566, 4);",
            &[0xff, 0x34, 0x24],
            "assert(g_esp == 0x1ffc && xbox_MmioLoad(0x1ffc, 4) == 0x33445566);",
        ),
        (
            "g_esp = 0x2000; xbox_MmioStore(0x2000, 0x778899aa, 4); xbox_MmioStore(0x2004, 0, 4);",
            &[0x8f, 0x04, 0x24],
            "assert(g_esp == 0x2004 && xbox_MmioLoad(0x2004, 4) == 0x778899aa);",
        ),
        (
            "g_esp = 0x10002;",
            &[0x66, 0x54, 0x66, 0x5c],
            "assert(g_esp == 0x10002 && xbox_MmioLoad(0x10000, 2) == 2);",
        ),
        (
            "g_esp = 0x2000;",
            &[0x6a, 0x80, 0x58],
            "assert(g_eax == 0xffffff80u && g_esp == 0x2000);",
        ),
        (
            "g_esp = 0x2000; g_eax = 0xabcd0000u;",
            &[0x66, 0x6a, 0x80, 0x66, 0x58],
            "assert(g_eax == 0xabcdff80u && g_esp == 0x2000);",
        ),
        (
            "g_eax = 0x12345678;",
            &[0x0f, 0xc8],
            "assert(g_eax == 0x78563412);",
        ),
        (
            "g_eax = 0x80000000u;",
            &[0x99],
            "assert(g_edx == 0xffffffffu);",
        ),
        ("g_eax = 0x7fffffffu;", &[0x99], "assert(g_edx == 0);"),
        (
            "g_eax = 0xaabb8000u; g_edx = 0x12345678;",
            &[0x66, 0x99],
            "assert(g_edx == 0x1234ffff);",
        ),
        (
            "g_eax = 0xabcd0080u;",
            &[0x66, 0x98, 0x98],
            "assert(g_eax == 0xffffff80u);",
        ),
        (
            "ebp = 0x3000; g_esp = 0x4000; xbox_MmioStore(0x3000, 0x01020304, 4);",
            &[0xc9],
            "assert(ebp == 0x01020304 && g_esp == 0x3004);",
        ),
        (
            "ebp = 0x12340; g_esp = 0x4000; xbox_MmioStore(0x12340, 0xabcd, 2);",
            &[0x66, 0xc9],
            "assert(ebp == 0x1abcd && g_esp == 0x12342);",
        ),
        (
            "g_eax = 0xaabbccddu; g_esi = 0x3000;",
            &[0x88, 0x26, 0x66, 0xc7, 0x46, 2, 0x34, 0x12, 0x8a, 0x06],
            "assert(g_eax == 0xaabbccccu && xbox_MmioLoad(0x3002, 2) == 0x1234);",
        ),
    ];
    for (setup, bytes, expected) in cases {
        c.push_str(setup);
        c.push('\n');
        c.push_str(&emit(bytes).unwrap());
        c.push_str(expected);
        c.push_str("\nassert(g_eflags == 0x00200ad7u);\n");
    }
    c.push_str("puts(\"native data lowering: 21 architectural cases passed\"); return 0; }\n");
    c
}

#[test]
fn synthetic_fixture_can_be_written_for_cross_compilation() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_DATA_FIXTURE") {
        std::fs::write(path, &c).unwrap();
    }
    assert!(c.contains("21 architectural cases passed"));
}

#[cfg(target_os = "linux")]
#[test]
fn generated_c_executes_register_memory_and_stack_cases() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("data.c");
    let binary = dir.path().join("data");
    std::fs::write(&source, fixture()).unwrap();
    let output = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-fsanitize=undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
