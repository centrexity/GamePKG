//! Actual generated C: alignment faults precede memory callbacks and arithmetic.
use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};

fn fixture() -> String {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let mut c = String::from(
        r#"#define _GNU_SOURCE 1
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "gamepkg_x86_fault.h"
#ifndef _WIN32
#include <signal.h>
#include <sys/wait.h>
#include <sys/resource.h>
#include <ucontext.h>
#include <unistd.h>
#endif
_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
typedef struct { uint32_t u[4]; } RecompXmm;
static RecompXmm g_xmm0;
static unsigned char memory[4096];
static unsigned reads,writes,completed;
static uint32_t local_ebp;
uint32_t xbox_MmioLoad(uint32_t address,int width) {
    assert(address>=1024 && address<=1036 && width==4); ++reads;
    uint32_t value=0;for(unsigned i=0;i<4;++i)value|=(uint32_t)memory[address+i]<<(i*8);
    return value;
}
void xbox_MmioStore(uint32_t address,uint32_t value,int width) {
    assert(address>=1024 && address<=1036 && width==4); ++writes;
    for(unsigned i=0;i<4;++i)memory[address+i]=(unsigned char)(value>>(i*8));
}
"#,
    );
    // MOVAPS load/store, logic/shuffle/unpack, every packed arithmetic opcode.
    let ops = [
        (0x28, "movaps (%[p]),%%xmm0", false),
        (0x29, "movaps %%xmm0,(%[p])", true),
        (0x54, "andps (%[p]),%%xmm0", false),
        (0x55, "andnps (%[p]),%%xmm0", false),
        (0x56, "orps (%[p]),%%xmm0", false),
        (0x57, "xorps (%[p]),%%xmm0", false),
        (0xc6, "shufps $0x1b,(%[p]),%%xmm0", false),
        (0x14, "unpcklps (%[p]),%%xmm0", false),
        (0x15, "unpckhps (%[p]),%%xmm0", false),
        (0x58, "addps (%[p]),%%xmm0", false),
        (0x5c, "subps (%[p]),%%xmm0", false),
        (0x59, "mulps (%[p]),%%xmm0", false),
        (0x5e, "divps (%[p]),%%xmm0", false),
        (0x51, "sqrtps (%[p]),%%xmm0", false),
    ];
    for (op, (opcode, asm, _)) in ops.iter().enumerate() {
        for mode in 0..6 {
            let mut bytes = match mode {
                0 => vec![0x0f, *opcode, 0x06],                      // [esi]
                1 => vec![0x64, 0x0f, *opcode, 0x06],                // FS:[esi]
                2 => vec![0x0f, *opcode, 0x84, 0xfe, 0x20, 0, 0, 0], // esi+edi*8+32
                3 => vec![0x67, 0x0f, *opcode, 0x00],                // [bx+si] modulo 65536
                4 => vec![0x0f, *opcode, 0x45, 0],                   // local EBP, not g_ebp
                _ => vec![0x0f, *opcode, 0x05, 1, 4, 0, 0],          // constant misalignment
            };
            if *opcode == 0xc6 {
                bytes.push(0x1b);
            }
            let decoded = decoder.linear(&bytes, 0x10000, &[]).unwrap();
            assert_eq!(decoded.instructions().len(), 1);
            let code = instruction(&decoded.instructions()[0]).unwrap();
            let guard = code
                .find("gamepkg_x86_general_protection(0x00010000u,ebp)")
                .unwrap();
            let access = code
                .find(if *opcode == 0x29 {
                    "xbox_MmioStore("
                } else {
                    "xbox_MmioLoad("
                })
                .unwrap();
            assert!(guard < access);
            c.push_str(&format!("static void translated_{op}_{mode}(void) {{uint32_t ebp=local_ebp; {code} ++completed;}}\n"));
        }
        c.push_str(&format!(
            r#"
static void hardware_{op}(void *p,RecompXmm *value) {{
    __asm__ volatile("movups %[in],%%xmm0\n\t{asm}\n\tmovups %%xmm0,%[out]"
        : [out] "=m"(*value) : [in] "m"(*value),[p] "r"(p) : "xmm0","memory");
}}
"#
        ));
    }
    c.push_str("static void (*const translated[][6])(void)={\n");
    for op in 0..ops.len() {
        c.push('{');
        for mode in 0..6 {
            c.push_str(&format!("translated_{op}_{mode},"));
        }
        c.push_str("},\n");
    }
    c.push_str("};\nstatic void (*const hardware[])(void*,RecompXmm*)={");
    for op in 0..ops.len() {
        c.push_str(&format!("hardware_{op},"));
    }
    c.push_str(
        r#"};
#ifndef _WIN32
static void gp_handler(int signal,siginfo_t *info,void *context) {
    (void)info;ucontext_t *u=context;
    _exit(signal==SIGSEGV && u->uc_mcontext.gregs[REG_TRAPNO]==13 && u->uc_mcontext.gregs[REG_ERR]==0 ? 0:90);
}
#endif
int main(void) {
    unsigned checks=0,physical=0;
    const RecompXmm initial={{0x3f800000,0x40000000,0x40400000,0x40800000}};
    for(unsigned op=0;op<14;++op)for(unsigned mode=0;mode<6;++mode)for(unsigned offset=0;offset<16;++offset) {
        for(unsigned i=0;i<sizeof(memory);++i)memory[i]=(unsigned char)(i*17u);
        memcpy(memory+1024,&initial,16);
        unsigned char before[sizeof(memory)];memcpy(before,memory,sizeof(memory));
        g_xmm0=initial;g_eflags=0x200ed7;g_ebp=0xbadbad;g_esp=0x800;g_fs_base=0;g_edi=0;
        local_ebp=0xabcdef12;g_esi=1024+offset;g_ebx=0;
        if(mode==1){g_fs_base=0xfffffff0;g_esi+=16;}
        if(mode==2){g_edi=0x20000001;g_esi-=40;}
        if(mode==3){g_ebx=0xffff;g_esi=0x10401+offset;}
        if(mode==4){local_ebp=1024+offset;g_ebp=0xbadbad;}
        uint32_t expected_ebp=local_ebp;
        gamepkg_x86_set_mxcsr(0x1f80,0,0);reads=writes=completed=0;
        GamePkgX86Fault fault;
        int failed=gamepkg_x86_run(translated[op][mode],&fault);
        if(offset || mode==5) {
            assert(failed==1 && fault.vector==13 && fault.eip==0x10000 && fault.ebp==expected_ebp);
            assert(fault.esp==0x800 && fault.eflags==0x200ed7 && fault.mxcsr==0x1f80);
            assert(g_ebp==expected_ebp && reads==0 && writes==0 && completed==0);
            assert(memcmp(&g_xmm0,&initial,16)==0 && memcmp(memory,before,sizeof(memory))==0);
        } else {
            assert(failed==0 && completed==1 && reads==(op==1?0u:4u) && writes==(op==1?4u:0u));
            _Alignas(16) unsigned char expected_memory[16];memcpy(expected_memory,&initial,16);
            RecompXmm expected=initial;hardware[op](expected_memory,&expected);
            assert(memcmp(&g_xmm0,&expected,16)==0 && memcmp(memory+1024,expected_memory,16)==0);
            assert(g_eflags==0x200ed7);
        }
        ++checks;
    }
#ifndef _WIN32
    struct rlimit core={0,0};assert(setrlimit(RLIMIT_CORE,&core)==0);
    // Verify actual x86 GP rather than a page fault, including null+misaligned
    // addresses: alignment must win before the missing-page memory access.
    for(unsigned op=0;op<14;++op)for(unsigned offset=1;offset<16;++offset)for(unsigned missing=0;missing<2;++missing){
        pid_t child=fork();assert(child>=0);
        if(child==0){struct sigaction action={0};action.sa_sigaction=gp_handler;action.sa_flags=SA_SIGINFO;sigemptyset(&action.sa_mask);assert(sigaction(SIGSEGV,&action,0)==0);
            _Alignas(16) unsigned char area[32]={0};RecompXmm value=initial;
            hardware[op](missing?(void*)(uintptr_t)offset:area+offset,&value);_exit(91);}
        int status;assert(waitpid(child,&status,0)==child && WIFEXITED(status) && WEXITSTATUS(status)==0);++physical;
    }
#endif
    printf("%u generated alignment/result checks; %u actual x86 GP faults\n",checks,physical);
    return 0;
}
"#,
    );
    c
}
#[test]
fn export_alignment_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_ALIGNMENT_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn generated_aligned_memory_checks_match_x86_and_precede_accesses() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("alignment.c");
    let binary = dir.path().join("alignment");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let sf = runtime.join("softfloat");
    let mut cmd = std::process::Command::new("cc");
    cmd.args([
        "-std=c11",
        "-O3",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-msse2",
        "-fsanitize=undefined",
        "-fno-sanitize-recover=all",
    ])
    .arg("-I")
    .arg(&runtime)
    .arg("-I")
    .arg(&sf)
    .arg("-I")
    .arg(sf.join("source/include"))
    .arg("-I")
    .arg(sf.join("source/8086-SSE"))
    .arg("-DSOFTFLOAT_ROUND_ODD")
    .arg("-DINLINE_LEVEL=1")
    .arg("-DSOFTFLOAT_FAST_DIV64TO32")
    .arg(&source)
    .arg(runtime.join("gamepkg_x86_fault.c"))
    .arg(runtime.join("gamepkg_x86_arithmetic.c"));
    for file in std::fs::read_to_string(sf.join("sources.txt"))
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty())
    {
        cmd.arg(sf.join(file));
    }
    let out = cmd.arg("-o").arg(&binary).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = std::process::Command::new(&binary).output().unwrap();
    assert!(
        out.status.success(),
        "stdout:{} stderr:{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    print!("{}", String::from_utf8_lossy(&out.stdout));
}
