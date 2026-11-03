use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{
    decode::Decoder,
    lower::integer_instruction,
    program::{Program, Unit},
};
use std::collections::BTreeSet;

fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let decoded = decoder.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        decoded.instructions().len() == 1
            && usize::from(decoded.instructions()[0].size) == bytes.len(),
        "incomplete divide fixture"
    );
    integer_instruction(&decoded.instructions()[0])
}

#[test]
fn division_rejects_unsupported_forms_and_untrusted_expressions() {
    for bytes in [
        &[0xf0, 0xf7, 0x30][..],
        &[0xf3, 0xf7, 0xf1],
        &[0x65, 0xf7, 0x30],
    ] {
        assert!(emit(bytes).is_err());
    }
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let decoded = decoder.linear(&[0xf7, 0xf1], 0x10000, &[]).unwrap();
    for mode in 0..4 {
        let mut insn = decoded.instructions()[0].clone();
        match mode {
            0 => insn.operands[0].expr = Some("bad()".into()),
            1 => insn.operands[0].reg = Some("cx".into()),
            2 => insn.operands.clear(),
            _ => insn.operands.push(insn.operands[0].clone()),
        }
        assert!(integer_instruction(&insn).is_err());
    }
}

fn program() -> String {
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let bytes: [&[u8]; 2] = [
        &[
            0xbd, 0x12, 0xef, 0xcd, 0xab, 0xe8, 0xf6, 0, 0, 0, 0xb8, 99, 0, 0, 0, 0xc3,
        ],
        &[0xf7, 0xf1, 0xb8, 77, 0, 0, 0, 0xc3],
    ];
    let starts = [0x20000, 0x20100];
    let decoded: Vec<_> = bytes
        .iter()
        .zip(starts)
        .map(|(bytes, entry)| {
            decoder
                .cfg(bytes, entry, &[entry], &BTreeSet::new(), &BTreeSet::new())
                .unwrap()
        })
        .collect();
    let units: Vec<_> = decoded
        .iter()
        .enumerate()
        .map(|(n, decoded)| Unit {
            decoded,
            entry: starts[n],
            end: starts[n] + bytes[n].len() as u32,
        })
        .collect();
    let p = Program::build(&units, &budget).unwrap();
    let mut source = Vec::new();
    p.write_source(&mut source).unwrap();
    String::from_utf8(source).unwrap()
}

fn fixture() -> String {
    let mut c=String::from("#include <stdint.h>\n#include <inttypes.h>\n#include <assert.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include \"gamepkg_x86_fault.h\"\n");
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\nuint32_t xbox_MmioLoad(uint32_t,int);\nvoid xbox_MmioStore(uint32_t,uint32_t,int);\n");
    }
    c.push_str(r#"
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <signal.h>
#include <sys/wait.h>
#include <sys/resource.h>
#include <unistd.h>
#endif
RECOMP_TLS uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
RECOMP_TLS int g_df;
static unsigned char memory[4096];
static unsigned reads,writes,committed;
static uint64_t hardware_checks,guest_fault_checks;
uint32_t xbox_MmioLoad(uint32_t address,int width){
    assert(width==1||width==2||width==4);assert(address<=sizeof(memory)-(unsigned)width);++reads;
    uint32_t value=0;for(int i=0;i<width;++i)value|=(uint32_t)memory[address+(unsigned)i]<<(8u*(unsigned)i);return value;
}
void xbox_MmioStore(uint32_t address,uint32_t value,int width){
    assert(width==1||width==2||width==4);assert(address<=sizeof(memory)-(unsigned)width);++writes;
    for(int i=0;i<width;++i)memory[address+(unsigned)i]=(unsigned char)(value>>(8u*(unsigned)i));
}
static int reference(unsigned size,unsigned signed_op,uint32_t lo,uint32_t hi,uint32_t divisor,uint32_t *eax,uint32_t *edx){
    unsigned bits=size*8;uint32_t mask=UINT32_MAX>>(32-bits),sign=UINT32_C(1)<<(bits-1);
    divisor&=mask;if(!divisor)return 1;
    uint64_t n=size==1?(lo&0xffffu):((uint64_t)(hi&mask)<<bits)|(lo&mask);
    uint32_t q,r;
    if(signed_op){
        int64_t numerator;
        if(size==4)memcpy(&numerator,&n,sizeof(n));
        else numerator=(int64_t)n-((n>>(2*bits-1))?(INT64_C(1)<<(2*bits)):0);
        int64_t denominator=(int64_t)divisor-((divisor&sign)?(INT64_C(1)<<bits):0);
        if(numerator==INT64_MIN&&denominator==-1)return 1;
        int64_t quotient=numerator/denominator,remainder=numerator%denominator;
        if(quotient<-(int64_t)sign||quotient>(int64_t)sign-1)return 1;
        q=(uint32_t)quotient;r=(uint32_t)remainder;
    }else{
        uint64_t quotient=n/divisor;if(quotient>mask)return 1;q=(uint32_t)quotient;r=(uint32_t)(n%divisor);
    }
    if(size==1){*eax=(lo&0xffff0000u)|(q&0xffu)|((r&0xffu)<<8);*edx=hi;}
    else{*eax=(lo&~mask)|(q&mask);*edx=(hi&~mask)|(r&mask);}return 0;
}
static void hardware(unsigned size,unsigned signed_op,uint32_t *lo,uint32_t *hi,uint32_t divisor){
"#);
    for size in [1, 2, 4] {
        for signed in [false, true] {
            let name = if signed { "idiv" } else { "div" };
            let suffix = match size {
                1 => "b",
                2 => "w",
                _ => "l",
            };
            c.push_str(&format!("if(size=={size}&&signed_op=={signed}){{__asm__ volatile(\"{name}{suffix} %[source]\" : \"+&a\"(*lo),\"+&d\"(*hi) : [source] \"m\"(divisor) : \"cc\");return;}}\n",signed=u8::from(signed)));
        }
    }
    c.push_str("abort();}\n");
    let mut names = Vec::new();
    for size in [1, 2, 4] {
        for (name, ext, signed) in [("div", 6, false), ("idiv", 7, true)] {
            for variant in 0..if size == 1 { 5 } else { 4 } {
                let memory = variant == 1;
                let source = match variant {
                    0 => 0xc1,
                    1 => 6,
                    2 => 0xc0,
                    3 => 0xc2,
                    _ => 0xc4,
                };
                let alias = match variant {
                    2 => "divisor=lo;",
                    3 => "divisor=hi;",
                    4 => "divisor=(lo>>8u)&255u;",
                    _ => "",
                };
                let bytes: Vec<_> = (size == 2)
                    .then_some(0x66)
                    .into_iter()
                    .chain([if size == 1 { 0xf6 } else { 0xf7 }, (ext << 3) | source])
                    .collect();
                let emitted = emit(&bytes).unwrap();
                let function = format!("{name}{size}_{variant}");
                names.push((function.clone(), size, memory));
                c.push_str(&format!("static void op_{function}(void){{uint32_t ebp=g_ebp; {emitted} ++committed;}}\n"));
                let setup = if memory {
                    format!("g_esi=0x301;xbox_MmioStore(g_esi,divisor,{size});reads=writes=0;")
                } else {
                    "g_esi=0x301;reads=writes=0;".into()
                };
                c.push_str(&format!(r#"
static void check_{function}(uint32_t lo,uint32_t hi,uint32_t divisor){{
    {alias}
    uint32_t expected_lo=lo,expected_hi=hi,real_lo=lo,real_hi=hi;
    int fault=reference({size},{signed},lo,hi,divisor,&expected_lo,&expected_hi);
    uint32_t flags=0x200202u|((lo^hi^divisor)&0xcd5u);
    g_eax=lo;g_edx=hi;g_ecx=divisor;g_eflags=flags;g_ebp=0xdead1234u;
    g_ebx=0x11112222;g_edi=0x33334444;g_esp=0x345;g_fs_base=0x100;g_df=1;
    {setup}committed=0;GamePkgX86Fault caught;
    int result=gamepkg_x86_run(op_{function},&caught);
    assert(result==fault&&reads=={reads}u&&writes==0u&&g_eflags==flags&&g_ecx==divisor&&g_ebp==0xdead1234u&&g_esp==0x345&&g_ebx==0x11112222&&g_edi==0x33334444&&g_fs_base==0x100&&g_df==1);
    if(fault){{
        assert(committed==0&&g_eax==lo&&g_edx==hi&&caught.vector==0&&caught.eip==0x10000&&caught.eax==lo&&caught.edx==hi&&caught.ecx==divisor&&caught.eflags==flags&&caught.ebp==0xdead1234u&&caught.esp==0x345&&caught.esi==0x301&&caught.edi==0x33334444&&caught.ebx==0x11112222&&caught.fs_base==0x100&&caught.direction==1);++guest_fault_checks;
    }}else{{
        hardware({size},{signed},&real_lo,&real_hi,divisor);
        assert(committed==1&&g_eax==real_lo&&g_edx==real_hi&&g_eax==expected_lo&&g_edx==expected_hi);
        GamePkgX86Fault zero={{0}};assert(memcmp(&caught,&zero,sizeof(zero))==0);++hardware_checks;
    }}
}}
"#,signed=u8::from(signed),reads=u8::from(memory)));
            }
        }
    }
    c.push_str(&program());
    c.push_str(r#"
static void normal(void){}
static unsigned outer_resumed;
static void nested(void){GamePkgX86Fault inner;g_ecx=0;assert(gamepkg_x86_run(op_div4_0,&inner)==1&&inner.eip==0x10000);++outer_resumed;gamepkg_x86_divide_error(0x33333,0x44444);}
#ifdef _WIN32
static DWORD WINAPI worker(void *raw){
#else
static void *worker(void *raw){
#endif
    uint32_t tag=(uint32_t)(uintptr_t)raw;
    for(unsigned i=0;i<1000;++i){g_eax=tag;g_edx=i;g_ecx=0;g_ebp=tag+1;GamePkgX86Fault fault;
        assert(gamepkg_x86_run(op_div4_0,&fault)==1&&fault.eax==tag&&fault.edx==i&&fault.ebp==tag+1&&fault.eip==0x10000);}
    return 0;
}
static void threads(void){
#ifdef _WIN32
    HANDLE ids[8];for(unsigned i=0;i<8;++i){ids[i]=CreateThread(NULL,0,worker,(void*)(uintptr_t)(i+1),0,NULL);assert(ids[i]);}
    for(unsigned i=0;i<8;++i){assert(WaitForSingleObject(ids[i],INFINITE)==WAIT_OBJECT_0);assert(CloseHandle(ids[i]));}
#else
    pthread_t ids[8];for(unsigned i=0;i<8;++i)assert(pthread_create(&ids[i],NULL,worker,(void*)(uintptr_t)(i+1))==0);
    for(unsigned i=0;i<8;++i)assert(pthread_join(ids[i],NULL)==0);
#endif
}
static unsigned physical_faults;
static void check_physical_fault(unsigned size,unsigned signed_op,uint32_t lo,uint32_t hi,uint32_t divisor){
#ifndef _WIN32
    pid_t pid=fork();assert(pid>=0);
    if(pid==0){struct rlimit limit={0,0};assert(setrlimit(RLIMIT_CORE,&limit)==0);signal(SIGFPE,SIG_DFL);hardware(size,signed_op,&lo,&hi,divisor);_exit(0);}
    int status;assert(waitpid(pid,&status,0)==pid);assert(WIFSIGNALED(status)&&WTERMSIG(status)==SIGFPE);++physical_faults;
#else
    (void)size;(void)signed_op;(void)lo;(void)hi;(void)divisor;
#endif
}
int main(void){
    for(uint32_t lo=0;lo<65536;++lo)for(uint32_t divisor=0;divisor<256;++divisor){check_div1_0(0xa5c30000u|lo,0xbcd01234u,divisor);check_idiv1_0(0xa5c30000u|lo,0xbcd01234u,divisor);}
    uint32_t edges[]={0,1,2,0x7f,0x80,0xff,0x100,0x7fff,0x8000,0xffff,0x10000,0x7fffffff,0x80000000,0xfffffffe,0xffffffff};
    uint32_t random=0x12345678u;
    for(unsigned i=0;i<8192;++i){random=random*1664525u+1013904223u;uint32_t lo=i<3375?edges[(i/225)%15]:random;random=random*1664525u+1013904223u;uint32_t hi=i<3375?edges[(i/15)%15]:random;random=random*1664525u+1013904223u;uint32_t divisor=i<3375?edges[i%15]:random;
"#);
    for (function, _, _) in &names {
        c.push_str(&format!("check_{function}(lo,hi,divisor);\n"));
    }
    c.push_str(r#"}
    for(unsigned size=1;size<=4;size*=2)for(unsigned sign=0;sign<2;++sign){
        check_physical_fault(size,sign,1,0,0);
        check_physical_fault(size,sign,sign&&size==1?0x7fffu:0xffffffffu,sign?(size==2?0x7fffu:0x7fffffffu):0xffffffffu,1);
    }
    check_physical_fault(4,1,0,0x80000000u,0xffffffffu);
    check_physical_fault(2,1,0,0x8000u,0xffffffffu);
    check_physical_fault(1,1,0x8000u,0,0xffffffffu);
    GamePkgX86Fault fault;
    assert(gamepkg_x86_run(NULL,&fault)==-1&&gamepkg_x86_run(normal,NULL)==-1);
    g_eax=10;g_edx=0;g_ecx=0;g_esp=0x800;g_ebp=0x1234;g_eflags=0x246;
    assert(gamepkg_x86_run(sub_00020000,&fault)==1&&fault.eip==0x20100&&fault.ebp==0xabcdef12u&&fault.esp==0x7fc&&fault.eax==10&&fault.edx==0&&fault.eflags==0x246);
    assert(g_eax==10&&g_esp==0x7fc&&g_ebp==0xabcdef12u&&xbox_MmioLoad(0x7fc,4)==0x2000a);
    assert(gamepkg_x86_run(nested,&fault)==1&&outer_resumed==1&&fault.eip==0x33333&&fault.ebp==0x44444);
    assert(gamepkg_x86_run(normal,&fault)==0);threads();assert(gamepkg_x86_run(normal,&fault)==0);
    printf("native division: %" PRIu64 " hardware successes, %" PRIu64 " checked guest faults, %u physical hardware faults; nested generated calls, boundary restoration and8threads passed\n",hardware_checks,guest_fault_checks,physical_faults);return 0;
}
"#);
    c
}

#[test]
fn synthetic_division_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_DIVISION_FIXTURE") {
        std::fs::write(path, c).unwrap();
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn generated_division_and_fault_boundary_match_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("division.c");
    let binary = dir.path().join("division");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let output = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O3",
            "-mno-red-zone",
            "-pthread",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-fsanitize=undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg("-I")
        .arg(&runtime)
        .arg(&source)
        .arg(runtime.join("gamepkg_x86_fault.c"))
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
