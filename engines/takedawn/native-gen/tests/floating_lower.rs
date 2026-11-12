use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};

fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let decoded = decoder.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        decoded.instructions().len() == 1
            && usize::from(decoded.instructions()[0].size) == bytes.len(),
        "incomplete SSE comparison/control fixture"
    );
    instruction(&decoded.instructions()[0])
}
#[test]
fn comparison_control_rejects_wrong_width_prefix_segment_and_untrusted_operands() {
    for bytes in [
        &[0x66, 0x0f, 0x2f, 0xc1][..],
        &[0xf3, 0x0f, 0x2f, 0xc1],
        &[0xf0, 0x0f, 0xae, 0x16],
        &[0x65, 0x0f, 0x2e, 0x06],
    ] {
        assert!(emit(bytes).is_err());
    }
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let decoded = decoder.linear(&[0x0f, 0x2f, 0x06], 0x10000, &[]).unwrap();
    for mode in 0..4 {
        let mut bad = decoded.instructions()[0].clone();
        match mode {
            0 => bad.operands[0].reg = Some("xmm8".into()),
            1 => bad.operands[1].mem_size = 16,
            2 => bad.operands[1].expr_src = Some("bad()".into()),
            _ => bad.operands.clear(),
        }
        assert!(instruction(&bad).is_err());
    }
}

fn fixture() -> String {
    let mut c=String::from("#define _GNU_SOURCE 1\n#include <stdint.h>\n#include <inttypes.h>\n#include <stddef.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <assert.h>\n#include <string.h>\n#include \"gamepkg_x86_fault.h\"\n#include \"gamepkg_x86_sse.h\"\n");
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\ntypedef union{uint32_t u[4];float f[4];double d[2];}RecompXmm;\nuint32_t xbox_MmioLoad(uint32_t,int);\nvoid xbox_MmioStore(uint32_t,uint32_t,int);\n");
    }
    c.push_str(r#"
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <ucontext.h>
#include <unistd.h>
#endif
RECOMP_TLS uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
RECOMP_TLS int g_df;
RECOMP_TLS RecompXmm g_xmm0,g_xmm1,g_xmm2,g_xmm3,g_xmm4,g_xmm5,g_xmm6,g_xmm7;
static unsigned char memory[4096];
static unsigned reads,writes,completed;
static int fail_read,fail_write;
static uint64_t comparisons,controls,unmasked,mask_cases;
uint32_t xbox_MmioLoad(uint32_t address,int width){
    if(fail_read)gamepkg_x86_general_protection(0xbad100,0xbad101);
    assert(width==4&&address<=sizeof(memory)-4);++reads;
    uint32_t v=0;for(unsigned i=0;i<4;++i)v|=(uint32_t)memory[address+i]<<(i*8u);return v;
}
void xbox_MmioStore(uint32_t address,uint32_t v,int width){
    if(fail_write)gamepkg_x86_general_protection(0xbad200,0xbad201);
    assert(width==4&&address<=sizeof(memory)-4);++writes;
    for(unsigned i=0;i<4;++i)memory[address+i]=(unsigned char)(v>>(i*8u));
}
static uint32_t hardware_mxcsr(void){uint32_t v;__asm__ volatile("stmxcsr %0":"=m"(v));return v;}
static uint32_t hardware_mask(void){
    _Alignas(16)unsigned char area[512]={0};__asm__ volatile("fxsave %0":"=m"(area));
    uint32_t mask=0;for(unsigned i=0;i<4;++i)mask|=(uint32_t)area[28+i]<<(8u*i);return mask?mask:0xffbfu;
}
static void hardware_compare(uint32_t a,uint32_t b,uint32_t initial,int quiet,uint32_t *flags,uint32_t *status){
    uint32_t old=hardware_mxcsr(),after;uintptr_t image;
    if(quiet){
        __asm__ volatile("push %[image]\n\tpopf\n\tldmxcsr %[initial]\n\tmovss %[a],%%xmm0\n\tucomiss %[b],%%xmm0\n\tpushf\n\tpop %[flags]\n\tstmxcsr %[status]\n\tldmxcsr %[old]"
        :[flags]"=&r"(image),[status]"=m"(after):[image]"r"((uintptr_t)0xad7),[a]"m"(a),[b]"m"(b),[initial]"m"(initial),[old]"m"(old):"xmm0","cc");
    }else{
        __asm__ volatile("push %[image]\n\tpopf\n\tldmxcsr %[initial]\n\tmovss %[a],%%xmm0\n\tcomiss %[b],%%xmm0\n\tpushf\n\tpop %[flags]\n\tstmxcsr %[status]\n\tldmxcsr %[old]"
        :[flags]"=&r"(image),[status]"=m"(after):[image]"r"((uintptr_t)0xad7),[a]"m"(a),[b]"m"(b),[initial]"m"(initial),[old]"m"(old):"xmm0","cc");
    }*flags=(uint32_t)image;*status=after;
}
"#);
    for (name, opcode, quiet) in [("comiss", 0x2f, 0), ("ucomiss", 0x2e, 1)] {
        for memory in [false, true] {
            let function = format!("{name}_{}", u8::from(memory));
            let code = emit(&[0x0f, opcode, if memory { 0x06 } else { 0xc1 }]).unwrap();
            c.push_str(&format!(
                "static void op_{function}(void){{uint32_t ebp=g_ebp;{code}++completed;}}\n"
            ));
            c.push_str(&format!(r#"
static void check_{function}(uint32_t a,uint32_t b,uint32_t initial){{
    uint32_t flags,status;hardware_compare(a,b,initial,{quiet},&flags,&status);
    uint32_t host=hardware_mxcsr(),prior=0x200202u|((a^b)&0xcd5u);
    RecompXmm left={{.u={{a,0x7f800001u,0x80000000u,0xffffffffu}}}},right={{.u={{b,7,8,9}}}};
    g_xmm0=left;g_xmm1=right;g_eflags=prior;g_ebp=0x12345678;g_esi=0x301;
    xbox_MmioStore(g_esi,b,4);reads=writes=completed=0;gamepkg_x86_set_mxcsr(initial,0,0);
    GamePkgX86Fault fault;assert(gamepkg_x86_run(op_{function},&fault)==0);
    if((g_eflags&0x8d5u)!=(flags&0x8d5u)||gamepkg_x86_get_mxcsr()!=status){{fprintf(stderr,"{function} a=%08x b=%08x initial=%08x flags=%08x/%08x status=%08x/%08x\n",a,b,initial,g_eflags,flags,gamepkg_x86_get_mxcsr(),status);abort();}}
    assert(((g_eflags^prior)&~0x8d5u)==0&&memcmp(&g_xmm0,&left,sizeof(left))==0&&memcmp(&g_xmm1,&right,sizeof(right))==0&&reads=={reads}u&&writes==0&&completed==1&&hardware_mxcsr()==host);++comparisons;
}}
"#,reads=u8::from(memory)));
        }
    }
    for (name, modrm) in [("load", 0x16), ("store", 0x1e)] {
        let code = emit(&[0x0f, 0xae, modrm]).unwrap();
        let frame = if name == "load" {
            "uint32_t ebp=g_ebp;"
        } else {
            ""
        };
        c.push_str(&format!(
            "static void op_{name}(void){{{frame}{code}++completed;}}\n"
        ));
    }
    c.push_str(r#"
static void check_control(uint32_t value){
    uint32_t old=hardware_mxcsr(),real;
    __asm__ volatile("ldmxcsr %[value]\n\tstmxcsr %[real]\n\tldmxcsr %[old]" :[real]"=m"(real):[value]"m"(value),[old]"m"(old));
    g_esi=0x303;g_eflags=0x8d7;xbox_MmioStore(g_esi,value,4);reads=writes=completed=0;
    GamePkgX86Fault fault;assert(gamepkg_x86_run(op_load,&fault)==0&&gamepkg_x86_get_mxcsr()==real&&reads==1&&writes==0&&completed==1&&g_eflags==0x8d7);
    reads=writes=completed=0;assert(gamepkg_x86_run(op_store,&fault)==0&&reads==0&&writes==1&&completed==1&&xbox_MmioLoad(g_esi,4)==real&&hardware_mxcsr()==old);++controls;
}
static void check_unmasked(uint32_t a,uint32_t b,uint32_t initial,int quiet,uint32_t raised){
    gamepkg_x86_set_mxcsr(initial,0,0);g_xmm0.u[0]=a;g_xmm1.u[0]=b;RecompXmm before=g_xmm0;
    g_eflags=0x8d7;g_ebp=0x1234;completed=0;GamePkgX86Fault fault;
    int error=gamepkg_x86_run(quiet?op_ucomiss_0:op_comiss_0,&fault);
    assert(error==1&&fault.vector==19&&fault.eip==0x10000&&fault.ebp==0x1234&&fault.eflags==0x8d7&&fault.mxcsr==(initial|raised)&&gamepkg_x86_get_mxcsr()==(initial|raised)&&g_eflags==0x8d7&&completed==0&&memcmp(&g_xmm0,&before,sizeof(before))==0);++unmasked;
}
static void check_masks(uint32_t a,uint32_t b,uint32_t initial,int quiet,int memory_source){
    uint32_t flags,status;hardware_compare(a,b,0x1f80,quiet,&flags,&status);uint32_t raised=status&63u;
    uint32_t expected=initial|raised;int fails=(raised&~(initial>>7))!=0;
    gamepkg_x86_set_mxcsr(initial,0,0);g_xmm0.u[0]=a;g_xmm1.u[0]=b;RecompXmm before=g_xmm0;
    g_eflags=0x8d7;g_ebp=0x1234;g_esi=0x301;xbox_MmioStore(g_esi,b,4);reads=writes=completed=0;GamePkgX86Fault fault;
    void(*op)(void)=quiet?(memory_source?op_ucomiss_1:op_ucomiss_0):(memory_source?op_comiss_1:op_comiss_0);
    int result=gamepkg_x86_run(op,&fault);
    assert(result==fails&&gamepkg_x86_get_mxcsr()==expected&&memcmp(&g_xmm0,&before,sizeof(before))==0&&reads==(unsigned)memory_source&&writes==0);
    if(fails)assert(fault.vector==19&&fault.mxcsr==expected&&fault.eflags==0x8d7&&g_eflags==0x8d7&&completed==0);
    else assert((g_eflags&0x8d5u)==(flags&0x8d5u)&&completed==1);
    ++mask_cases;
}
#ifndef _WIN32
static int pipe_output;
static void signal_context(int signal,siginfo_t *info,void *raw){
    (void)info;ucontext_t *context=(ucontext_t*)raw;
    const unsigned char *fp=(const unsigned char*)context->uc_mcontext.fpregs;
    size_t at=offsetof(struct _fpstate,mxcsr);uint32_t mx=0;
    for(unsigned i=0;i<4;++i)mx|=(uint32_t)fp[at+i]<<(8u*i);
    uint32_t values[4]={(uint32_t)signal,(uint32_t)context->uc_mcontext.gregs[REG_TRAPNO],(uint32_t)context->uc_mcontext.gregs[REG_EFL],mx};
    if(write(pipe_output,values,sizeof(values))!=(ssize_t)sizeof(values)){_exit(90);}
    _exit(0);
}
static unsigned physical;
static void physical_compare(uint32_t a,uint32_t b,uint32_t initial,int quiet,uint32_t raised){
    int pipes[2];assert(pipe(pipes)==0);pid_t pid=fork();assert(pid>=0);
    if(pid==0){close(pipes[0]);pipe_output=pipes[1];struct rlimit limit={0,0};assert(setrlimit(RLIMIT_CORE,&limit)==0);
        struct sigaction action;memset(&action,0,sizeof(action));action.sa_flags=SA_SIGINFO;action.sa_sigaction=signal_context;sigemptyset(&action.sa_mask);assert(sigaction(SIGFPE,&action,NULL)==0);
        uintptr_t image=0xad7;__asm__ volatile("push %0\n\tpopf"::"r"(image):"cc");
        uint32_t flags,status;hardware_compare(a,b,initial,quiet,&flags,&status);_exit(91);
    }
    close(pipes[1]);uint32_t values[4]={0};ssize_t got=read(pipes[0],values,sizeof(values));close(pipes[0]);int status;assert(waitpid(pid,&status,0)==pid);
    assert(got==(ssize_t)sizeof(values)&&WIFEXITED(status)&&WEXITSTATUS(status)==0&&values[0]==SIGFPE&&values[1]==19&&(values[2]&0x8d5u)==0x8d5u&&values[3]==(initial|raised));++physical;
}
static void physical_reserved_load(uint32_t value){
    int pipes[2];assert(pipe(pipes)==0);pid_t pid=fork();assert(pid>=0);
    if(pid==0){close(pipes[0]);pipe_output=pipes[1];struct rlimit limit={0,0};assert(setrlimit(RLIMIT_CORE,&limit)==0);
        struct sigaction action;memset(&action,0,sizeof(action));action.sa_flags=SA_SIGINFO;action.sa_sigaction=signal_context;sigemptyset(&action.sa_mask);assert(sigaction(SIGSEGV,&action,NULL)==0);
        uint32_t initial=0x1f80;__asm__ volatile("ldmxcsr %0\n\tldmxcsr %1"::"m"(initial),"m"(value));_exit(91);
    }
    close(pipes[1]);uint32_t values[4]={0};ssize_t got=read(pipes[0],values,sizeof(values));close(pipes[0]);int status;assert(waitpid(pid,&status,0)==pid);
    if(!(got==(ssize_t)sizeof(values)&&WIFEXITED(status)&&WEXITSTATUS(status)==0&&values[0]==SIGSEGV&&values[1]==13&&values[3]==0x1f80)){
        fprintf(stderr,"reserved value=%08x bytes=%ld status=%x signal=%u vector=%u mxcsr=%08x\n",value,(long)got,status,values[0],values[1],values[3]);abort();
    }++physical;
}
#else
static unsigned physical;
static void physical_compare(uint32_t a,uint32_t b,uint32_t initial,int quiet,uint32_t raised){(void)a;(void)b;(void)initial;(void)quiet;(void)raised;}
static void physical_reserved_load(uint32_t value){(void)value;}
#endif
#ifdef _WIN32
static DWORD WINAPI worker(void *raw){
#else
static void *worker(void *raw){
#endif
    uint32_t tag=(uint32_t)(uintptr_t)raw;
    assert(gamepkg_x86_get_mxcsr()==0x1f80);
    for(unsigned i=0;i<1000;++i){uint32_t value=0x1f80|((tag&3)<<13)|(i&63);gamepkg_x86_set_mxcsr(value,0,0);assert(gamepkg_x86_compare_f32(0x3f800000,0x40000000,0,0,0)==1&&gamepkg_x86_get_mxcsr()==value);}
    return 0;
}
static void threads(void){
#ifdef _WIN32
    HANDLE ids[8];for(unsigned i=0;i<8;++i){ids[i]=CreateThread(NULL,0,worker,(void*)(uintptr_t)i,0,NULL);assert(ids[i]);}for(unsigned i=0;i<8;++i){assert(WaitForSingleObject(ids[i],INFINITE)==WAIT_OBJECT_0);assert(CloseHandle(ids[i]));}
#else
    pthread_t ids[8];for(unsigned i=0;i<8;++i)assert(pthread_create(&ids[i],NULL,worker,(void*)(uintptr_t)i)==0);for(unsigned i=0;i<8;++i)assert(pthread_join(ids[i],NULL)==0);
#endif
}
int main(void){
    assert(gamepkg_x86_get_mxcsr()==0x1f80);
    uint32_t host_mask=hardware_mask();unsigned host_extensions=0;
    uint32_t edges[]={0,0x80000000u,1,0x80000001u,0x007fffffu,0x807fffffu,0x00800000u,0x80800000u,0x3f800000u,0xbf800000u,0x7f7fffffu,0xff7fffffu,0x7f800000u,0xff800000u,0x7f800001u,0xff800001u,0x7fc00000u,0xffc00000u,0x7fffffffu,0xffffffffu};
    uint32_t random=0x12345678;
    for(unsigned i=0;i<16784;++i){random=random*1664525u+1013904223u;uint32_t a=i<400?edges[i/20]:random;random=random*1664525u+1013904223u;uint32_t b=i<400?edges[i%20]:random;
    for(unsigned round=0;round<4;++round)for(unsigned flush=0;flush<2;++flush){uint32_t initial=0x1f80|(round<<13)|(flush<<15)|((a^b)&63);
"#);
    for name in ["comiss_0", "comiss_1", "ucomiss_0", "ucomiss_1"] {
        c.push_str(&format!("check_{name}(a,b,initial);\n"));
    }
    c.push_str(r#"}}
    for(uint32_t value=0;value<65536;++value)if(!(value&0x40))check_control(value);
    for(unsigned a=0;a<20;++a)for(unsigned b=0;b<20;++b)for(unsigned masks=0;masks<4;++masks)for(unsigned quiet=0;quiet<2;++quiet)for(unsigned mem=0;mem<2;++mem){
        uint32_t initial=(0x1f80u&~0x180u)|(masks<<7)|((edges[a]^edges[b])&63u);check_masks(edges[a],edges[b],initial,(int)quiet,(int)mem);
    }
    uint32_t initial=0x1f80&~0x80u;
    check_unmasked(0x7fc00001,1,initial,0,1);physical_compare(0x7fc00001,1,initial,0,1);
    check_unmasked(1,0xff800001,initial,1,1);physical_compare(1,0xff800001,initial,1,1);
    initial=0x1f80&~0x100u;
    check_unmasked(1,0x3f800000,initial,0,2);physical_compare(1,0x3f800000,initial,0,2);
    check_unmasked(0x80000001,0,initial,1,2);physical_compare(0x80000001,0,initial,1,2);
    initial=0x1f80&~0x180u;
    check_unmasked(1,0x7f800001,initial,0,1);physical_compare(1,0x7f800001,initial,0,1);
    check_unmasked(0x7f800001,1,initial,1,1);physical_compare(0x7f800001,1,initial,1,1);
    /* Old unmasked sticky flags do not raise a fresh condition. */
    gamepkg_x86_set_mxcsr(0x1e01,0,0);assert(gamepkg_x86_compare_f32(0x3f800000,0x3f800000,0,0,0)==0x40);
    GamePkgX86Fault fault;
    for(unsigned bit=6;bit<32;++bit){if(bit>6&&bit<16)continue;gamepkg_x86_set_mxcsr(0x1f80,0,0);g_esi=0x303;g_ebp=0x1234;g_eflags=0x8d7;xbox_MmioStore(g_esi,UINT32_C(1)<<bit,4);completed=reads=writes=0;
        assert(gamepkg_x86_run(op_load,&fault)==1&&fault.vector==13&&fault.eip==0x10000&&fault.mxcsr==0x1f80&&gamepkg_x86_get_mxcsr()==0x1f80&&g_eflags==0x8d7&&completed==0&&reads==1&&writes==0);
        /* Guest profile rejection is unconditional. Modern host DAZ and AMD
         * MM extensions are not a physical reserved-bit oracle. */
        uint32_t value=UINT32_C(1)<<bit;
        if(!(host_mask&value))physical_reserved_load(value);
        else{uint32_t old_host=hardware_mxcsr(),observed;__asm__ volatile("ldmxcsr %[value]\n\tstmxcsr %[observed]\n\tldmxcsr %[old]" :[observed]"=m"(observed):[value]"m"(value),[old]"m"(old_host));assert(observed==value);++host_extensions;}
    }
    g_esi=0x303;fail_read=1;assert(gamepkg_x86_run(op_load,&fault)==1&&fault.eip==0xbad100&&gamepkg_x86_get_mxcsr()==0x1f80);fail_read=0;
    g_eflags=0x8d7;g_xmm0.u[0]=0x7f800001u;fail_read=1;assert(gamepkg_x86_run(op_comiss_1,&fault)==1&&fault.eip==0xbad100&&gamepkg_x86_get_mxcsr()==0x1f80&&g_eflags==0x8d7);fail_read=0;
    uint32_t old=xbox_MmioLoad(g_esi,4);fail_write=1;assert(gamepkg_x86_run(op_store,&fault)==1&&fault.eip==0xbad200&&gamepkg_x86_get_mxcsr()==0x1f80);fail_write=0;assert(xbox_MmioLoad(g_esi,4)==old);
    gamepkg_x86_set_mxcsr(0x1f80,0,0);threads();assert(gamepkg_x86_get_mxcsr()==0x1f80);
    printf("native scalar compare: %" PRIu64 " hardware comparisons, %" PRIu64 " valid MXCSR round trips, %" PRIu64 " mask cases, %" PRIu64 " checked SIMD faults, %u physical fault contexts; host_mask=%08x host_extensions=%u; reserved/load/store/thread tests passed\n",comparisons,controls,mask_cases,unmasked,physical,host_mask,host_extensions);return 0;
}
"#);
    c
}
#[test]
fn synthetic_scalar_compare_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_FLOATING_FIXTURE") {
        std::fs::write(path, c).unwrap();
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn scalar_comparison_and_guest_controls_match_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("floating.c");
    let binary = dir.path().join("floating");
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
        .arg(runtime.join("gamepkg_x86_sse.c"))
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
