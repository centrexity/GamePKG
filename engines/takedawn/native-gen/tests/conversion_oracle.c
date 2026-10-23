/* Asset-free real x86 oracle. Appended functions are produced by the Rust
 * typed emitter, not hand-written substitutes. No guest C float casts. */
#define _GNU_SOURCE 1
#include <assert.h>
#include <stdint.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_sse.h"
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
typedef struct {uint32_t u[4];}RecompXmm;
static _Thread_local RecompXmm g_xmm0,g_xmm1;
static _Thread_local uint32_t local_ebp,observed_ebp;
static _Thread_local unsigned completed,reads;
static _Thread_local int fail_read;
static _Thread_local uint32_t memory_value;
static void (*selected(unsigned,unsigned))(void);
uint32_t xbox_MmioLoad(uint32_t address,int width){assert(address==1025 && width==4);++reads;if(fail_read)gamepkg_x86_general_protection(0xbad100,local_ebp);return memory_value;}
static uint32_t control(void){uint32_t value;__asm__ volatile("stmxcsr %0":"=m"(value));return value;}
static uint32_t hardware(uint32_t value,unsigned op,uint32_t initial,uint32_t *status){
    uint32_t old=control(),result=0xdecafbad;
    const RecompXmm seed={{0x12345678,0xabcdef01,0xdeadbeef,0x87654321}};
    __asm__ volatile("ldmxcsr %0"::"m"(initial));
    if(op==0)__asm__ volatile("movups %[seed],%%xmm0\n\tcvtsi2ss %[in],%%xmm0\n\tmovd %%xmm0,%[out]" :[out]"+a"(result):[in]"m"(value),[seed]"m"(seed):"xmm0","memory");
    else if(op==1)__asm__ volatile("movd %[in],%%xmm0\n\tcvtss2si %%xmm0,%[out]" :[out]"+a"(result):[in]"m"(value):"xmm0","memory");
    else __asm__ volatile("movd %[in],%%xmm0\n\tcvttss2si %%xmm0,%[out]" :[out]"+a"(result):[in]"m"(value):"xmm0","memory");
    *status=control();__asm__ volatile("ldmxcsr %0"::"m"(old));return result;
}
static uint64_t checks,emitted_checks,mask_checks;
static const uint32_t edges[]={0,1,0x80000000,0x80000001,0x007fffff,0x00800000,0x3effffff,0x3f000000,0x3f000001,0x3f800000,0x3fc00000,0x40200000,0x40600000,0xbeffffff,0xbf000000,0xbf000001,0xbfc00000,0xc0200000,0x4effffff,0x4f000000,0x4f000001,0xceffffff,0xcf000000,0xcf000001,0x7f800000,0xff800000,0x7f800001,0xff800001,0x7fc00001,0xffc00001,0x7fffffff,0xffffffff,0x00ffffff,0x01000001,0xfeffffff};
static void prepare(unsigned op,unsigned mode,uint32_t value,uint32_t initial){
    g_eax=op==0?value:0xdecafbad;g_esp=mode==3&&op==0?value:0x800;g_ebp=0xbadbad;
    local_ebp=mode==2&&op==0?value:0xabcdef12;observed_ebp=0xbadbad;
    g_ebx=0;g_esi=1025;g_fs_base=0;g_eflags=0x8d7;
    if(mode==4){g_fs_base=0xfffffff0;g_esi+=16;}
    if(mode==5){g_ebx=0xffff;g_esi=0x10402;}
    g_xmm0=(RecompXmm){{0x12345678,0xabcdef01,0xdeadbeef,0x87654321}};
    g_xmm1=(RecompXmm){{value,2,3,4}};memory_value=value;reads=completed=0;
    gamepkg_x86_set_mxcsr(initial,0,0);
}
static void check_emitted(unsigned op,unsigned mode,uint32_t value,uint32_t initial){
    uint32_t status,expected=hardware(value,op,initial|0x1f80,&status);status=(status&63u)|initial;
    prepare(op,mode,value,initial);GamePkgX86Fault fault;
    int error=gamepkg_x86_run(selected(op,mode),&fault);
    uint32_t raised=status&~initial&63u;
    /* Pre-existing flags do not retrigger, but a newly raised condition still
     * faults even when its sticky bit was already set. Use fresh masked output. */
    uint32_t clean_status;hardware(value,op,(initial&~63u)|0x1f80,&clean_status);raised=clean_status&63u;
    int should_fault=(raised&~(initial>>7u))!=0;
    assert(error==should_fault && gamepkg_x86_get_mxcsr()==status && g_eflags==0x8d7 && reads==((mode==1||mode>=4)?1u:0u));
    assert(g_xmm0.u[1]==0xabcdef01 && g_xmm0.u[2]==0xdeadbeef && g_xmm0.u[3]==0x87654321 && g_xmm1.u[0]==value);
    if(error){assert(fault.vector==19 && fault.eip==0x10000 && fault.ebp==local_ebp && fault.mxcsr==status && completed==0 && observed_ebp==0xbadbad);assert(g_xmm0.u[0]==0x12345678 && g_eax==(op==0?value:0xdecafbad) && g_esp==(mode==3&&op==0?value:0x800));}
    else {assert(completed==1);uint32_t actual=op==0?g_xmm0.u[0]:mode==2?observed_ebp:mode==3?g_esp:g_eax;assert(actual==expected);}
    ++emitted_checks;
}
static void check(uint32_t value,unsigned op,uint32_t initial){
    uint32_t status,expected=hardware(value,op,initial,&status),old=control();
    gamepkg_x86_set_mxcsr(initial,0,0);uint32_t actual=gamepkg_x86_convert_f32(value,op,0x10000,0xabcdef12);
    if(actual!=expected || gamepkg_x86_get_mxcsr()!=status){fprintf(stderr,"conversion op=%u value=%08x control=%04x actual=%08x/%04x expected=%08x/%04x\n",op,value,initial,actual,gamepkg_x86_get_mxcsr(),expected,status);abort();}
    assert(control()==old);++checks;
}
#ifndef _WIN32
static int output_pipe;
static void handler(int signal,siginfo_t *info,void *raw){
    (void)info;ucontext_t *u=raw;const unsigned char *fp=(const unsigned char*)u->uc_mcontext.fpregs;uint32_t report[5]={(uint32_t)signal,(uint32_t)u->uc_mcontext.gregs[REG_TRAPNO],0,0,0};
#ifdef __x86_64__
    report[4]=(uint32_t)u->uc_mcontext.gregs[REG_RAX];
#else
    report[4]=(uint32_t)u->uc_mcontext.gregs[REG_EAX];
#endif
    for(unsigned i=0;i<4;++i)report[3]|=(uint32_t)fp[offsetof(struct _fpstate,_xmm)+i]<<(8*i);
    for(unsigned i=0;i<4;++i)report[2]|=(uint32_t)fp[offsetof(struct _fpstate,mxcsr)+i]<<(8*i);
    if(write(output_pipe,report,sizeof(report))!=(ssize_t)sizeof(report)){_exit(90);}_exit(0);
}
static unsigned physical;
static void physical_fault(uint32_t value,unsigned op,uint32_t initial,uint32_t expected_status){
    int pipes[2];assert(pipe(pipes)==0);pid_t pid=fork();assert(pid>=0);
    if(pid==0){close(pipes[0]);output_pipe=pipes[1];struct rlimit core={0,0};assert(setrlimit(RLIMIT_CORE,&core)==0);struct sigaction action={0};action.sa_sigaction=handler;action.sa_flags=SA_SIGINFO;sigemptyset(&action.sa_mask);assert(sigaction(SIGFPE,&action,0)==0);uint32_t unused;hardware(value,op,initial,&unused);_exit(91);}
    close(pipes[1]);uint32_t report[5];ssize_t size=read(pipes[0],report,sizeof(report));close(pipes[0]);int child;assert(waitpid(pid,&child,0)==pid && WIFEXITED(child)&&WEXITSTATUS(child)==0 && size==(ssize_t)sizeof(report));assert(report[0]==SIGFPE && report[1]==19 && report[2]==expected_status);assert(report[3]==(op==0?0x12345678:value) && report[4]==0xdecafbad);++physical;
}
#else
static unsigned physical;
static void physical_fault(uint32_t value,unsigned op,uint32_t initial,uint32_t status){(void)value;(void)op;(void)initial;(void)status;}
#endif
#ifdef _WIN32
static DWORD WINAPI worker(void *raw){
#else
static void *worker(void *raw){
#endif
    unsigned tag=(unsigned)(uintptr_t)raw;assert(gamepkg_x86_get_mxcsr()==0x1f80);
    for(unsigned i=0;i<1000;++i){uint32_t initial=0x1f80|((tag&3)<<13)|((i&1)<<15)|(i&63),status,value=i*0x1234567u;unsigned op=i%3;uint32_t expected=hardware(value,op,initial,&status);gamepkg_x86_set_mxcsr(initial,0,0);assert(gamepkg_x86_convert_f32(value,op,0,0)==expected && gamepkg_x86_get_mxcsr()==status);}
    return 0;
}
static void threads(void){
#ifdef _WIN32
    HANDLE handles[8];for(unsigned i=0;i<8;++i){handles[i]=CreateThread(0,0,worker,(void*)(uintptr_t)i,0,0);assert(handles[i]);}assert(WaitForMultipleObjects(8,handles,1,INFINITE)==WAIT_OBJECT_0);for(unsigned i=0;i<8;++i)CloseHandle(handles[i]);
#else
    pthread_t handles[8];for(unsigned i=0;i<8;++i)assert(pthread_create(&handles[i],0,worker,(void*)(uintptr_t)i)==0);for(unsigned i=0;i<8;++i)assert(pthread_join(handles[i],0)==0);
#endif
}
int main(void){
    uint32_t random=0x12345678;
    for(unsigned op=0;op<3;++op)for(unsigned rc=0;rc<4;++rc)for(unsigned ftz=0;ftz<2;++ftz)for(unsigned sticky=0;sticky<2;++sticky){
        uint32_t initial=0x1f80|(rc<<13)|(ftz<<15)|(sticky?63u:0u);
        for(unsigned i=0;i<sizeof(edges)/sizeof(*edges);++i){check(edges[i],op,initial);for(unsigned mode=0;mode<6;++mode)check_emitted(op,mode,edges[i],initial);}
        for(unsigned i=0;i<32768;++i){random^=random<<13;random^=random>>17;random^=random<<5;check(random,op,initial);}
        for(unsigned exp=0;exp<256;++exp)for(unsigned sign=0;sign<2;++sign)for(unsigned low=0;low<8;++low){static const uint32_t mantissa[]={0,1,0x3fffff,0x400000,0x400001,0x7ffffd,0x7ffffe,0x7fffff};check((sign<<31)|(exp<<23)|mantissa[low],op,initial);}
    }
    for(unsigned op=0;op<3;++op)for(unsigned masks=0;masks<64;++masks)for(unsigned rc=0;rc<4;++rc)for(unsigned i=0;i<sizeof(edges)/sizeof(*edges);++i){
        uint32_t initial=(masks<<7)|(rc<<13);check_emitted(op,i%6,edges[i],initial);++mask_checks;
    }
    for(unsigned op=0;op<3;++op)for(unsigned rc=0;rc<4;++rc){uint32_t value=op==0?0x1000001u:0x3fc00000u;uint32_t initial=0x0f80|(rc<<13);physical_fault(value,op,initial,initial|32u);if(op){initial=0x1f00|(rc<<13);physical_fault(0x7fc00001,op,initial,initial|1u);}}
    for(unsigned op=0;op<3;++op)for(unsigned mode=1;mode<6;++mode){if(mode==2||mode==3)continue;prepare(op,mode,0x7f800001,0);fail_read=1;GamePkgX86Fault fault;assert(gamepkg_x86_run(selected(op,mode),&fault)==1 && fault.vector==13 && fault.eip==0xbad100 && fault.mxcsr==0 && completed==0 && g_xmm0.u[0]==0x12345678 && g_eax==(op==0?0x7f800001:0xdecafbad));fail_read=0;}
    gamepkg_x86_set_mxcsr(0x1f80,0,0);threads();assert(gamepkg_x86_get_mxcsr()==0x1f80);
    printf("conversion: %" PRIu64 " hardware cases, %" PRIu64 " emitted cases, %" PRIu64 " mask cases; %u physical SIMD faults; load priority and8000 threaded comparisons passed\n",checks,emitted_checks,mask_checks,physical);return 0;
}
