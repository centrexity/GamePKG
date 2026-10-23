use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::integer_instruction};

fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let decoded = decoder.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        decoded.instructions().len() == 1
            && usize::from(decoded.instructions()[0].size) == bytes.len(),
        "incomplete ALU fixture"
    );
    integer_instruction(&decoded.instructions()[0])
}

#[test]
fn unsupported_arithmetic_is_rejected_without_partial_c() {
    for bytes in [
        &[0xf0, 0x01, 0x00][..], // lock memory ADD requires atomic RMW
        &[0x0f, 0xc7, 0x0f],     // CMPXCHG8B needs a separate 64-bit transaction
        &[0x65, 0x01, 0x00],     // GS model is not present
    ] {
        assert!(emit(bytes).is_err());
    }
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let decoded = decoder.linear(&[0x01, 0xc8], 0x10000, &[]).unwrap();
    let mut instruction = decoded.instructions()[0].clone();
    instruction.operands[1].expr_src = Some("bad()".into());
    assert!(integer_instruction(&instruction).is_err());
    instruction.operands.clear();
    assert!(integer_instruction(&instruction).is_err());
}

// Synthetic public instruction fixtures, not game-derived code. The hardware
// oracle executes actual x86 operations instead of repeating flag formulas.
fn fixture() -> String {
    let mut c = String::from("#include <stdint.h>\n#include <inttypes.h>\n#include <assert.h>\n#include <stdio.h>\n#include <stdlib.h>\n");
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\nuint32_t xbox_MmioLoad(uint32_t, int);\nvoid xbox_MmioStore(uint32_t, uint32_t, int);\n");
    }
    c.push_str(r#"
RECOMP_TLS uint32_t g_eax, g_ebx, g_ecx, g_edx, g_esi, g_edi, g_esp, g_fs_base, g_eflags;
static unsigned char memory[4096];
static unsigned reads, writes;
static uint64_t checks;
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
"#);
    let binary = [
        ("add", 0x00),
        ("adc", 0x10),
        ("sub", 0x28),
        ("sbb", 0x18),
        ("cmp", 0x38),
        ("test", 0x84),
        ("and", 0x20),
        ("or", 0x08),
        ("xor", 0x30),
    ];
    let unary = [
        ("inc", 0xfe, 0xc0),
        ("dec", 0xfe, 0xc8),
        ("neg", 0xf6, 0xd8),
        ("not", 0xf6, 0xd0),
    ];
    for width in [1, 2, 4] {
        for (op, opcode, modrm, is_unary) in binary
            .iter()
            .map(|&(op, code)| (op, code, 0xc8, false))
            .chain(
                unary
                    .iter()
                    .map(|&(op, code, modrm)| (op, code, modrm, true)),
            )
        {
            let bytes: Vec<_> = (width == 2)
                .then_some(0x66)
                .into_iter()
                .chain([opcode + u8::from(width != 1), modrm])
                .collect();
            let emitted = emit(&bytes).unwrap();
            let (suffix, reg) = match width {
                1 => ("b", "b"),
                2 => ("w", "w"),
                _ => ("l", "k"),
            };
            let asm = if is_unary {
                format!("{op}{suffix} %{reg}[result]")
            } else {
                format!("{op}{suffix} %{reg}[y], %{reg}[result]")
            };
            let changed = match op {
                "not" => 0,
                "inc" | "dec" => 0x8d4,
                "and" | "or" | "xor" | "test" => 0x8c5,
                _ => 0x8d5,
            };
            let compared = if matches!(op, "inc" | "dec") {
                0x8d5
            } else {
                changed
            };
            c.push_str(&format!(r#"
static void check_{op}{width}(uint32_t x, uint32_t y, uint32_t carry) {{
    uint32_t real = x;
    uintptr_t flags;
    (void)y;
    __asm__ volatile("btl $0, %[carry]\n\t{asm}\n\tpushf\n\tpop %[flags]"
        : [result] "+&a"(real), [flags] "=&r"(flags)
        : [y] "q"(y), [carry] "r"(carry) : "cc");
    g_eax = x; g_ecx = y;
    uint32_t before = 0x00240ed6u | carry;
    g_eflags = before;
    {emitted}
    if (g_eax != real || ((g_eflags ^ (uint32_t)flags) & 0x{compared:x}u) != 0u ||
        ((g_eflags ^ before) & ~0x{changed:x}u) != 0u || g_ecx != y) {{
        fprintf(stderr, "{op}{width} x=%08x y=%08x carry=%u result=%08x hardware=%08x flags=%08x hardware=%08x\n",
            x,y,carry,g_eax,real,g_eflags,(uint32_t)flags);
        abort();
    }}
    ++checks;
}}
"#));
        }
    }
    c.push_str("int main(void) {\n");
    c.push_str("for (uint32_t x=0; x<256; ++x) for (uint32_t y=0; y<256; ++y) for (uint32_t carry=0; carry<2; ++carry) {\n");
    for (op, _) in binary {
        c.push_str(&format!(
            "check_{op}1(0xa5c30000u|x, 0x5ac30000u|y, carry);\n"
        ));
    }
    c.push_str("}\nfor (uint32_t x=0; x<256; ++x) for (uint32_t carry=0; carry<2; ++carry) {\n");
    for (op, _, _) in unary {
        c.push_str(&format!("check_{op}1(0xa5c30000u|x, 0, carry);\n"));
    }
    c.push_str(r#"}
uint32_t edge[] = {0,1,15,16,0x7f,0x80,0xff,0x100,0x7fff,0x8000,0xffff,0x10000,0x7fffffff,0x80000000,0xfffffffe,0xffffffff};
uint32_t random = 0x12345678;
for (unsigned i=0; i<4352; ++i) {
    uint32_t x, y;
    if (i<256) {x=edge[i/16]; y=edge[i%16];}
    else { random = random*1664525u+1013904223u; x=random;
           random = random*1664525u+1013904223u; y=random; }
    for (uint32_t carry=0; carry<2; ++carry) {
"#);
    for width in [2, 4] {
        for op in binary.iter().map(|x| x.0).chain(unary.iter().map(|x| x.0)) {
            c.push_str(&format!("check_{op}{width}(x,y,carry);\n"));
        }
    }
    c.push_str("}}\n");
    let memory_cases: &[(&str,&[u8],&str)] = &[
        ("g_esp=0x300; g_eax=7; g_eflags=2; xbox_MmioStore(0x304,0xffffffffu,4); reads=writes=0;",
        &[0x01,0x44,0x24,4], "assert(xbox_MmioLoad(0x304,4)==6 && g_eax==7 && (g_eflags&1)==1 && reads==2 && writes==1);"),
        ("g_eax=0x7fffffffu; g_eflags=3;", &[0x11,0xc0], "assert(g_eax==0xffffffffu && (g_eflags&0x801)==0x800);"),
        ("g_eax=0x300; xbox_MmioStore(0x300,0x300,4); g_eflags=2; reads=writes=0;",
        &[0x39,0x00], "assert(g_eax==0x300 && (g_eflags&0x40)!=0 && writes==0 && reads==1);"),
        ("g_fs_base=0x400; xbox_MmioStore(0x420,0xff,1); g_eflags=3; reads=writes=0;",
        &[0x64,0xfe,0x05,0x20,0,0,0], "assert(xbox_MmioLoad(0x420,1)==0 && (g_eflags&0x55)==0x55 && reads==2 && writes==1);"),
        ("g_eax=0xabcdff01u; g_eflags=2;", &[0x00,0xe0], "assert(g_eax==0xabcdff00u && (g_eflags&0x51)==0x51);"),
    ];
    for (setup, bytes, expected) in memory_cases {
        c.push_str(setup);
        c.push_str(&emit(bytes).unwrap());
        c.push_str(expected);
        c.push('\n');
    }
    c.push_str("printf(\"native ALU: %\" PRIu64 \" hardware comparisons and 5 memory/alias cases passed\\n\",checks); return 0; }\n");
    c
}

#[test]
fn synthetic_hardware_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_ALU_FIXTURE") {
        std::fs::write(path, &c).unwrap();
    }
    assert!(c.contains("check_sbb4"));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn generated_arithmetic_matches_actual_x86_results_and_defined_flags() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("alu.c");
    let binary = dir.path().join("alu");
    std::fs::write(&source, fixture()).unwrap();
    let output = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O3",
            "-mno-red-zone",
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
    print!("{}", String::from_utf8_lossy(&output.stdout));
}
