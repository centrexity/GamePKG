use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let decoded = decoder.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        decoded.instructions().len() == 1
            && usize::from(decoded.instructions()[0].size) == bytes.len(),
        "incomplete SSE predicate fixture"
    );
    instruction(&decoded.instructions()[0])
}
#[test]
fn predicate_emission_rejects_wrong_forms_and_untrusted_operands() {
    for bytes in [
        &[0x66, 0x0f, 0xc2, 0xc1, 0][..],
        &[0xf2, 0x0f, 0xc2, 0xc1, 0],
        &[0xf0, 0xf3, 0x0f, 0xc2, 0x00, 0],
        &[0x65, 0xf3, 0x0f, 0xc2, 0x00, 0],
    ] {
        assert!(emit(bytes).is_err());
    }
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let decoded = decoder
        .linear(&[0xf3, 0x0f, 0xc2, 0xc1, 0], 0x10000, &[])
        .unwrap();
    for index in 0..2 {
        let mut bad = decoded.instructions()[0].clone();
        bad.operands[index].expr = Some("bad()".into());
        assert!(instruction(&bad).is_err());
    }
    let mut bad = decoded.instructions()[0].clone();
    bad.operands[0].reg = Some("xmm8".into());
    assert!(instruction(&bad).is_err());
    let mut bad = decoded.instructions()[0].clone();
    bad.mnemonic = "cmpneqss".into();
    assert!(instruction(&bad).is_err());
    let mut bad = decoded.instructions()[0].clone();
    bad.operands[1].expr_src = Some("bad()".into());
    assert!(instruction(&bad).is_err());
}

#[test]
fn legacy_predicate_aliases_and_generic_metadata_agree_and_reserved_values_fail() {
    let decoder = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let names = [
        "cmpeq", "cmplt", "cmple", "cmpunord", "cmpneq", "cmpnlt", "cmpnle", "cmpord",
    ];
    for scalar in [false, true] {
        for predicate in 0..=255u8 {
            let mut bytes = vec![0x0f, 0xc2, 0xc1, predicate];
            if scalar {
                bytes.insert(0, 0xf3);
            }
            if predicate >= 8 {
                assert!(emit(&bytes).is_err());
                continue;
            }
            let decoded = decoder.linear(&bytes, 0x10000, &[]).unwrap();
            let alias = &decoded.instructions()[0];
            let suffix = if scalar { "ss" } else { "ps" };
            assert_eq!(
                alias.mnemonic,
                format!("{}{suffix}", names[usize::from(predicate)])
            );
            let mut generic = alias.clone();
            generic.mnemonic = format!("cmp{suffix}");
            generic
                .operands
                .push(gamepkg_takedawn_native_gen::decode::Operand {
                    kind: "imm",
                    imm: Some(u32::from(predicate)),
                    ..Default::default()
                });
            assert_eq!(instruction(alias).unwrap(), instruction(&generic).unwrap());
            generic.operands[2].imm = Some(u32::from(predicate) ^ 1);
            assert!(instruction(&generic).is_err());
            generic.operands[2].imm = Some(u32::from(predicate));
            generic.operands[2].expr = Some("escape()".into());
            assert!(instruction(&generic).is_err());
        }
    }
}

fn fixture() -> String {
    let mut c=String::from("#define _GNU_SOURCE 1\n#include <stdint.h>\n#include <inttypes.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <assert.h>\n#include <stddef.h>\n#include <string.h>\n#include \"gamepkg_x86_fault.h\"\n#include \"gamepkg_x86_sse.h\"\n");
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    } else {
        c.push_str("typedef struct {uint32_t u[4];}RecompXmm;\n");
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
_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
_Thread_local RecompXmm g_xmm0,g_xmm1;
typedef struct Bits {_Alignas(16)uint32_t v[4];}Bits;
typedef struct Work {Bits a,b,result;uint32_t lanes,op,completed;}Work;
static _Thread_local Work work;
static uint64_t comparisons;
static FILE *corpus;
static uint64_t corpus_rows;
static void corpus_word(uint32_t value){unsigned char bytes[4];for(unsigned i=0;i<4;++i)bytes[i]=(unsigned char)(value>>(8u*i));assert(fwrite(bytes,1,4,corpus)==4);}
static void corpus_record(const Bits *a,const Bits *b,const Bits *result,uint32_t status,uint32_t initial,uint32_t lanes,uint32_t op,uint32_t fault){
    if(!corpus){return;}
    corpus_word(op);corpus_word(lanes);corpus_word(initial);corpus_word(fault);
    for(unsigned i=0;i<4;++i)corpus_word(a->v[i]);
    for(unsigned i=0;i<4;++i)corpus_word(b->v[i]);
    for(unsigned i=0;i<4;++i)corpus_word(result->v[i]);
    corpus_word(status);++corpus_rows;
}
static unsigned char memory[4096];static unsigned reads,writes;static int fail_read;
uint32_t xbox_MmioLoad(uint32_t address,int width){++reads;if(fail_read==(int)reads)gamepkg_x86_general_protection(0xbad100,0xbad101);assert(width==4&&address<=sizeof(memory)-4);uint32_t v=0;for(unsigned i=0;i<4;++i)v|=(uint32_t)memory[address+i]<<(i*8u);return v;}
void xbox_MmioStore(uint32_t address,uint32_t value,int width){assert(width==4&&address<=sizeof(memory)-4);++writes;for(unsigned i=0;i<4;++i)memory[address+i]=(unsigned char)(value>>(i*8u));}
static uint32_t host_control(void){uint32_t value;__asm__ volatile("stmxcsr %0":"=m"(value));return value;}
static void invoke(void){gamepkg_x86_predicate_f32(work.a.v,work.b.v,work.result.v,work.lanes,work.op,0x10000,0xabcdef12);++work.completed;}
static void hardware(const Bits *a,const Bits *b,Bits *result,uint32_t initial,uint32_t lanes,uint32_t op,uint32_t *status){
    uint32_t old=host_control(),after;
"#);
    for op in 0..8 {
        for (lanes, suffix) in [(1, "ss"), (4, "ps")] {
            c.push_str(&format!(r#"
    if(op=={op}&&lanes=={lanes}){{
        __asm__ volatile("ldmxcsr %[initial]\n\tmovups %[a],%%xmm0\n\tcmp{suffix} ${op},%[b],%%xmm0\n\tmovups %%xmm0,%[result]\n\tstmxcsr %[after]\n\tldmxcsr %[old]"
        :[result]"=m"(*result),[after]"=m"(after):[a]"m"(*a),[b]"m"(*b),[initial]"m"(initial),[old]"m"(old):"xmm0");*status=after;return;
    }}
"#));
        }
    }
    c.push_str("abort();}\n");
    let mut emitted = Vec::new();
    for op in 0..8 {
        let opcode = 0xc2;
        for lanes in [1, 4] {
            for mode in 0..5 {
                let mut bytes: Vec<_> = (lanes == 1)
                    .then_some(0xf3)
                    .into_iter()
                    .chain([
                        0x0f,
                        opcode,
                        match mode {
                            0 => 0xc1,
                            1 => 0x05,
                            2 => 0xc0,
                            3 => 0x06,
                            _ => 0x00,
                        },
                    ])
                    .collect();
                if mode == 1 {
                    bytes.extend(0x400u32.to_le_bytes());
                }
                if mode == 3 {
                    bytes.insert(0, 0x64);
                }
                if mode == 4 {
                    bytes.insert(0, 0x67);
                }
                bytes.push(op as u8);
                let code = emit(&bytes).unwrap();
                let name = format!("emitted_{op}_{lanes}_{mode}");
                emitted.push((name.clone(), op, lanes, mode));
                c.push_str(&format!(
                    "static void {name}(void){{uint32_t ebp=g_ebp;{code}++work.completed;}}\n"
                ));
            }
        }
    }
    c.push_str("static void (*emitted(uint32_t op,uint32_t lanes,uint32_t mode))(void){\n");
    for (name, op, lanes, mode) in emitted {
        c.push_str(&format!(
            "if(op=={op}&&lanes=={lanes}&&mode=={mode})return {name};\n"
        ));
    }
    c.push_str("abort();}\n");
    c.push_str(r#"
static uint64_t emitted_checks;
static int uses_memory(unsigned mode){return mode==1||mode>=3;}
static void prepare_address(unsigned mode){g_fs_base=mode==3?0xfffffff0:0;g_esi=mode==3?0x410:0x30401;g_ebx=0xffff;}
static void check_emitted(const Bits *a,const Bits *b,uint32_t initial,uint32_t lanes,uint32_t op,uint32_t mode){
    Bits rhs=mode==2?*a:*b,real;uint32_t status;hardware(a,&rhs,&real,initial,lanes,op,&status);
    memcpy(g_xmm0.u,a,sizeof(*a));memcpy(g_xmm1.u,b,sizeof(*b));for(unsigned i=0;i<4;++i)xbox_MmioStore(0x400+4*i,b->v[i],4);reads=writes=work.completed=0;
    prepare_address(mode);g_ebp=0xabcdef12;g_eflags=0x8d7;gamepkg_x86_set_mxcsr(initial,0,0);GamePkgX86Fault fault;
    assert(gamepkg_x86_run(emitted(op,lanes,mode),&fault)==0&&memcmp(g_xmm0.u,&real,sizeof(real))==0&&memcmp(g_xmm1.u,b,sizeof(*b))==0&&gamepkg_x86_get_mxcsr()==status&&g_eflags==0x8d7&&reads==(uses_memory(mode)?lanes:0)&&writes==0&&work.completed==1);++emitted_checks;
}
static void emitted_faults(void){
    for(unsigned op=0;op<8;++op)for(unsigned packed=0;packed<2;++packed)for(unsigned mode=0;mode<5;++mode){
        Bits a={{0x7f800001,0x80000000,9,10}},b={{0x7f800001,7,8,9}};
        memcpy(g_xmm0.u,&a,sizeof(a));memcpy(g_xmm1.u,&b,sizeof(b));for(unsigned i=0;i<4;++i)xbox_MmioStore(0x400+4*i,b.v[i],4);reads=writes=work.completed=0;
        prepare_address(mode);gamepkg_x86_set_mxcsr(0x1f00,0,0);g_ebp=0xabcdef12;g_eflags=0x8d7;GamePkgX86Fault fault;uint32_t lanes=packed?4:1;
        Bits rhs=mode==2?a:b,unused;uint32_t status;hardware(&a,&rhs,&unused,0x1f80,lanes,op,&status);
        assert(gamepkg_x86_run(emitted(op,lanes,mode),&fault)==1&&fault.vector==19&&fault.eip==0x10000&&fault.ebp==0xabcdef12&&fault.mxcsr==(0x1f00|(status&7u))&&memcmp(g_xmm0.u,&a,sizeof(a))==0&&g_eflags==0x8d7&&reads==(uses_memory(mode)?lanes:0)&&writes==0&&work.completed==0);
    }
    for(unsigned op=0;op<8;++op)for(unsigned packed=0;packed<2;++packed)for(unsigned mode=1;mode<5;++mode)if(uses_memory(mode))for(unsigned fail=1;fail<=(packed?4u:1u);++fail){
        Bits a={{0x12345678,0xabcdef01,0x7f800001,0x80000000}};
        memcpy(g_xmm0.u,&a,sizeof(a));gamepkg_x86_set_mxcsr(0x1f80,0,0);g_eflags=0x8d7;
        prepare_address(mode);reads=writes=work.completed=0;fail_read=(int)fail;GamePkgX86Fault fault;
        assert(gamepkg_x86_run(emitted(op,packed?4:1,mode),&fault)==1&&fault.eip==0xbad100&&fault.mxcsr==0x1f80&&memcmp(g_xmm0.u,&a,sizeof(a))==0&&g_eflags==0x8d7&&reads==fail&&writes==0&&work.completed==0);fail_read=0;
    }
    for(unsigned op=0;op<8;++op)for(unsigned mode=3;mode<5;++mode){
        prepare_address(mode);if(mode==3)++g_fs_base;else --g_ebx;
        reads=writes=work.completed=0;gamepkg_x86_set_mxcsr(0x1f00,0,0);
        Bits before;memcpy(&before,g_xmm0.u,sizeof(before));GamePkgX86Fault fault;
        assert(gamepkg_x86_run(emitted(op,4,mode),&fault)==1&&fault.vector==13&&fault.eip==0x10000&&fault.mxcsr==0x1f00&&reads==0&&writes==0&&work.completed==0&&!memcmp(g_xmm0.u,&before,sizeof(before)));
    }
}
static void check(const Bits *a,const Bits *b,uint32_t initial,uint32_t lanes,uint32_t op){
    Bits real;uint32_t status;hardware(a,b,&real,initial,lanes,op,&status);
    uint32_t host=host_control();work=(Work){.a=*a,.b=*b,.result=*a,.lanes=lanes,.op=op};
    gamepkg_x86_set_mxcsr(initial,0,0);g_eax=0x11112222;g_edx=0x33334444;g_eflags=0x8d7;
    GamePkgX86Fault fault;int error=gamepkg_x86_run(invoke,&fault);
    if(error||memcmp(&work.result,&real,sizeof(real))||gamepkg_x86_get_mxcsr()!=status){
        fprintf(stderr,"predicate op=%u lanes=%u initial=%04x error=%d status=%04x/%04x\n",op,lanes,initial,error,gamepkg_x86_get_mxcsr(),status);
        for(unsigned i=0;i<4;++i){fprintf(stderr,"lane%u a=%08x b=%08x actual=%08x expected=%08x\n",i,a->v[i],b->v[i],work.result.v[i],real.v[i]);}abort();
    }
    assert(work.completed==1&&g_eax==0x11112222&&g_edx==0x33334444&&g_eflags==0x8d7&&host_control()==host);
    for(unsigned alias=0;alias<2;++alias){
        Bits aa=*a,bb=*b;gamepkg_x86_predicate_f32(aa.v,bb.v,alias?bb.v:aa.v,lanes,op,0x10000,0xabcdef12);
        /* Scalar writes touch only lane0 of the supplied output buffer. */
        Bits expected=alias?*b:*a;for(unsigned i=0;i<lanes;++i)expected.v[i]=real.v[i];
        assert(!memcmp(alias?&bb:&aa,&expected,sizeof(expected))&&host_control()==host);
    }
    corpus_record(a,b,&real,status,initial,lanes,op,0);++comparisons;
}
typedef struct Report {uint32_t signal,vector,status;Bits result;}Report;
static unsigned physical_faults,physical_normals;
#ifndef _WIN32
static int pipe_output;
static void context_handler(int signal,siginfo_t *info,void *raw){
    (void)info;ucontext_t *context=(ucontext_t*)raw;const unsigned char *bytes=(const unsigned char*)context->uc_mcontext.fpregs;
    Report report={.signal=(uint32_t)signal,.vector=(uint32_t)context->uc_mcontext.gregs[REG_TRAPNO]};
    for(unsigned i=0;i<4;++i)report.status|=(uint32_t)bytes[offsetof(struct _fpstate,mxcsr)+i]<<(8u*i);
    for(unsigned lane=0;lane<4;++lane)for(unsigned i=0;i<4;++i)report.result.v[lane]|=(uint32_t)bytes[offsetof(struct _fpstate,_xmm)+lane*4+i]<<(8u*i);
    if(write(pipe_output,&report,sizeof(report))!=(ssize_t)sizeof(report)){_exit(90);}
    _exit(0);
}
static void physical(const Bits *a,const Bits *b,uint32_t initial,uint32_t lanes,uint32_t op){
    int pipes[2];assert(pipe(pipes)==0);pid_t pid=fork();assert(pid>=0);
    if(pid==0){close(pipes[0]);pipe_output=pipes[1];struct rlimit limit={0,0};assert(setrlimit(RLIMIT_CORE,&limit)==0);struct sigaction action;memset(&action,0,sizeof(action));action.sa_flags=SA_SIGINFO;action.sa_sigaction=context_handler;sigemptyset(&action.sa_mask);assert(sigaction(SIGFPE,&action,NULL)==0);
        Report report={0};hardware(a,b,&report.result,initial,lanes,op,&report.status);if(write(pipe_output,&report,sizeof(report))!=(ssize_t)sizeof(report))_exit(90);_exit(0);
    }
    close(pipes[1]);Report report={0};ssize_t got=read(pipes[0],&report,sizeof(report));close(pipes[0]);int child;assert(waitpid(pid,&child,0)==pid);assert(got==(ssize_t)sizeof(report)&&WIFEXITED(child)&&WEXITSTATUS(child)==0);
    work=(Work){.a=*a,.b=*b,.result=*a,.lanes=lanes,.op=op};gamepkg_x86_set_mxcsr(initial,0,0);g_eflags=0x8d7;
    GamePkgX86Fault fault;int error=gamepkg_x86_run(invoke,&fault);
    if(error!=(report.signal!=0)||gamepkg_x86_get_mxcsr()!=report.status||memcmp(&work.result,&report.result,sizeof(report.result))){fprintf(stderr,"fault predicate op=%u lanes=%u initial=%04x error=%d signal=%u status=%04x/%04x\n",op,lanes,initial,error,report.signal,gamepkg_x86_get_mxcsr(),report.status);for(unsigned i=0;i<4;++i)fprintf(stderr,"lane%u a=%08x b=%08x actual=%08x expected=%08x\n",i,a->v[i],b->v[i],work.result.v[i],report.result.v[i]);abort();}
    if(error){assert(report.signal==SIGFPE&&report.vector==19&&fault.vector==19&&fault.eip==0x10000&&fault.ebp==0xabcdef12&&fault.mxcsr==report.status&&work.completed==0);++physical_faults;}else{assert(work.completed==1);++physical_normals;}
    corpus_record(a,b,&report.result,report.status,initial,lanes,op,(uint32_t)error);
}
#else
static void physical(const Bits *a,const Bits *b,uint32_t initial,uint32_t lanes,uint32_t op){(void)a;(void)b;(void)initial;(void)lanes;(void)op;}
#endif
#ifdef _WIN32
static DWORD WINAPI worker(void *raw){
#else
static void *worker(void *raw){
#endif
    uint32_t tag=(uint32_t)(uintptr_t)raw;
    assert(gamepkg_x86_get_mxcsr()==0x1f80);
    for(unsigned i=0;i<1000;++i){Bits a={{0x3f800001,0x3f123456,0x3f200003,0x3f700000}},b={{0x40400000,0x3f777777,0x3f345678,0x3f000001}},real;
        uint32_t initial=0x1f80|((tag&3)<<13)|((i&1)<<15)|(i&63),status;uint32_t lanes=(i&1)?1:4,op=i%8;
        hardware(&a,&b,&real,initial,lanes,op,&status);work=(Work){.a=a,.b=b,.result=a,.lanes=lanes,.op=op};
        gamepkg_x86_set_mxcsr(initial,0,0);g_eflags=tag;
        GamePkgX86Fault fault;assert(gamepkg_x86_run(invoke,&fault)==0&&memcmp(&work.result,&real,sizeof(real))==0&&gamepkg_x86_get_mxcsr()==status&&g_eflags==tag);
    }return 0;
}
static void threads(void){
#ifdef _WIN32
    HANDLE ids[8];for(unsigned i=0;i<8;++i){ids[i]=CreateThread(NULL,0,worker,(void*)(uintptr_t)i,0,NULL);assert(ids[i]);}for(unsigned i=0;i<8;++i){assert(WaitForSingleObject(ids[i],INFINITE)==WAIT_OBJECT_0);assert(CloseHandle(ids[i]));}
#else
    pthread_t ids[8];for(unsigned i=0;i<8;++i)assert(pthread_create(&ids[i],NULL,worker,(void*)(uintptr_t)i)==0);for(unsigned i=0;i<8;++i)assert(pthread_join(ids[i],NULL)==0);
#endif
}
int main(void){
    const char *corpus_path=getenv("GAMEPKG_TAKEDAWN_PREDICATE_CORPUS");if(corpus_path){corpus=fopen(corpus_path,"wb");assert(corpus&&fwrite("GPKCMPv1",1,8,corpus)==8);corpus_word(0);}
    uint32_t edges[]={0,0x80000000u,1,0x80000001u,0x007fffffu,0x807fffffu,0x00800000u,0x80800000u,0x00800001u,0x80800001u,0x3f000000u,0x3f7fffffu,0x3f7ffffeu,0x3f800000u,0xbf800000u,0x40000000u,0xc0000000u,0x40400000u,0x7f7fffffu,0xff7fffffu,0x7f800000u,0xff800000u,0x7f800001u,0xff800001u,0x7fc00000u,0xffc00000u,0x7fc12345u,0x7fffffffu};
    unsigned count=sizeof(edges)/sizeof(edges[0]);uint32_t random=0x12345678;
    for(unsigned i=0;i<count*count+8192;++i){Bits a,b;for(unsigned lane=0;lane<4;++lane){random=random*1664525u+1013904223u;a.v[lane]=i<count*count?edges[(i/count+lane)%count]:random;random=random*1664525u+1013904223u;b.v[lane]=i<count*count?edges[(i+lane)%count]:random;}
        for(unsigned round=0;round<4;++round)for(unsigned flush=0;flush<2;++flush){uint32_t initial=0x1f80|(round<<13)|(flush<<15);for(unsigned op=0;op<8;++op){check(&a,&b,initial,1,op);check(&a,&b,initial,4,op);if(i<count*count||(i&63u)==0u)for(unsigned mode=0;mode<5;++mode){check_emitted(&a,&b,initial,1,op,mode);check_emitted(&a,&b,initial,4,op,mode);}}}
    }
    uint32_t special[32][3];for(unsigned p=0;p<8;++p){uint32_t pairs[4][2]={{0x7fc12345,1},{1,0x7f800001},{0,0x80000000},{1,0x80000001}};for(unsigned i=0;i<4;++i){special[4*p+i][0]=p;special[4*p+i][1]=pairs[i][0];special[4*p+i][2]=pairs[i][1];}}
    for(unsigned row=0;row<sizeof(special)/sizeof(special[0]);++row)for(unsigned setting=0;setting<8;++setting)for(unsigned mask=0;mask<(setting?8u:64u);++mask)for(unsigned packed=0;packed<2;++packed){Bits a={{special[row][1],0x7f800001,1,0x7f7fffff}},b={{special[row][2],0x3f800000,0,0x40000000}};uint32_t initial=0x1f80|((setting&3u)<<13u)|((setting>>2u)<<15u);uint32_t choices[]={0,0x80,0x100,0x200,0x400,0x800,0x1000,0x1f80};initial&=~(setting?choices[mask]:(mask<<7));physical(&a,&b,initial,packed?4:1,special[row][0]);}
    for(unsigned op=0;op<8;++op)for(unsigned packed=0;packed<2;++packed)for(unsigned setting=0;setting<8;++setting){Bits a={{0x3f800000,0x40000000,0x80000000,0x7f800000}},b={{0x40000000,0x3f800000,0,0xff800000}};physical(&a,&b,0x3fu|((setting&3)<<13)|((setting>>2)<<15),packed?4:1,op);}
    emitted_faults();
    gamepkg_x86_set_mxcsr(0x1f80,0,0);threads();assert(gamepkg_x86_get_mxcsr()==0x1f80);
    if(corpus){assert(corpus_rows<=UINT32_MAX&&fseek(corpus,8,SEEK_SET)==0);corpus_word((uint32_t)corpus_rows);assert(fclose(corpus)==0);}
    printf("software SSE predicate: %" PRIu64 " masked hardware comparisons, %" PRIu64 " emitted comparisons, %u physical faults, %u physical normals; emitted fault/load and8000 threaded hardware gates passed\n",comparisons,emitted_checks,physical_faults,physical_normals);return 0;
}
"#);
    c
}
#[test]
fn synthetic_software_predicate_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_PREDICATE_FIXTURE") {
        std::fs::write(path, c).unwrap();
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn software_sse_predicate_matches_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("predicate.c");
    let binary = dir.path().join("predicate");
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
