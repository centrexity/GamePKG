/* Synthetic x87/MMX shared-state oracle. No game data or C float model. */
#define _GNU_SOURCE 1
/* FXRSTOR overwrites compiler-held XMM temporaries as well as x87 state. */
#ifndef GAMEPKG_TEST_FX_CLOBBERS
#ifdef __x86_64__
#define GAMEPKG_TEST_FX_CLOBBERS "memory","xmm0","xmm1","xmm2","xmm3","xmm4","xmm5","xmm6","xmm7","xmm8","xmm9","xmm10","xmm11","xmm12","xmm13","xmm14","xmm15"
#else
#define GAMEPKG_TEST_FX_CLOBBERS "memory","xmm0","xmm1","xmm2","xmm3","xmm4","xmm5","xmm6","xmm7"
#endif
#endif
#include <stdint.h>
#include <stdio.h>
#include <assert.h>
#include <string.h>
#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_fault.h"
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <signal.h>
#include <ucontext.h>
#include <sys/wait.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <unistd.h>
#endif
_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
static unsigned reads,writes,completed,fail_read;
static uint64_t memory;
uint32_t xbox_MmioLoad(uint32_t address,int width){assert(width==4 && (address==1025 || address==1029));if(++reads==fail_read)gamepkg_x86_general_protection(0xbad100,0xabcdef12);return address==1025?(uint32_t)memory:(uint32_t)(memory>>32);}
void xbox_MmioStore(uint32_t address,uint32_t value,int width){assert(address==1025 || address==1029);++writes;if(width==2){memory=(memory&~UINT64_C(65535))|value;return;}assert(width==4);if(address==1025)memory=(memory&UINT64_C(0xffffffff00000000))|value;else memory=(memory&UINT64_C(0xffffffff))|((uint64_t)value<<32);}
typedef struct {_Alignas(16) unsigned char b[512];}Image;
static unsigned get16(const unsigned char *b){return b[0]|((unsigned)b[1]<<8);}
static uint64_t get64(const unsigned char *b){uint64_t v=0;for(unsigned i=0;i<8;++i)v|=(uint64_t)b[i]<<(8*i);return v;}
static void save(Image *p){__asm__ volatile("fxsave %0":"=m"(*p)::"memory");}
static void restore(const Image *p){__asm__ volatile("fxrstor %0"::"m"(*p): GAMEPKG_TEST_FX_CLOBBERS);}
static void seed(const uint64_t *values,unsigned top){
    __asm__ volatile("fninit\n\tmovq 0(%0),%%mm0\n\tmovq 8(%0),%%mm1\n\tmovq 16(%0),%%mm2\n\tmovq 24(%0),%%mm3\n\tmovq 32(%0),%%mm4\n\tmovq 40(%0),%%mm5\n\tmovq 48(%0),%%mm6\n\tmovq 56(%0),%%mm7\n\temms"::"r"(values):"mm0","mm1","mm2","mm3","mm4","mm5","mm6","mm7","memory");
    for(unsigned i=0;i<top;++i)__asm__ volatile("fincstp":::"memory");
}
static void import(const Image *image){GamePkgX87State *s=gamepkg_x86_x87_state();memset(s,0,sizeof(*s));s->control=get16(image->b);s->status=get16(image->b+2);unsigned top=(s->status>>11)&7;
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;s->significand[p]=get64(image->b+32+16*i);s->sign_exponent[p]=(uint16_t)get16(image->b+40+16*i);if(!(image->b[4]&(1u<<p)))s->tags|=(uint16_t)(3u<<(2*p));}
}
static void compare(const Image *image){const GamePkgX87State *s=gamepkg_x86_x87_state();assert(s->control==get16(image->b) && s->status==get16(image->b+2));unsigned top=(s->status>>11)&7,valid=0;
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;if(((s->tags>>(p*2))&3)!=3)valid|=1u<<p;assert(s->significand[p]==get64(image->b+32+16*i) && s->sign_exponent[p]==get16(image->b+40+16*i));}assert(valid==image->b[4]);
}
static void (*const generated[37][3])(void);
static void (*const physical[37][3])(const void*);
static void (*const extras[15])(void);
static void (*const real_extras[15])(uint64_t*,uint32_t*);
static void control_cw(void);static void control_sw(void);static void control_ax(void);static void control_wait(void);
static uint64_t random64(uint64_t *v){*v^=*v<<13;*v^=*v>>7;*v^=*v<<17;return *v;}
#ifdef _WIN32
static DWORD WINAPI worker(void *raw){
#else
static void *worker(void *raw){
#endif
    uint64_t tag=(uint64_t)(uintptr_t)raw;GamePkgX87State *s=gamepkg_x86_x87_state();assert(s->control==0x37f&&s->tags==0xffff&&s->status==0);
    for(unsigned i=0;i<1000;++i){uint64_t value=(tag<<48)|i;gamepkg_x86_mmx_commit((uint32_t)tag,value);assert(s->significand[tag]==value&&s->sign_exponent[tag]==0xffff&&s->tags==0&&!(s->status&0x3800));gamepkg_x86_mmx_empty();assert(s->tags==0xffff&&s->significand[tag]==value);}
    return 0;
}
static void threads(void){
#ifdef _WIN32
    HANDLE h[8];for(unsigned i=0;i<8;++i){h[i]=CreateThread(0,0,worker,(void*)(uintptr_t)i,0,0);assert(h[i]);}assert(WaitForMultipleObjects(8,h,1,INFINITE)==WAIT_OBJECT_0);for(unsigned i=0;i<8;++i)CloseHandle(h[i]);
#else
    pthread_t h[8];for(unsigned i=0;i<8;++i)assert(pthread_create(&h[i],0,worker,(void*)(uintptr_t)i)==0);for(unsigned i=0;i<8;++i)assert(pthread_join(h[i],0)==0);
#endif
}
#ifndef _WIN32
static void pending_handler(int signal,siginfo_t *info,void *raw){(void)info;ucontext_t *u=raw;_exit(signal==SIGFPE&&u->uc_mcontext.gregs[REG_TRAPNO]==16?20:99);}
static void low_unpack_guard(void){
    for(unsigned op=31;op<=33;++op){pid_t child=fork();assert(child>=0);
        if(child==0){struct rlimit core={0,0};assert(setrlimit(RLIMIT_CORE,&core)==0);long page=sysconf(_SC_PAGESIZE);assert(page>4);unsigned char *map=mmap(0,(size_t)page*2,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);assert(map!=MAP_FAILED&&mprotect(map+page,(size_t)page,PROT_NONE)==0);memset(map+page-4,0x81,4);__asm__ volatile("fninit":::"memory");physical[op][2]((const void*)(map+page-4));_exit(0);}
        int status;assert(waitpid(child,&status,0)==child&&WIFEXITED(status)&&WEXITSTATUS(status)==0);
    }
}
#else
static void low_unpack_guard(void){}
#endif
static void pending_cases(void){
    const unsigned statuses[]={0,1,0x80,0x81,0x8000,0x8081};
    for(unsigned flag=0;flag<6;++flag)for(unsigned mask=0;mask<2;++mask)for(unsigned j=0;j<6;++j)for(unsigned op=0;op<4;++op){
        unsigned cw=mask?(0x37f&~(1u<<flag)):0x37f,sw=(statuses[j]&~1u)|((statuses[j]&1u)<<flag);int expected=(sw&~cw&63)!=0;
#ifndef _WIN32
        pid_t child=fork();assert(child>=0);if(child==0){struct sigaction action={0};action.sa_sigaction=pending_handler;action.sa_flags=SA_SIGINFO;sigemptyset(&action.sa_mask);assert(sigaction(SIGFPE,&action,0)==0);Image image;__asm__ volatile("fninit":::"memory");save(&image);image.b[0]=(unsigned char)cw;image.b[1]=(unsigned char)(cw>>8);image.b[2]=(unsigned char)sw;image.b[3]=(unsigned char)(sw>>8);restore(&image);
            if(op==0)__asm__ volatile("fwait":::"memory");else if(op==1)__asm__ volatile("emms":::"memory");else if(op==2)__asm__ volatile("movd %%mm0,%%eax":::"eax","memory");else __asm__ volatile("pxor %%mm0,%%mm0":::"mm0","memory");_exit(10);}
        int status;assert(waitpid(child,&status,0)==child&&WIFEXITED(status)&&WEXITSTATUS(status)==(expected?20:10));
#endif
        GamePkgX87State *state=gamepkg_x86_x87_state();state->control=(uint16_t)cw;state->status=(uint16_t)sw;completed=reads=writes=0;GamePkgX86Fault fault;void (*entry)(void)=op==0?control_wait:op==1?extras[0]:op==2?extras[4]:generated[3][1];
        assert(gamepkg_x86_run(entry,&fault)==expected && completed==(expected?0u:1u) && reads==0&&writes==0);if(expected)assert(fault.vector==16&&fault.eip==0x10000&&fault.ebp==0xabcdef12);
    }
}
int main(void){Image original,before,after;save(&original);uint64_t rng=0x123456789abcdef,checks=0;GamePkgX86Fault fault;
    for(unsigned top=0;top<8;++top)for(unsigned op=0;op<37;++op)for(unsigned mode=0;mode<3;++mode)for(unsigned sample=0;sample<1024;++sample){
        uint64_t values[8];for(unsigned i=0;i<8;++i)values[i]=random64(&rng);if(sample<256){values[0]=UINT64_C(0x0101010101010101)*sample;values[1]=~values[0];}
        static const uint64_t edges[]={0,UINT64_MAX,UINT64_C(0x8000800080008000),UINT64_C(0x7fff7fff7fff7fff),UINT64_C(0x8000000080000000),UINT64_C(0x7fffffff7fffffff),UINT64_C(0x007f008000ff0100),UINT64_C(0xff7fff80ff00feff)};
        if(sample<64){values[0]=edges[sample/8];values[1]=edges[sample%8];}
        uint64_t source=sample<64?edges[sample%8]:random64(&rng);seed(values,top);save(&before);physical[op][mode](&source);save(&after);restore(&original);
        import(&before);memory=source;g_esi=1025;g_eflags=0x8d7;reads=writes=completed=0;assert(gamepkg_x86_run(generated[op][mode],&fault)==0 && completed==1 && reads==(mode==2?(op>=31&&op<=33?1u:2u):0u)&&writes==0&&g_eflags==0x8d7);compare(&after);++checks;
    }
    for(unsigned top=0;top<8;++top)for(unsigned op=0;op<15;++op){uint64_t values[8];for(unsigned i=0;i<8;++i)values[i]=random64(&rng);seed(values,top);save(&before);uint64_t source=random64(&rng),expected_memory=source;uint32_t expected=0xdeadbeef;real_extras[op](&expected_memory,&expected);save(&after);restore(&original);import(&before);memory=source;g_eax=0xdeadbeef;g_esi=1025;g_eflags=0x8d7;reads=writes=completed=0;assert(gamepkg_x86_run(extras[op],&fault)==0&&completed==1&&g_eflags==0x8d7);compare(&after);assert(g_eax==expected && memory==expected_memory);++checks;}
    /* Exhaust all byte pairs for each saturating byte add/subtract. Compare
     * register and memory sources against real MMX, including every lane. */
    const unsigned byte_ops[]={16,18,20,22};
    for(unsigned opindex=0;opindex<4;++opindex)for(unsigned x=0;x<256;++x)for(unsigned y=0;y<256;++y)for(unsigned mode=0;mode<3;mode+=2){
        unsigned op=byte_ops[opindex];uint64_t values[8]={0};values[0]=UINT64_C(0x0101010101010101)*x;values[1]=UINT64_C(0x0101010101010101)*y;uint64_t source=values[1];seed(values,0);save(&before);physical[op][mode](&source);save(&after);restore(&original);import(&before);memory=source;g_esi=1025;g_eflags=0x8d7;reads=writes=completed=0;assert(gamepkg_x86_run(generated[op][mode],&fault)==0&&completed==1&&reads==(mode==2?(op>=31&&op<=33?1u:2u):0u)&&writes==0&&g_eflags==0x8d7);compare(&after);++checks;
    }
    /* Non-waiting control/status operations must work with a pending error. */
    GamePkgX87State *s=gamepkg_x86_x87_state();gamepkg_x86_x87_init();s->status=0xf8ff;s->control=0x37f;g_esi=1025;g_eax=0x12340000;memory=UINT64_MAX;completed=0;control_cw();assert((memory&65535)==0x37f);control_sw();assert((memory&65535)==0xf8ff);control_ax();assert(g_eax==0x1234f8ff);
    for(unsigned op=0;op<37;++op)for(unsigned mode=0;mode<3;++mode){s->status=0xf8ff;s->control=0x37e;GamePkgX87State old=*s;reads=writes=completed=0;assert(gamepkg_x86_run(generated[op][mode],&fault)==1&&fault.vector==16&&fault.eip==0x10000&&fault.ebp==0xabcdef12&&reads==0&&writes==0&&completed==0&&memcmp(&old,s,sizeof(old))==0);}
    completed=0;assert(gamepkg_x86_run(control_wait,&fault)==1 && fault.vector==16 && completed==0);
    for(unsigned op=0;op<37;++op)for(unsigned failure=1;failure<=(op>=31&&op<=33?1u:2u);++failure){gamepkg_x86_x87_init();s->status=0x2800;s->significand[0]=0x1234;GamePkgX87State old=*s;reads=writes=completed=0;fail_read=failure;assert(gamepkg_x86_run(generated[op][2],&fault)==1&&fault.eip==0xbad100&&writes==0&&completed==0&&memcmp(&old,s,sizeof(old))==0);fail_read=0;}
    for(unsigned status=0;status<65536;++status){Image input=original;input.b[0]=0x7f;input.b[1]=3;input.b[2]=(unsigned char)status;input.b[3]=(unsigned char)(status>>8);restore(&input);__asm__ volatile("fnclex":::"memory");save(&after);restore(&original);s->status=(uint16_t)status;gamepkg_x86_x87_clear();assert(s->status==get16(after.b+2));}
    low_unpack_guard();pending_cases();GamePkgX87State parent=*s;threads();assert(memcmp(&parent,s,sizeof(parent))==0);
    restore(&original);printf("MMX/x87: %llu generated/hardware register-state cases; deferred faults, read rollback,65536 hardware clear-status cases,288 pending-exception gates and8000 threaded transitions passed\n",(unsigned long long)checks);return 0;
}
