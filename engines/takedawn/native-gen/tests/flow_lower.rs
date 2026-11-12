use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{
    decode::Decoder,
    lower::{flow, scalar_instruction},
};
use std::collections::BTreeSet;

const CONDITIONS: [&str; 16] = [
    "o", "no", "b", "ae", "e", "ne", "be", "a", "s", "ns", "p", "np", "l", "ge", "le", "g",
];

fn instruction(bytes: &[u8]) -> String {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let decoded = decoder.linear(bytes, 0x10000, &[]).unwrap();
    assert_eq!(decoded.instructions().len(), 1);
    scalar_instruction(&decoded.instructions()[0]).unwrap()
}
fn function(bytes: &[u8], entry: u32) -> String {
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let decoded = decoder
        .cfg(bytes, entry, &[entry], &BTreeSet::new(), &BTreeSet::new())
        .unwrap();
    let leaf = flow::leaf(&decoded, entry, entry + bytes.len() as u32, &budget).unwrap();
    leaf.source().to_owned()
}

#[test]
fn leaf_edges_unsupported_transfers_and_budget_ownership() {
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    for bytes in [
        &[0xeb, 0x20][..],         // outside the function
        &[0xe8, 0, 0, 0, 0, 0xc3], // calls require full ABI
        &[0xff, 0xe0],             // indirect branch requires recovery/dispatch
        &[0x90],                   // no decoded successor/return
        &[0xeb, 0xff],             // branch into an instruction
        &[0x66, 0xeb, 0, 0xc3],    // 16bit IP not implemented
    ] {
        let decoded = decoder
            .cfg(
                bytes,
                0x10000,
                &[0x10000],
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .unwrap();
        let held = budget.used();
        assert!(
            flow::leaf(&decoded, 0x10000, 0x10000 + bytes.len() as u32, &budget).is_err(),
            "{bytes:x?}"
        );
        assert_eq!(budget.used(), held);
    }
    let decoded = decoder.linear(&[0xc3], 0x10000, &[]).unwrap();
    assert!(flow::leaf(&decoded, 0x10000, 0x10001, &Budget::new(1)).is_err());
    let held = budget.used();
    let leaf = flow::leaf(&decoded, 0x10000, 0x10001, &budget).unwrap();
    assert!(leaf.source().contains("g_esp += 4u"));
    assert!(budget.used() > held);
    drop(decoded);
    drop(decoder);
    assert!(budget.used() > 0);
    drop(leaf);
    assert_eq!(budget.used(), 0);
    for (a, b) in [
        ("e", "z"),
        ("ae", "nc"),
        ("p", "pe"),
        ("g", "nle"),
        ("l", "nge"),
    ] {
        assert_eq!(flow::condition(a).unwrap(), flow::condition(b).unwrap());
    }
    assert!(flow::condition("e); bad()").is_err());
}

fn fixture() -> String {
    let mut c = String::from("#include <stdint.h>\n#include <inttypes.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <assert.h>\n");
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let path = path.to_str().unwrap();
        assert!(!path.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{path}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\nuint32_t xbox_MmioLoad(uint32_t,int);\nvoid xbox_MmioStore(uint32_t,uint32_t,int);\n");
    }
    c.push_str(
        r#"
RECOMP_TLS uint32_t g_eax,g_ebx,g_ecx,g_edx,g_esi,g_edi,g_esp,g_ebp,g_fs_base,g_eflags;
static unsigned char memory[4096];
static uint64_t checks;
static unsigned reads,writes,truth[16];
uint32_t xbox_MmioLoad(uint32_t address,int width) {
    assert(width==1 || width==2 || width==4);
    assert(address<=sizeof(memory)-(unsigned)width); ++reads;
    uint32_t result=0;
    for(int i=0;i<width;++i) result|=(uint32_t)memory[address+(unsigned)i]<<(8u*(unsigned)i);
    return result;
}
void xbox_MmioStore(uint32_t address,uint32_t value,int width) {
    assert(width==1 || width==2 || width==4);
    assert(address<=sizeof(memory)-(unsigned)width); ++writes;
    for(int i=0;i<width;++i) memory[address+(unsigned)i]=(unsigned char)(value>>(8u*(unsigned)i));
}
"#,
    );
    for (index, suffix) in CONDITIONS.iter().enumerate() {
        for width in [1, 2, 4] {
            let bytes: Vec<_> = (width == 2)
                .then_some(0x66)
                .into_iter()
                .chain([
                    0x0f,
                    if width == 1 {
                        0x90 + index as u8
                    } else {
                        0x40 + index as u8
                    },
                    if width == 1 { 0xc0 } else { 0xc1 },
                ])
                .collect();
            let code = instruction(&bytes);
            let asm = if width == 1 {
                format!("set{suffix} %b[result]")
            } else {
                let reg = if width == 2 { "w" } else { "k" };
                format!("cmov{suffix} %{reg}[y], %{reg}[result]")
            };
            let save_truth = if width == 1 {
                format!("truth[{index}] = real & 1u;")
            } else {
                String::new()
            };
            c.push_str(&format!(r#"
static void check_{suffix}{width}(uint32_t image) {{
    uint32_t real=0xaabbcc78u,y=0x1234fedcu;
    uintptr_t flags,load=(uintptr_t)image;
    __asm__ volatile("push %[load]\n\tpopf\n\t{asm}\n\tpushf\n\tpop %[flags]"
        : [result] "+&a"(real),[flags] "=&r"(flags)
        : [y] "q"(y),[load] "r"(load) : "cc");
    g_eax=0xaabbcc78u; g_ecx=y; g_eflags=image|0x200200u;
    uint32_t before=g_eflags;
    {code}
    if(g_eax!=real || g_ecx!=y || g_eflags!=before || (((uint32_t)flags^image)&0x8d5u)!=0u) {{
        fprintf(stderr,"condition {suffix}/{width} image=%08x actual=%08x expected=%08x\n",image,g_eax,real); abort();
    }}
    {save_truth} ++checks;
}}
"#));
        }
        let entry = 0x20000 + index as u32 * 0x100;
        let bytes = [
            0x70 + index as u8,
            6,
            0xb8,
            1,
            0,
            0,
            0,
            0xc3,
            0xb8,
            2,
            0,
            0,
            0,
            0xc3,
        ];
        c.push_str(&function(&bytes, entry));
    }
    // LOOP-family functions branch once so initial count zero is bounded.
    for (i, opcode) in [0xe2, 0xe1, 0xe0, 0xe3].into_iter().enumerate() {
        for address16 in [false, true] {
            let entry = 0x30000 + i as u32 * 0x200 + u32::from(address16) * 0x100;
            let bytes: Vec<_> = address16
                .then_some(0x67)
                .into_iter()
                .chain([opcode, 6, 0xb8, 1, 0, 0, 0, 0xc3, 0xb8, 2, 0, 0, 0, 0xc3])
                .collect();
            c.push_str(&function(&bytes, entry));
            let mnemonic = ["loop", "loope", "loopne", "jecxz"][i];
            let native = if address16 {
                if i == 3 {
                    "jcxz".to_owned()
                } else {
                    format!(".byte 0x67; {mnemonic}")
                }
            } else {
                mnemonic.to_owned()
            };
            let native = if !address16 && i == 3 {
                "jecxz".to_owned()
            } else {
                native
            };
            let native_guard = if address16 {
                "#if defined(__i386__)\n"
            } else {
                ""
            };
            let native_end = if address16 { "#endif\n" } else { "" };
            c.push_str(&format!(r#"
static void loop_{i}_{address16}(uint32_t count,uint32_t image) {{
    uint32_t real=1,native_count=count;
    uintptr_t flags=0,load=(uintptr_t)image;
    (void)flags;(void)load;
    {native_guard}
    __asm__ volatile("push %[load]\n\tpopf\n\t{native} 1f\n\tjmp 2f\n1:\tmovl $2, %[result]\n2:\tpushf\n\tpop %[flags]"
        : [result] "+&a"(real),[count] "+&c"(native_count),[flags] "=&r"(flags)
        : [load] "r"(load) : "cc");
    assert((((uint32_t)flags^image)&0x8d5u)==0u);
    {native_end}
    g_ecx=count;g_esp=0x700;g_ebp=0xabcdef01;g_eflags=image|0x200200u;
    uint32_t before=g_eflags;
    sub_{entry:08x}();
"#));
            if address16 {
                c.push_str(&format!(
                    r#"
#if !defined(__i386__)
    native_count=(count&0xffff0000u)|((count-{}u)&0xffffu);
    real=((native_count&0xffffu) {} 0u){} ? 2u:1u;
#endif
"#,
                    if i == 3 { 0 } else { 1 },
                    if i == 3 { "==" } else { "!=" },
                    match i {
                        1 => " && ((image&0x40u)!=0u)",
                        2 => " && ((image&0x40u)==0u)",
                        _ => "",
                    }
                ));
            }
            c.push_str("assert(g_eax==real && g_ecx==native_count && g_esp==0x704 && g_ebp==0xabcdef01 && g_eflags==before); ++checks;\n}\n");
        }
    }
    c.push_str(&function(
        &[0xb9, 10, 0, 0, 0, 0x31, 0xc0, 0x01, 0xc8, 0xe2, 0xfc, 0xc3],
        0x40000,
    ));
    c.push_str(&function(
        &[0x55, 0x89, 0xe5, 0x8b, 0x45, 8, 0xc9, 0xc2, 4, 0],
        0x40100,
    ));
    c.push_str(&function(&[0x66, 0xc3], 0x40200));
    c.push_str("int main(void) {\nfor(uint32_t bits=0;bits<64;++bits) {\nuint32_t image=2u|((bits&1u)?1u:0u)|((bits&2u)?4u:0u)|((bits&4u)?0x10u:0u)|((bits&8u)?0x40u:0u)|((bits&16u)?0x80u:0u)|((bits&32u)?0x800u:0u);\n");
    for (index, suffix) in CONDITIONS.iter().enumerate() {
        for width in [1, 2, 4] {
            c.push_str(&format!("check_{suffix}{width}(image);\n"));
        }
        let entry = 0x20000 + index as u32 * 0x100;
        c.push_str(&format!("g_esp=0x700; g_ebp=0xabcdef01; g_eflags=image|0x200200u; sub_{entry:08x}(); assert(g_eax==1u+truth[{index}] && g_esp==0x704 && g_ebp==0xabcdef01 && g_eflags==(image|0x200200u)); ++checks;\n"));
    }
    c.push_str("uint32_t counts[]={0,1,2,3,0xffff,0x10000,0x10001,0xffffffffu};\nfor(unsigned i=0;i<8;++i) {\n");
    for i in 0..4 {
        for address16 in [false, true] {
            c.push_str(&format!("loop_{i}_{address16}(counts[i],image);\n"));
        }
    }
    c.push_str("}}\ng_esp=0x700;g_ebp=0x12345678; sub_00040000(); assert(g_eax==55 && g_ecx==0 && g_esp==0x704 && g_ebp==0x12345678);\ng_esp=0x700;g_ebp=0x12345678;xbox_MmioStore(0x704,0xdeadbeef,4);sub_00040100();assert(g_eax==0xdeadbeef && g_esp==0x708 && g_ebp==0x12345678);\ng_esp=0x700;sub_00040200();assert(g_esp==0x702);\n");
    c.push_str(
        "g_eax=7;g_esi=0x100;g_eflags=2;xbox_MmioStore(0x100,0x87654321u,4);reads=writes=0;\n",
    );
    c.push_str(&instruction(&[0x0f, 0x44, 0x06]));
    c.push_str("assert(g_eax==7 && reads==1 && writes==0 && g_eflags==2);\ng_eflags=0x42; reads=writes=0;\n");
    c.push_str(&instruction(&[0x0f, 0x94, 0x06]));
    c.push_str(
        "assert(memory[0x100]==1 && reads==0 && writes==1 && g_eflags==0x42);\ng_eax=0x12345678;\n",
    );
    c.push_str(&instruction(&[0x0f, 0x94, 0xc4]));
    c.push_str("assert(g_eax==0x12340178 && g_eflags==0x42);\nprintf(\"native flow: %\" PRIu64 \" condition/branch/counter checks plus frame and memory cases passed\\n\",checks);return 0;}\n");
    c
}

#[test]
fn synthetic_condition_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_FLOW_FIXTURE") {
        std::fs::write(path, c).unwrap();
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn generated_conditions_and_closed_functions_execute_against_x86() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flow.c");
    let binary = dir.path().join("flow");
    std::fs::write(&path, fixture()).unwrap();
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
        .arg(path)
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
