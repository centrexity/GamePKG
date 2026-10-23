use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::integer_instruction};

fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let decoded = decoder.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        decoded.instructions().len() == 1
            && usize::from(decoded.instructions()[0].size) == bytes.len(),
        "incomplete multiply fixture"
    );
    integer_instruction(&decoded.instructions()[0])
}

#[test]
fn multiply_rejects_unsupported_forms_prefixes_and_untrusted_expressions() {
    for bytes in [
        &[0xf0, 0xf7, 0x20][..],
        &[0xf3, 0x0f, 0xaf, 0xc1],
        &[0x65, 0xf7, 0x20],
    ] {
        assert!(emit(bytes).is_err());
    }
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let decoded = decoder.linear(&[0x0f, 0xaf, 0xc1], 0x10000, &[]).unwrap();
    let original = decoded.instructions()[0].clone();
    for index in 0..2 {
        let mut instruction = original.clone();
        instruction.operands[index].expr_src = Some("bad()".into());
        assert!(integer_instruction(&instruction).is_err());
    }
    let mut instruction = original.clone();
    instruction.operands[0].reg = Some("al".into());
    assert!(integer_instruction(&instruction).is_err());
    instruction = original.clone();
    instruction.operands[1].reg = Some("cx".into());
    assert!(integer_instruction(&instruction).is_err());
    instruction = original;
    instruction.operands.clear();
    assert!(integer_instruction(&instruction).is_err());
}

fn fixture() -> String {
    let mut c = String::from("#include <stdint.h>\n#include <inttypes.h>\n#include <assert.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <setjmp.h>\n");
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\nuint32_t xbox_MmioLoad(uint32_t, int);\nvoid xbox_MmioStore(uint32_t, uint32_t, int);\n");
    }
    c.push_str(
        r#"
RECOMP_TLS uint32_t g_eax,g_ebx,g_ecx,g_edx,g_esi,g_edi,g_esp,g_fs_base,g_eflags;
static unsigned char memory[4096];
static unsigned reads,writes;
static uint64_t checks;
static jmp_buf fault;
static unsigned fail_read;
uint32_t xbox_MmioLoad(uint32_t address,int width) {
    if(fail_read)longjmp(fault,1);
    assert(width==1||width==2||width==4);
    assert(address<=sizeof(memory)-(unsigned)width);++reads;
    uint32_t value=0;
    for(int i=0;i<width;++i)value|=(uint32_t)memory[address+(unsigned)i]<<(8u*(unsigned)i);
    return value;
}
void xbox_MmioStore(uint32_t address,uint32_t value,int width) {
    assert(width==1||width==2||width==4);
    assert(address<=sizeof(memory)-(unsigned)width);++writes;
    for(int i=0;i<width;++i)memory[address+(unsigned)i]=(unsigned char)(value>>(8u*(unsigned)i));
}
"#,
    );
    let mut functions = Vec::new();
    for size in [1u8, 2, 4] {
        let suffix = match size {
            1 => "b",
            2 => "w",
            _ => "l",
        };
        for (name, extension) in [("mul", 4), ("imul", 5)] {
            for variant in 0..if size == 1 { 5 } else { 4 } {
                let memory = variant == 1;
                let source = match variant {
                    0 => 0xc1,
                    1 => 6,
                    2 => 0xc0,
                    3 => 0xc2,
                    _ => 0xc4,
                };
                let bytes: Vec<_> = (size == 2)
                    .then_some(0x66)
                    .into_iter()
                    .chain([
                        if size == 1 { 0xf6 } else { 0xf7 },
                        (extension << 3) | source,
                    ])
                    .collect();
                let code = emit(&bytes).unwrap();
                let function = format!("{name}{size}_{variant}");
                let alias = match variant {
                    2 => "y=x;",
                    3 => "y=0xbed01234u;",
                    4 => "y=(x>>8u)&255u;",
                    _ => "",
                };
                functions.push((function.clone(), size));
                let setup = if memory {
                    format!("g_esi=0x301; xbox_MmioStore(g_esi,y,{size}); reads=writes=0;")
                } else {
                    "reads=writes=0;".into()
                };
                c.push_str(&format!(r#"
static void check_{function}(uint32_t x,uint32_t y,uint32_t prior) {{
    {alias}
    uint32_t real=x,high=0xbed01234u; uintptr_t flags;
    __asm__ volatile("{name}{suffix} %[source]\n\tpushf\n\tpop %[flags]"
        : [result] "+&a"(real),[high] "+&d"(high),[flags] "=&r"(flags)
        : [source] "m"(y) : "cc");
    g_eax=x;g_edx=0xbed01234u;g_ecx=y;g_eflags=prior;
    {setup}
    {code}
    if(g_eax!=real||g_edx!=high||g_ecx!=y||((g_eflags^(uint32_t)flags)&0x801u)!=0u||((g_eflags^prior)&~0x801u)!=0u) {{
        fprintf(stderr,"{function} x=%08x y=%08x actual=%08x:%08x expected=%08x:%08x flags=%08x expected=%08x\n",x,y,g_edx,g_eax,high,real,g_eflags,(uint32_t)flags);abort();
    }}
    assert(reads=={reads}u&&writes==0u);++checks;
}}
"#, reads = u8::from(memory)));
            }
        }
    }
    let mut truncated = Vec::new();
    for size in [2, 4] {
        let (suffix, reg) = if size == 2 { ("w", "w") } else { ("l", "k") };
        for memory in [false, true] {
            for immediate in [
                None,
                Some(-128i32),
                Some(-1),
                Some(0),
                Some(1),
                Some(127),
                Some(-32768),
                Some(32767),
                Some(i32::MIN),
                Some(i32::MAX),
            ] {
                if size == 2 && immediate.is_some_and(|n| !(-32768..=32767).contains(&n)) {
                    continue;
                }
                let mut bytes: Vec<_> = (size == 2).then_some(0x66).into_iter().collect();
                let operand = if memory { 6 } else { 0xc1 };
                let asm = if let Some(value) = immediate {
                    let short = (-128..=127).contains(&value);
                    bytes.extend([if short { 0x6b } else { 0x69 }, operand]);
                    if short {
                        bytes.push(value as u8);
                    } else if size == 2 {
                        bytes.extend((value as i16).to_le_bytes());
                    } else {
                        bytes.extend(value.to_le_bytes());
                    }
                    format!("imul{suffix} ${value},%[source],%{reg}[result]")
                } else {
                    bytes.extend([0x0f, 0xaf, operand]);
                    format!("imul{suffix} %[source],%{reg}[result]")
                };
                let code = emit(&bytes).unwrap();
                let function = format!(
                    "trunc{size}_{}_{index}",
                    u8::from(memory),
                    index = truncated.len()
                );
                truncated.push(function.clone());
                let setup = if memory {
                    format!("g_esi=0x301; xbox_MmioStore(g_esi,y,{size}); reads=writes=0;")
                } else {
                    "reads=writes=0;".into()
                };
                c.push_str(&format!(r#"
static void check_{function}(uint32_t x,uint32_t y,uint32_t prior) {{
    uint32_t real=x;uintptr_t flags;
    __asm__ volatile("{asm}\n\tpushf\n\tpop %[flags]"
        : [result] "+&a"(real),[flags] "=&r"(flags) : [source] "m"(y) : "cc");
    g_eax=x;g_ecx=y;g_edx=0xbed01234u;g_eflags=prior;
    {setup}
    {code}
    assert(g_eax==real&&g_ecx==y&&g_edx==0xbed01234u&&((g_eflags^(uint32_t)flags)&0x801u)==0u&&((g_eflags^prior)&~0x801u)==0u&&reads=={reads}u&&writes==0u);++checks;
}}
"#,reads=u8::from(memory)));
            }
        }
    }
    c.push_str("int main(void) {\nfor(uint32_t x=0;x<256;++x)for(uint32_t y=0;y<256;++y)for(unsigned image=0;image<2;++image){uint32_t prior=image?0x240ed7u:0x200202u;\n");
    for (function, size) in &functions {
        if *size == 1 {
            c.push_str(&format!(
                "check_{function}(0xa5c30000u|(y<<8u)|x,0x5ad30000u|y,prior);\n"
            ));
        }
    }
    c.push_str("}\nuint32_t edges[]={0,1,2,0x7f,0x80,0xff,0x100,0x7fff,0x8000,0xffff,0x10000,0x7fffffff,0x80000000,0xfffffffe,0xffffffff};uint32_t random=0x12345678u;\nfor(unsigned i=0;i<4321;++i){uint32_t x,y;if(i<225){x=edges[i/15];y=edges[i%15];}else{random=random*1664525u+1013904223u;x=random;random=random*1664525u+1013904223u;y=random;}for(unsigned image=0;image<2;++image){uint32_t prior=0x200202u|(image?0xcd5u:((x^y)&0x8d5u));\n");
    for (function, size) in &functions {
        if *size != 1 {
            c.push_str(&format!("check_{function}(x,y,prior);\n"));
        }
    }
    for function in truncated {
        c.push_str(&format!("check_{function}(x,y,prior);\n"));
    }
    c.push_str("}}\n");
    let extras: &[(&str, &[u8], &str)] = &[
        (
            "g_eax=0x12348080u;g_edx=7;g_eflags=0x246;",
            &[0xf6, 0xec],
            "assert(g_eax==0x12344000u&&g_edx==7&&(g_eflags&0x801u)==0x801u);",
        ),
        (
            "g_eax=0xffffffffu;g_edx=0x80000000u;g_eflags=0x246;",
            &[0xf7, 0xea],
            "assert(g_eax==0x80000000u&&g_edx==0&&(g_eflags&0x801u)==0x801u);",
        ),
        (
            "g_eax=0xa5a50003u;g_eflags=0x246;",
            &[0x66, 0x0f, 0xaf, 0xc0],
            "assert(g_eax==0xa5a50009u&&(g_eflags&0x801u)==0);",
        ),
        (
            "g_eax=0x300;g_fs_base=1;g_eflags=0x246;xbox_MmioStore(0x301,3,4);reads=writes=0;",
            &[0x64, 0x0f, 0xaf, 0x00],
            "assert(g_eax==0x900&&reads==1&&writes==0);",
        ),
        (
            "g_eax=7;g_esi=0xffff0301u;g_eflags=0x246;xbox_MmioStore(0x301,9,4);reads=writes=0;",
            &[0x67, 0xf7, 0x24],
            "assert(g_eax==63&&g_edx==0&&reads==1&&writes==0);",
        ),
    ];
    for (setup, bytes, expected) in extras {
        c.push_str(setup);
        c.push_str(&emit(bytes).unwrap());
        c.push_str(expected);
        c.push_str("++checks;\n");
    }
    for bytes in [
        vec![0xf6, 0x26],
        vec![0xf6, 0x2e],
        vec![0x66, 0xf7, 0x26],
        vec![0x66, 0xf7, 0x2e],
        vec![0xf7, 0x26],
        vec![0xf7, 0x2e],
        vec![0x66, 0x0f, 0xaf, 0x06],
        vec![0x0f, 0xaf, 0x06],
        vec![0x66, 0x6b, 0x06, 0x80],
        vec![0x6b, 0x06, 0x80],
    ] {
        c.push_str("g_eax=0x12345678;g_edx=0x98765432;g_esi=0x301;g_eflags=0x246;fail_read=1;if(setjmp(fault)==0){\n");
        c.push_str(&emit(&bytes).unwrap());
        c.push_str("abort();}assert(g_eax==0x12345678&&g_edx==0x98765432&&g_eflags==0x246);fail_read=0;++checks;\n");
    }
    c.push_str("printf(\"native multiply: %\" PRIu64 \" hardware/alias/fault checks passed\\n\",checks);return 0;}\n");
    c
}

#[test]
fn synthetic_multiply_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_MULTIPLY_FIXTURE") {
        std::fs::write(path, &c).unwrap();
    }
    assert!(c.contains("check_mul1_0"));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn generated_multiply_matches_hardware_products_and_defined_flags() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("multiply.c");
    let binary = dir.path().join("multiply");
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
