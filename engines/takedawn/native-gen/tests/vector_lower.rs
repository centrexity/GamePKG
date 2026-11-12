use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::program::{Program, Unit};
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
use std::collections::BTreeSet;

fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let decoded = decoder.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        decoded.instructions().len() == 1
            && usize::from(decoded.instructions()[0].size) == bytes.len(),
        "incomplete vector fixture"
    );
    instruction(&decoded.instructions()[0])
}
#[test]
fn vector_register_width_prefix_alignment_and_expression_rejection() {
    for bytes in [
        &[0x66, 0x0f, 0x10, 0xc1][..],   // SSE2 MOVUPD
        &[0xf2, 0x0f, 0x10, 0xc1],       // SSE2 MOVSD
        &[0x65, 0xf3, 0x0f, 0x10, 0x06], // unmodeled GS
    ] {
        assert!(emit(bytes).is_err(), "{bytes:x?}");
    }
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let decoded = decoder
        .linear(&[0xf3, 0x0f, 0x10, 0xc1], 0x10000, &[])
        .unwrap();
    let original = decoded.instructions()[0].clone();
    let mut bad = original.clone();
    bad.operands[0].reg = Some("xmm8".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.operands[1].expr = Some("bad()".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.operands[0].reg = Some("xmm0; bad()".into());
    assert!(instruction(&bad).is_err());
    let mut bad = original.clone();
    bad.bytes_hex = "f20f10c1".into();
    assert!(instruction(&bad).is_err());
    let mut harmless = original.clone();
    harmless.op_str = "bad()".into();
    assert_eq!(
        instruction(&original).unwrap(),
        instruction(&harmless).unwrap()
    );
    let decoded = decoder
        .linear(&[0xf3, 0x0f, 0x10, 0x06], 0x10000, &[])
        .unwrap();
    let mut bad = decoded.instructions()[0].clone();
    bad.operands[1].mem_size = 16;
    assert!(instruction(&bad).is_err());
}

fn fixture() -> String {
    let mut c=String::from("#include <stdint.h>\n#include <stdio.h>\n#include <string.h>\n#include <assert.h>\n#include <stdlib.h>\n#include <setjmp.h>\n#include <xmmintrin.h>\n");
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let path = path.to_str().unwrap();
        assert!(!path.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{path}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\ntypedef union RecompXmm {float f[4];double d[2];uint32_t u[4];int32_t i[4];uint64_t q[2];} RecompXmm;\nuint32_t xbox_MmioLoad(uint32_t,int);\nvoid xbox_MmioStore(uint32_t,uint32_t,int);\n");
    }
    c.push_str(
        r#"
RECOMP_TLS uint32_t g_eax,g_ebx,g_ecx,g_edx,g_esi,g_edi,g_esp,g_ebp,g_fs_base,g_eflags;
RECOMP_TLS RecompXmm g_xmm0,g_xmm1,g_xmm2,g_xmm3,g_xmm4,g_xmm5,g_xmm6,g_xmm7;
static unsigned char memory[4096];
static unsigned reads,writes,checks;
static int fail_read_after=-1;
static jmp_buf read_failure;
struct Bytes {unsigned char b[16];};
uint32_t xbox_MmioLoad(uint32_t address,int width) {
    assert(width==4 && address<=sizeof(memory)-4);
    unsigned which=reads++;if(fail_read_after>=0 && which==(unsigned)fail_read_after)longjmp(read_failure,1);
    uint32_t value=0;for(unsigned i=0;i<4;++i)value|=(uint32_t)memory[address+i]<<(i*8u);
    return value;
}
void xbox_MmioStore(uint32_t address,uint32_t value,int width) {
    assert(width==4 && address<=sizeof(memory)-4);++writes;
    for(unsigned i=0;i<4;++i)memory[address+i]=(unsigned char)(value>>(i*8u));
}
"#,
    );
    let mut registers: Vec<(Vec<u8>, String, bool)> = vec![
        (
            vec![0xf3, 0x0f, 0x10, 0xc1],
            "movss %%xmm1,%%xmm0".into(),
            false,
        ),
        (
            vec![0xf3, 0x0f, 0x11, 0xc8],
            "movss %%xmm1,%%xmm0".into(),
            false,
        ),
        (vec![0x0f, 0x28, 0xc1], "movaps %%xmm1,%%xmm0".into(), false),
        (vec![0x0f, 0x10, 0xc1], "movups %%xmm1,%%xmm0".into(), false),
        (
            vec![0x0f, 0x16, 0xc1],
            "movlhps %%xmm1,%%xmm0".into(),
            false,
        ),
        (
            vec![0x0f, 0x12, 0xc1],
            "movhlps %%xmm1,%%xmm0".into(),
            false,
        ),
        (
            vec![0x0f, 0x50, 0xc1],
            "movmskps %%xmm1,%[mask]".into(),
            true,
        ),
    ];
    for (opcode, name) in [
        (0x54, "andps"),
        (0x56, "orps"),
        (0x57, "xorps"),
        (0x55, "andnps"),
        (0x14, "unpcklps"),
        (0x15, "unpckhps"),
    ] {
        registers.push((
            vec![0x0f, opcode, 0xc1],
            format!("{name} %%xmm1,%%xmm0"),
            false,
        ));
        registers.push((
            vec![0x0f, opcode, 0xc0],
            format!("{name} %%xmm0,%%xmm0"),
            false,
        ));
    }
    for immediate in 0..=255u8 {
        registers.push((
            vec![0x0f, 0xc6, 0xc1, immediate],
            format!("shufps $0x{immediate:x},%%xmm1,%%xmm0"),
            false,
        ));
        registers.push((
            vec![0x0f, 0xc6, 0xc0, immediate],
            format!("shufps $0x{immediate:x},%%xmm0,%%xmm0"),
            false,
        ));
    }
    for (index, (bytes, asm, gpr)) in registers.iter().enumerate() {
        let code = emit(bytes).unwrap();
        c.push_str(&format!(r#"
static void reg_{index}(const RecompXmm *a,const RecompXmm *b) {{
    RecompXmm expected;uint32_t mask=0,mx_before,mx_after;
    __asm__ volatile("stmxcsr %[mb]\n\tmovups %[a],%%xmm0\n\tmovups %[b],%%xmm1\n\t{asm}\n\tmovups %%xmm0,%[out]\n\tstmxcsr %[ma]"
        : [out] "=m"(expected),[mask] "+a"(mask),[mb] "=m"(mx_before),[ma] "=m"(mx_after)
        : [a] "m"(*a),[b] "m"(*b) : "xmm0","xmm1","memory");
    assert(mx_before==mx_after);
    g_xmm0=*a;g_xmm1=*b;g_eax=0xabcdef01;g_eflags=0x200ed7;
    uint32_t native_before=_mm_getcsr();
    {code}
    assert(_mm_getcsr()==native_before && g_eflags==0x200ed7 && memcmp(&g_xmm0,&expected,16)==0 && memcmp(&g_xmm1,b,16)==0);
    assert(g_eax=={expected_gpr});++checks;
}}
"#,expected_gpr=if *gpr {"mask"}else{"0xabcdef01u"}));
    }
    // Dynamic scalar/unaligned/half transfers cover every byte alignment.
    type MemoryCase = (Vec<u8>, &'static str, u32, u32, bool, u8);
    let mut mem: Vec<MemoryCase> = vec![
        (
            vec![0xf3, 0x0f, 0x10, 0x06],
            "movss %[mem],%%xmm0",
            1,
            0,
            false,
            0,
        ),
        (
            vec![0xf3, 0x0f, 0x11, 0x06],
            "movss %%xmm0,%[mem]",
            0,
            1,
            false,
            0,
        ),
        (
            vec![0x0f, 0x10, 0x06],
            "movups %[mem],%%xmm0",
            4,
            0,
            false,
            0,
        ),
        (
            vec![0x0f, 0x11, 0x06],
            "movups %%xmm0,%[mem]",
            0,
            4,
            false,
            0,
        ),
        (
            vec![0x0f, 0x12, 0x06],
            "movlps %[mem],%%xmm0",
            2,
            0,
            false,
            0,
        ),
        (
            vec![0x0f, 0x13, 0x06],
            "movlps %%xmm0,%[mem]",
            0,
            2,
            false,
            0,
        ),
        (
            vec![0x0f, 0x16, 0x06],
            "movhps %[mem],%%xmm0",
            2,
            0,
            false,
            0,
        ),
        (
            vec![0x0f, 0x17, 0x06],
            "movhps %%xmm0,%[mem]",
            0,
            2,
            false,
            0,
        ),
        (
            vec![0x64, 0xf3, 0x0f, 0x10, 0x06],
            "movss %[mem],%%xmm0",
            1,
            0,
            false,
            1,
        ),
        (
            vec![0x67, 0xf3, 0x0f, 0x10, 0x00],
            "movss %[mem],%%xmm0",
            1,
            0,
            false,
            2,
        ),
        (
            vec![0x0f, 0x28, 0x05, 0, 4, 0, 0],
            "movaps %[mem],%%xmm0",
            4,
            0,
            true,
            0,
        ),
        (
            vec![0x0f, 0x29, 0x05, 0, 4, 0, 0],
            "movaps %%xmm0,%[mem]",
            0,
            4,
            true,
            0,
        ),
    ];
    for (opcode, name) in [
        (0x54, "andps"),
        (0x56, "orps"),
        (0x57, "xorps"),
        (0x55, "andnps"),
        (0x14, "unpcklps"),
        (0x15, "unpckhps"),
    ] {
        mem.push((
            vec![0x0f, opcode, 0x05, 0, 4, 0, 0],
            match name {
                "andps" => "andps %[mem],%%xmm0",
                "orps" => "orps %[mem],%%xmm0",
                "xorps" => "xorps %[mem],%%xmm0",
                "andnps" => "andnps %[mem],%%xmm0",
                "unpcklps" => "unpcklps %[mem],%%xmm0",
                _ => "unpckhps %[mem],%%xmm0",
            },
            4,
            0,
            true,
            0,
        ));
    }
    mem.push((
        vec![0x0f, 0xc6, 0x05, 0, 4, 0, 0, 0xb1],
        "shufps $177,%[mem],%%xmm0",
        4,
        0,
        true,
        0,
    ));
    for (index, (bytes, asm, expected_reads, expected_writes, _, kind)) in mem.iter().enumerate() {
        let code = emit(bytes).unwrap();
        c.push_str(&format!(r#"
static void mem_{index}(const RecompXmm *a,const RecompXmm *b,unsigned offset) {{
    _Alignas(16) unsigned char data[64];for(unsigned i=0;i<64;++i)data[i]=(unsigned char)(i*7u+3u);
    memcpy(data+16+offset,b,16);memcpy(memory+0x3f0,data,64);
    RecompXmm expected;uint32_t mx_before,mx_after;
    __asm__ volatile("stmxcsr %[mb]\n\tmovups %[a],%%xmm0\n\t{asm}\n\tmovups %%xmm0,%[out]\n\tstmxcsr %[ma]"
        : [mem] "+m"(*(struct Bytes *)(data+16+offset)),[out] "=m"(expected),[mb] "=m"(mx_before),[ma] "=m"(mx_after)
        : [a] "m"(*a) : "xmm0","memory");
    assert(mx_before==mx_after);
    g_xmm0=*a;g_eflags=0x200ed7;g_fs_base={fs};g_ebx=0xfffffff0u;g_esi={base}+offset;reads=writes=0;
    uint32_t native_before=_mm_getcsr();
    {code}
    assert(memcmp(&g_xmm0,&expected,16)==0 && memcmp(memory+0x3f0,data,64)==0);
    assert(reads=={expected_reads} && writes=={expected_writes} && g_eflags==0x200ed7 && _mm_getcsr()==native_before);++checks;
}}
"#,fs=if *kind==1 {16}else{0},base=match kind {1=>"0x3f0u",2=>"0x410u",_=>"0x400u"}));
    }
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let caller = [
        0xf3, 0x0f, 0x10, 0x06, 0xe8, 0xf7, 0, 0, 0, 0x0f, 0x11, 0x07, 0xc3,
    ];
    let callee = [0x0f, 0x16, 0xc0, 0xc3];
    let a = decoder
        .cfg(
            &caller,
            0x18000,
            &[0x18000],
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
    let b = decoder
        .cfg(
            &callee,
            0x18100,
            &[0x18100],
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
    let program = Program::build(
        &[
            Unit {
                decoded: &a,
                entry: 0x18000,
                end: 0x18000 + caller.len() as u32,
            },
            Unit {
                decoded: &b,
                entry: 0x18100,
                end: 0x18100 + callee.len() as u32,
            },
        ],
        &budget,
    )
    .unwrap();
    let mut program_c = Vec::new();
    program.write_source(&mut program_c).unwrap();
    c.push_str(std::str::from_utf8(&program_c).unwrap());
    c.push_str(r#"
int main(void) {
    uint32_t saved=_mm_getcsr(),random=0x12345678u;
    uint32_t edge[]={0,0x80000000u,0x7f800000u,0xff800000u,0x7fc12345u,0x7f800001u,1,0x807fffffu,0x3f800000u,0xffffffffu};
    for(unsigned sample=0;sample<64;++sample) {
        _mm_setcsr(0x1f80u | ((sample&8u)?0x3fu:0u) | ((sample&3u)<<13u) | ((sample&4u)?0x8000u:0u));
        RecompXmm a,b;
        for(unsigned lane=0;lane<4;++lane) {
            if(sample<10) {a.u[lane]=edge[(sample+lane)%10];b.u[lane]=edge[(sample+lane+3)%10];}
            else {random=random*1664525u+1013904223u;a.u[lane]=random;random=random*1664525u+1013904223u;b.u[lane]=random;}
        }
"#);
    for index in 0..registers.len() {
        c.push_str(&format!("reg_{index}(&a,&b);\n"));
    }
    for (index, (_, _, _, _, aligned, _)) in mem.iter().enumerate() {
        c.push_str(&format!(
            "for(unsigned offset=0;offset<{};++offset)mem_{index}(&a,&b,offset);\n",
            if *aligned { 1 } else { 16 }
        ));
    }
    c.push_str("}\n");
    for index in 0..8 {
        for register in 0..8 {
            for lane in 0..4 {
                c.push_str(&format!(
                    "g_xmm{register}.u[{lane}] = {}u;\n",
                    register * 256 + lane
                ));
            }
        }
        c.push_str(&emit(&[0x0f, 0x28, 0xc0 | ((index as u8) << 3) | (7 - index as u8)]).unwrap());
        for lane in 0..4 {
            c.push_str(&format!(
                "assert(g_xmm{index}.u[{lane}]=={}u);\n",
                (7 - index) * 256 + lane
            ));
        }
    }
    c.push_str("g_esi=0x400;g_edi=0x600;g_esp=0x800;g_ebp=0xaabbccdd;g_eflags=0x200ed7;for(unsigned i=0;i<4;++i)g_xmm0.u[i]=99;xbox_MmioStore(0x400,0x7f800001,4);reads=writes=0;\nuint32_t call_mxcsr=_mm_getcsr();sub_00018000();assert(reads==1 && writes==5 && g_esp==0x804 && g_ebp==0xaabbccdd && g_eflags==0x200ed7 && _mm_getcsr()==call_mxcsr);\nassert(xbox_MmioLoad(0x600,4)==0x7f800001 && xbox_MmioLoad(0x604,4)==0 && xbox_MmioLoad(0x608,4)==0x7f800001 && xbox_MmioLoad(0x60c,4)==0 && xbox_MmioLoad(0x7fc,4)==0x18009);\n");
    for (bytes, fail) in [
        (&[0x0f, 0x10, 0x06][..], 2),
        (&[0x0f, 0x12, 0x06][..], 1),
        (&[0x0f, 0x16, 0x06][..], 1),
        (&[0xf3, 0x0f, 0x10, 0x06][..], 0),
    ] {
        c.push_str(&format!("g_xmm0.u[0]=0x12345678;g_xmm0.u[1]=0xabcdef01;g_xmm0.u[2]=0x87654321;g_xmm0.u[3]=0x7f800001;g_esi=0x400;g_eflags=0x200ed7;reads=0;fail_read_after={fail};\nif(setjmp(read_failure)==0) {{\n"));
        c.push_str(&emit(bytes).unwrap());
        c.push_str(&format!("assert(!\"expected read failure\");}}\nfail_read_after=-1;assert(g_xmm0.u[0]==0x12345678 && g_xmm0.u[1]==0xabcdef01 && g_xmm0.u[2]==0x87654321 && g_xmm0.u[3]==0x7f800001 && reads=={} && g_eflags==0x200ed7);\n",fail+1));
    }
    c.push_str("_mm_setcsr(saved);printf(\"native SSE1: %u exact-bit register/memory/shuffle comparisons and 4 failed-load cases passed\\n\",checks);return 0;}\n");
    c
}
#[test]
fn synthetic_vector_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_VECTOR_FIXTURE") {
        std::fs::write(path, c).unwrap();
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn generated_vector_data_matches_hardware_bits_and_mxcsr() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("vector.c");
    let binary = dir.path().join("vector");
    std::fs::write(&source, fixture()).unwrap();
    let output = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O3",
            "-msse",
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
