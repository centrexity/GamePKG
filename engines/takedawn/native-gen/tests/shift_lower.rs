use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let decoded = decoder.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        decoded.instructions().len() == 1
            && usize::from(decoded.instructions()[0].size) == bytes.len(),
        "incomplete shift fixture"
    );
    instruction(&decoded.instructions()[0])
}
#[test]
fn count_forms_widths_prefixes_and_synthetic_expressions_are_checked() {
    for bytes in [
        &[0xf0, 0xd3, 0x20][..],
        &[0x65, 0xd3, 0x20],
        &[0x66, 0x0f, 0xa5, 0xd0], // dynamic word SHLD includes undefined counts
        &[0x66, 0x0f, 0xac, 0xd0, 17], // undefined word result
    ] {
        assert!(emit(bytes).is_err());
    }
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let decoded = decoder.linear(&[0xd3, 0xe0], 0x10000, &[]).unwrap();
    let original = decoded.instructions()[0].clone();
    let mut bad = original.clone();
    bad.operands[1].reg = Some("cx".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.operands[1].expr = Some("bad()".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.operands[0].reg = Some("eax;bad()".into());
    assert!(instruction(&bad).is_err());
    let mut alias = original.clone();
    alias.mnemonic = "sal".into();
    assert_eq!(
        instruction(&original).unwrap(),
        instruction(&alias).unwrap()
    );
}
fn fixture() -> String {
    let mut c=String::from("#define _GNU_SOURCE\n#include <stdint.h>\n#include <inttypes.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <assert.h>\n#include <string.h>\n#if defined(__linux__)\n#include <sys/mman.h>\n#include <sys/wait.h>\n#include <sys/resource.h>\n#include <unistd.h>\n#include <signal.h>\n#endif\n");
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let path = path.to_str().unwrap();
        assert!(!path.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{path}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\nuint32_t xbox_MmioLoad(uint32_t,int);\nvoid xbox_MmioStore(uint32_t,uint32_t,int);\n");
    }
    c.push_str(r#"
RECOMP_TLS uint32_t g_eax,g_ebx,g_ecx,g_edx,g_esi,g_edi,g_esp,g_ebp,g_fs_base,g_eflags;
static unsigned char memory[1024];
static unsigned reads,writes;
static uint64_t checks;
uint32_t xbox_MmioLoad(uint32_t address,int width) {
    assert((width==1||width==2||width==4) && address<=sizeof(memory)-(unsigned)width);++reads;
    uint32_t value=0;for(int i=0;i<width;++i)value|=(uint32_t)memory[address+(unsigned)i]<<(8u*(unsigned)i);
    return value;
}
void xbox_MmioStore(uint32_t address,uint32_t value,int width) {
    assert((width==1||width==2||width==4) && address<=sizeof(memory)-(unsigned)width);++writes;
    for(int i=0;i<width;++i)memory[address+(unsigned)i]=(unsigned char)(value>>(8u*(unsigned)i));
}
"#);
    let operations = [
        ("shl", 4),
        ("shr", 5),
        ("sar", 7),
        ("rol", 0),
        ("ror", 1),
        ("rcl", 2),
        ("rcr", 3),
    ];
    let mut functions = Vec::new();
    for width in [1u8, 2, 4] {
        for (index, (name, extension)) in operations.iter().enumerate() {
            let bytes: Vec<_> = (width == 2)
                .then_some(0x66)
                .into_iter()
                .chain([
                    if width == 1 { 0xd2 } else { 0xd3 },
                    0xc0 + (extension << 3),
                ])
                .collect();
            let code = emit(&bytes).unwrap();
            let (suffix, reg) = match width {
                1 => ("b", "b"),
                2 => ("w", "w"),
                _ => ("l", "k"),
            };
            let shift = index < 3;
            let defined = if shift {
                format!(
                    "(c==0u?0x8d5u:(0xc4u | ((c<{}u || {})?1u:0u) | (c==1u?0x800u:0u)))",
                    width * 8,
                    u8::from(*name == "sar")
                )
            } else {
                "(c==0u?0x8d5u:(0xd5u | (c==1u?0x800u:0u)))".into()
            };
            let changed = if shift {
                format!(
                    "(c==0u?0u:(0xc4u | ((c<{}u || {})?1u:0u) | (c==1u?0x800u:0u)))",
                    width * 8,
                    u8::from(*name == "sar")
                )
            } else {
                "(c==0u?0u:(1u | (c==1u?0x800u:0u)))".into()
            };
            let function = format!("{name}{width}");
            functions.push((function.clone(), width));
            c.push_str(&format!(r#"
static void check_{function}(uint32_t x,uint32_t raw,uint32_t carry,uint32_t overflow) {{
    uint32_t real=x,count=0x12340000u|raw,c=raw&31u;
    uint32_t before=0x200202u|((x^raw)&0xd4u)|carry|(overflow<<11u);
    uintptr_t image=before,hardware;
    __asm__ volatile("push %[image]\n\tpopf\n\t{name}{suffix} %%cl,%{reg}[result]\n\tpushf\n\tpop %[flags]"
        : [result] "+&a"(real),[flags] "=&r"(hardware)
        : [count] "c"(count),[image] "r"(image) : "cc");
    g_eax=x;g_ecx=count;g_eflags=before;
    {code}
    uint32_t defined={defined},changed={changed};
    if(g_eax!=real || ((g_eflags^(uint32_t)hardware)&defined)!=0u || ((g_eflags^before)&~changed)!=0u || g_ecx!=count) {{
        fprintf(stderr,"{function}: x=%08x raw=%u CF=%u OF=%u actual=%08x expected=%08x flags=%08x expectedflags=%08x mask=%08x\n",
            x,raw,carry,overflow,g_eax,real,g_eflags,(uint32_t)hardware,defined);abort();
    }}++checks;
}}
"#));
        }
    }
    let mut doubles = Vec::new();
    for name in ["shld", "shrd"] {
        for width in [2u8, 4] {
            let counts: Vec<Option<u8>> = if width == 4 {
                vec![None]
            } else {
                [0, 1, 7, 8, 15, 16, 32, 33, 48, 64]
                    .into_iter()
                    .map(Some)
                    .collect()
            };
            for (index, count) in counts.into_iter().enumerate() {
                let left = name == "shld";
                let mut bytes: Vec<_> = (width == 2)
                    .then_some(0x66)
                    .into_iter()
                    .chain([
                        0x0f,
                        match (left, count.is_some()) {
                            (true, true) => 0xa4,
                            (true, false) => 0xa5,
                            (false, true) => 0xac,
                            _ => 0xad,
                        },
                        0xd0,
                    ])
                    .collect();
                if let Some(count) = count {
                    bytes.push(count);
                }
                let code = emit(&bytes).unwrap();
                let function = format!("{name}{width}_{index}");
                doubles.push((function.clone(), count));
                let (suffix, reg) = if width == 2 { ("w", "w") } else { ("l", "k") };
                let argument = count
                    .map(|n| format!("$0x{n:x}"))
                    .unwrap_or_else(|| "%%cl".into());
                c.push_str(&format!(r#"
static void check_{function}(uint32_t x,uint32_t y,uint32_t raw,uint32_t carry,uint32_t overflow) {{
    uint32_t real=x,count=0x12340000u|raw,c=({effective})&31u;
    uint32_t before=0x200202u|((x^raw)&0xd4u)|carry|(overflow<<11u);
    uintptr_t image=before,hardware;
    __asm__ volatile("push %[image]\n\tpopf\n\t{name}{suffix} {argument},%{reg}[source],%{reg}[result]\n\tpushf\n\tpop %[flags]"
        : [result] "+&a"(real),[flags] "=&r"(hardware)
        : [source] "d"(y),[count] "c"(count),[image] "r"(image) : "cc");
    g_eax=x;g_edx=y;g_ecx=count;g_eflags=before;
    {code}
    uint32_t defined=c==0u?0x8d5u:(0xc5u|(c==1u?0x800u:0u));
    uint32_t changed=c==0u?0u:(0xc5u|(c==1u?0x800u:0u));
    assert(g_eax==real && g_edx==y && g_ecx==count && ((g_eflags^(uint32_t)hardware)&defined)==0u && ((g_eflags^before)&~changed)==0u);++checks;
}}
"#,effective=count.map(|n|format!("{n}u")).unwrap_or_else(||"raw".into())));
            }
        }
    }
    c.push_str("int main(void) {\nfor(uint32_t x=0;x<256;++x)for(uint32_t c=0;c<256;++c)for(uint32_t cf=0;cf<2;++cf)for(uint32_t of=0;of<2;++of){\n");
    for (name, width) in &functions {
        if *width == 1 {
            c.push_str(&format!("check_{name}(0xabcdef00u|x,c,cf,of);\n"));
        }
    }
    c.push_str("}\nuint32_t edges[]={0,1,2,15,16,0x7f,0x80,0xff,0x100,0x7fff,0x8000,0xffff,0x7fffffff,0x80000000,0xfffffffe,0xffffffff};\nuint32_t random=0x12345678;\nfor(unsigned i=0;i<144;++i){random=random*1664525u+1013904223u;uint32_t x=i<16?edges[i]:random;random=random*1664525u+1013904223u;uint32_t y=random;\nfor(uint32_t c=0;c<256;++c)for(uint32_t cf=0;cf<2;++cf)for(uint32_t of=0;of<2;++of){\n");
    for (name, width) in &functions {
        if *width != 1 {
            c.push_str(&format!("check_{name}(x,c,cf,of);\n"));
        }
    }
    for (name, count) in &doubles {
        if count.is_none() {
            c.push_str(&format!("check_{name}(x,y,c,cf,of);\n"));
        }
    }
    c.push_str("}\nfor(uint32_t cf=0;cf<2;++cf)for(uint32_t of=0;of<2;++of){\n");
    for (name, count) in &doubles {
        if count.is_some() {
            c.push_str(&format!("check_{name}(x,y,255,cf,of);\n"));
        }
    }
    c.push_str("}}\n");
    // Memory counts0/32 still perform a read/write; addresses may be unaligned.
    for width in [1u8, 2, 4] {
        for (kind, (name, extension)) in operations.iter().enumerate() {
            let bytes: Vec<_> = (width == 2)
                .then_some(0x66)
                .into_iter()
                .chain([
                    if width == 1 { 0xd2 } else { 0xd3 },
                    0x06 + (extension << 3),
                ])
                .collect();
            for count in [0, 1, 8, 16, 31, 32, 255] {
                let (suffix, _) = match width {
                    1 => ("b", "b"),
                    2 => ("w", "w"),
                    _ => ("l", "k"),
                };
                let masked = count & 31;
                let defined = if masked == 0 {
                    0x8d5
                } else if kind >= 3 {
                    0xd5 | if masked == 1 { 0x800 } else { 0 }
                } else {
                    0xc4 | if masked < u32::from(width) * 8 || kind == 2 {
                        1
                    } else {
                        0
                    } | if masked == 1 { 0x800 } else { 0 }
                };
                c.push_str(&format!(r#"
{{ uint32_t expected=0x87abcdef,count={count};uintptr_t image=0x200ad7,hardware;
__asm__ volatile("push %[image]\n\tpopf\n\t{name}{suffix} %%cl,%[value]\n\tpushf\n\tpop %[flags]"
    : [value] "+m"(expected),[flags] "=&r"(hardware) : [count] "c"(count),[image] "r"(image) : "cc","memory");
g_esi=0x101;g_ecx={count};g_eflags=0x200ad7;xbox_MmioStore(0x101,0x87abcdef,4);reads=writes=0;
"#));
                c.push_str(&emit(&bytes).unwrap());
                c.push_str(&format!("assert(reads==1 && writes==1);assert(xbox_MmioLoad(0x101,4)==expected && ((g_eflags^(uint32_t)hardware)&0x{defined:x}u)==0u);++checks;}}\n"));
            }
        }
    }
    for (name, left) in [("shld", true), ("shrd", false)] {
        for width in [2u8, 4] {
            let counts = if width == 2 {
                vec![16u8]
            } else {
                vec![0, 1, 16, 31]
            };
            for count in counts {
                let mut bytes: Vec<_> = (width == 2)
                    .then_some(0x66)
                    .into_iter()
                    .chain([
                        0x0f,
                        match (left, width) {
                            (true, 2) => 0xa4,
                            (true, _) => 0xa5,
                            (false, 2) => 0xac,
                            _ => 0xad,
                        },
                        0x16,
                    ])
                    .collect();
                if width == 2 {
                    bytes.push(count);
                }
                let suffix = if width == 2 { "w" } else { "l" };
                let argument = if width == 2 { "$16" } else { "%%cl" };
                let defined = if count == 0 {
                    0x8d5
                } else {
                    0xc5 | if count == 1 { 0x800 } else { 0 }
                };
                c.push_str(&format!(r#"
{{ uint32_t expected=0x87abcdef,source=0x87654321,count={count};uintptr_t image=0x200ad7,hardware;
__asm__ volatile("push %[image]\n\tpopf\n\t{name}{suffix} {argument},%{reg}[source],%[value]\n\tpushf\n\tpop %[flags]"
    : [value] "+m"(expected),[flags] "=&r"(hardware) : [source] "d"(source),[count] "c"(count),[image] "r"(image) : "cc","memory");
g_esi=0x101;g_edx=source;g_ecx=count;g_eflags=0x200ad7;xbox_MmioStore(0x101,0x87abcdef,4);reads=writes=0;
"#,reg=if width==2 {"w"}else{"k"}));
                c.push_str(&emit(&bytes).unwrap());
                c.push_str(&format!("assert(reads==1 && writes==1);assert(xbox_MmioLoad(0x101,4)==expected && g_edx==source && g_ecx==count && ((g_eflags^(uint32_t)hardware)&0x{defined:x}u)==0u);++checks;}}\n"));
            }
        }
    }
    for (bytes, expected) in [
        (&[0xd0, 0xe0][..], "assert(g_eax==0x123456f0);"),
        (
            &[0xc0, 0xe0, 0][..],
            "assert(g_eax==0x12345678 && g_eflags==0x200ad7);",
        ),
        (
            &[0xc1, 0xe8, 255][..],
            "assert(g_eax==0 && (g_eflags&0xc5)==0x44);",
        ),
        (
            &[0xd3, 0xe1][..],
            "assert(g_ecx==0x91a2b380 && g_eax==0x12345678);",
        ),
    ] {
        c.push_str("g_eax=0x12345678;g_ecx=0x81234567;g_eflags=0x200ad7;\n");
        c.push_str(&emit(bytes).unwrap());
        c.push_str(expected);
    }
    c.push_str("g_eax=0x12348056;g_eflags=0x200ad7;\n");
    c.push_str(&emit(&[0xd0, 0xcc]).unwrap());
    c.push_str("assert(g_eax==0x12344056 && (g_eflags&0x801)==0x800);\n");
    c.push_str(r#"
#if defined(__linux__)
    long page=sysconf(_SC_PAGESIZE);assert(page>0);
    void *data=mmap(NULL,(size_t)page,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);assert(data!=MAP_FAILED);
    *(uint32_t*)data=123;assert(mprotect(data,(size_t)page,PROT_READ)==0);
    pid_t child=fork();assert(child>=0);
    if(child==0){struct rlimit limit={0,0};setrlimit(RLIMIT_CORE,&limit);
        __asm__ volatile("movb $0,%%cl\n\tshll %%cl,(%0)" : : "r"(data) : "ecx","cc","memory");_exit(0);}
    int status;assert(waitpid(child,&status,0)==child && WIFSIGNALED(status) && WTERMSIG(status)==SIGSEGV);
    assert(munmap(data,(size_t)page)==0);
    puts("native shifts: zero-count read-only destination fault verified");
#endif
    printf("native shifts: %" PRIu64 " hardware result/defined-flag and memory RMW comparisons passed\n",checks);return 0;
}
"#);
    c
}
#[test]
fn synthetic_shift_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_SHIFT_FIXTURE") {
        std::fs::write(path, c).unwrap();
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn generated_shifts_match_hardware_without_undefined_c_arithmetic() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("shift.c");
    let binary = dir.path().join("shift");
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
        .arg(source)
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
