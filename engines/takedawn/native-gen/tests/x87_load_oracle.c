#define _GNU_SOURCE 1
/* FXRSTOR overwrites compiler-held XMM temporaries as well as x87 state. */
#ifndef GAMEPKG_TEST_FX_CLOBBERS
#ifdef __x86_64__
#define GAMEPKG_TEST_FX_CLOBBERS "memory","xmm0","xmm1","xmm2","xmm3","xmm4","xmm5","xmm6","xmm7","xmm8","xmm9","xmm10","xmm11","xmm12","xmm13","xmm14","xmm15"
#else
#define GAMEPKG_TEST_FX_CLOBBERS "memory","xmm0","xmm1","xmm2","xmm3","xmm4","xmm5","xmm6","xmm7"
#endif
#endif
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_fault.h"
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <signal.h>
#include <ucontext.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <sys/resource.h>
#include <unistd.h>
#endif
_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
typedef struct {_Alignas(16) unsigned char b[512];}Image;
static _Thread_local unsigned reads,completed,fail_read;
static void distinct_wait(void){gamepkg_x86_x87_wait(0x20000,0x9876);++completed;}
static _Thread_local unsigned char memory[16];
uint32_t xbox_MmioLoad(uint32_t address,int width){assert(address>=1025&&address+(unsigned)width<=1041&&(width==2||width==4));if(++reads==fail_read)gamepkg_x86_general_protection(0xbad100,0xbad101);uint32_t v=0;for(int i=0;i<width;++i)v|=(uint32_t)memory[address-1025+(unsigned)i]<<(8*i);return v;}
static uint64_t get64(const unsigned char *b){uint64_t v=0;for(unsigned i=0;i<8;++i)v|=(uint64_t)b[i]<<(8*i);return v;}
static unsigned get16(const unsigned char *b){return b[0]|((unsigned)b[1]<<8);}
static void put64(unsigned char *b,uint64_t value){for(unsigned i=0;i<8;++i)b[i]=(unsigned char)(value>>(8*i));}
static uint64_t random64(uint64_t *v){*v^=*v<<13;*v^=*v>>7;*v^=*v<<17;return *v;}
static void (*const generated[51])(void);
static void (*const physical[51])(const unsigned char*);
static const unsigned opcodes[51],widths[51],modes[51],kinds[51];
static const uint64_t input64[]={0,1,UINT64_MAX,UINT64_C(0x8000000000000000),UINT64_C(0x7fffffffffffffff),UINT64_C(0x3ff0000000000000),UINT64_C(0x7ff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0x000fffffffffffff),UINT64_C(0x0010000000000000),UINT64_C(0xfff8000000000007),0x8000,0x7fff,UINT64_C(0x80000000),UINT64_C(0x7fffffff),UINT64_C(0xffff8000),UINT64_C(0xffffffff80000000),UINT64_C(0xffffffff),UINT64_C(0x7fffffff7fffffff)};
static const uint32_t input32[]={0,1,UINT32_MAX,0x80000000,0x7fffffff,0x3f800000,0x7f800000,0x7f800001,0x7fc00001,0x7fffff,0x800000,0xffc00007};
static void prepare(Image *image,GamePkgX87State *s,unsigned op,unsigned cw,unsigned top,unsigned empties,uint64_t *rng){
    static const uint64_t sig[]={0,UINT64_C(0x8000000000000000),UINT64_C(0xc000000000000007),UINT64_C(0x8000000000000001),1,UINT64_C(0x123456789)};
    static const unsigned se[]={0,0x3fff,0x7fff,0x7fff,0,0xffff};
    memset(s,0,sizeof(*s));s->control=(uint16_t)cw;s->status=(uint16_t)((top<<11)|0x4700);s->instruction_pointer=0x40000;s->data_pointer=0x50000;s->opcode=0x123;
    image->b[0]=(unsigned char)cw;image->b[1]=(unsigned char)(cw>>8);image->b[2]=(unsigned char)s->status;image->b[3]=(unsigned char)(s->status>>8);image->b[4]=0;
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7,which=(op+cw+top+i)%6;uint64_t v=sig[which];unsigned exponent=se[which];s->significand[p]=v;s->sign_exponent[p]=(uint16_t)exponent;
        int empty=empties==2||(empties==1&&i==7)||(empties==3&&(i&1));unsigned tag=empty?3:!exponent&&!v?1:exponent&&exponent!=0x7fff&&(v>>63)?0:2;s->tags|=(uint16_t)(tag<<(2*p));if(!empty)image->b[4]|=(unsigned char)(1u<<p);put64(image->b+32+16*i,v);image->b[40+16*i]=(unsigned char)exponent;image->b[41+16*i]=(unsigned char)(exponent>>8);
    }
    unsigned which=(op+cw+top+empties)%(sizeof(input64)/sizeof(*input64));uint64_t bits=kinds[op]==0?input32[which%(sizeof(input32)/sizeof(*input32))]:input64[which];if((op+cw+top+empties)%5==0)bits=random64(rng);put64(memory,bits);unsigned upper=(which<6?se[which]:which*1231)&65535;memory[8]=(unsigned char)upper;memory[9]=(unsigned char)(upper>>8);
    g_eax=0x1234;g_esi=1025;g_edi=0;g_ebx=0;g_fs_base=0;g_ebp=0x777;g_esp=0x800;g_eflags=0x8d7;
    if(modes[op]==1){g_fs_base=0xfffffff0;g_esi+=16;}if(modes[op]==2){g_ebx=0xffff;g_esi=0x10402;}if(modes[op]==3){g_edi=0x20000001;g_esi-=40;}
}
static void compare(const Image *after,unsigned op,unsigned cw,unsigned top,unsigned empties){
    const GamePkgX87State *s=gamepkg_x86_x87_state();unsigned sw=get16(after->b+2),newtop=(sw>>11)&7,valid=0;
    if((s->status&~0x4500u)!=(sw&~0x4500u)){fprintf(stderr,"load op%u kind%u cw%04x top%u empty%u status actual%04x expected%04x bits%016llx\n",op,kinds[op],cw,top,empties,s->status,sw,(unsigned long long)get64(memory));assert(0);}
    for(unsigned i=0;i<8;++i){unsigned p=(newtop+i)&7;uint64_t sig=get64(after->b+32+16*i);unsigned se=get16(after->b+40+16*i);if(s->significand[p]!=sig||s->sign_exponent[p]!=se){fprintf(stderr,"load op%u kind%u cw%04x top%u empty%u p%u value actual%04x:%016llx expected%04x:%016llx bits%016llx\n",op,kinds[op],cw,top,empties,p,s->sign_exponent[p],(unsigned long long)s->significand[p],se,(unsigned long long)sig,(unsigned long long)get64(memory));assert(0);}if(((s->tags>>(2*p))&3)!=3)valid|=1u<<p;}
    assert(valid==after->b[4]&&s->control==cw&&s->instruction_pointer==0x10000&&s->opcode==opcodes[op]&&s->data_pointer==(widths[op]?(modes[op]==1?1041u:1025u):0x50000u));
}
#ifdef _WIN32
static DWORD WINAPI worker(void *raw){
#else
static void *worker(void *raw){
#endif
    unsigned tag=(unsigned)(uintptr_t)raw;GamePkgX87State *state=gamepkg_x86_x87_state();assert(state->control==0x37f&&state->tags==0xffff&&state->status==0);Image original,input,after;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef)+tag;
    for(unsigned n=0;n<1000;++n){unsigned op=(n%6)*6+(tag+n)%6,cw=0x40|((n+tag)&63)|(((n+tag)&3)<<8)|(((n+tag)&3)<<10),top=n&7,empty=n&3;input=original;GamePkgX87State initial;prepare(&input,&initial,op,cw,top,empty,&rng);__asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);physical[op](memory);__asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(original): GAMEPKG_TEST_FX_CLOBBERS);*state=initial;reads=completed=0;GamePkgX86Fault fault;assert(gamepkg_x86_run(generated[op],&fault)==0&&completed==1);compare(&after,op,cw,top,empty);}
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
static Image fault_before;
static unsigned expected_vector;
static void fault_handler(int signal,siginfo_t *info,void *raw){
    (void)info;ucontext_t *u=raw;const struct _libc_fpstate *fp=u->uc_mcontext.fpregs;if(!fp)_exit(90);
#ifdef __x86_64__
    unsigned sw=fp->swd,cw=fp->cwd,valid=fp->ftw&255u;
    uint64_t ip=fp->rip;
#else
    unsigned sw=(unsigned)fp->sw&65535u,cw=(unsigned)fp->cw&65535u,valid=0;for(unsigned p=0;p<8;++p)if(((fp->tag>>(2*p))&3)!=3)valid|=1u<<p;
    uint64_t ip=fp->ipoff;
#endif
    if((unsigned)u->uc_mcontext.gregs[REG_TRAPNO]!=expected_vector || signal!=(expected_vector==14?SIGSEGV:SIGFPE))_exit(91);
    if(cw!=get16(fault_before.b)){_exit(94);} if(sw!=get16(fault_before.b+2)){_exit(95);} if(valid!=fault_before.b[4]){_exit(96);} uint64_t before_ip=get64(fault_before.b+8);
#ifndef __x86_64__
    before_ip=(uint32_t)before_ip;
#endif
    if(ip!=before_ip){char message[]="load context FIP=0000000000000000\n";static const char digits[]="0123456789abcdef";for(unsigned j=0;j<16;++j)message[17+j]=digits[(ip>>(60-4*j))&15];if(write(2,message,sizeof(message)-1)!=(ssize_t)sizeof(message)-1){_exit(98);}_exit(97);}
    for(unsigned i=0;i<8;++i){uint64_t sig=0;for(unsigned j=0;j<4;++j)sig|=(uint64_t)fp->_st[i].significand[j]<<(16*j);if(sig!=get64(fault_before.b+32+16*i)||fp->_st[i].exponent!=get16(fault_before.b+40+16*i))_exit(92);}
    _exit(0);
}
static unsigned physical_faults;
static void fault_priority(const Image *original){
    for(unsigned kind=0;kind<6;++kind)for(unsigned scenario=0;scenario<3;++scenario){pid_t child=fork();assert(child>=0);if(child==0){struct rlimit core={0,0};assert(setrlimit(RLIMIT_CORE,&core)==0);long page=sysconf(_SC_PAGESIZE);assert(page>32);unsigned char *map=mmap(0,(size_t)page*2,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);assert(map!=MAP_FAILED&&mprotect(map+page,(size_t)page,PROT_NONE)==0);unsigned width=widths[kind*6],part=width==10?8:width/2;unsigned char *ptr=map+page-part;memset(ptr,0xff,part);fault_before=*original;GamePkgX87State initial;uint64_t rng=1;prepare(&fault_before,&initial,kind*6,scenario?0x40:0x37f,2,scenario?0:1,&rng);fault_before.b[8]=0;fault_before.b[9]=0;fault_before.b[10]=4;fault_before.b[11]=0;for(unsigned j=12;j<16;++j)fault_before.b[j]=0;
        if(scenario==2){fault_before.b[2]=0x81;fault_before.b[3]|=0x80;}expected_vector=scenario==2?16:14;struct sigaction action={0};action.sa_sigaction=fault_handler;action.sa_flags=SA_SIGINFO;sigemptyset(&action.sa_mask);assert(sigaction(SIGSEGV,&action,0)==0&&sigaction(SIGFPE,&action,0)==0);__asm__ volatile("fxrstor %0"::"m"(fault_before): GAMEPKG_TEST_FX_CLOBBERS);if(scenario<2)__asm__ volatile("fnop":::"memory");
#ifdef __x86_64__
        __asm__ volatile("fxsave64 %0":"=m"(fault_before)::"memory");
#else
        __asm__ volatile("fxsave %0":"=m"(fault_before)::"memory");
#endif
        /* Compare the live FXSAVE baseline, not the requested FXRSTOR image.
         * Some CPUs report zero FIP in FXSAVE even when FNSTENV reports the
         * preceding FNOP address. The virtual-state tests independently assert
         * nonzero instruction/data pointers and exact deferred-fault EIPs. */
        physical[kind*6](ptr);_exit(93);}
        int status;assert(waitpid(child,&status,0)==child);if(!WIFEXITED(status)||WEXITSTATUS(status)!=0){fprintf(stderr,"load physical fault kind%u scenario%u status%x\n",kind,scenario,status);assert(0);}++physical_faults;
    }
}
#else
static unsigned physical_faults;
static void fault_priority(const Image *original){(void)original;}
#endif
int main(void){Image original,input,after;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef),checks=0,queued=0;GamePkgX86Fault fault;
    for(unsigned op=0;op<51;++op)for(unsigned masks=0;masks<64;++masks)for(unsigned pc=0;pc<4;++pc)for(unsigned rc=0;rc<4;++rc)for(unsigned top=0;top<8;++top)for(unsigned empty=0;empty<4;++empty){
        input=original;unsigned cw=0x40|masks|(pc<<8)|(rc<<10);GamePkgX87State initial;prepare(&input,&initial,op,cw,top,empty,&rng);
        __asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);physical[op](memory);__asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(original): GAMEPKG_TEST_FX_CLOBBERS);
        *gamepkg_x86_x87_state()=initial;reads=completed=0;assert(gamepkg_x86_run(generated[op],&fault)==0&&completed==1&&g_eax==0x1234&&g_eflags==0x8d7&&g_esp==0x800);compare(&after,op,cw,top,empty);
        assert(reads==(widths[op]==10?3:widths[op]==8?2:widths[op]?1:0));++checks;
        if(gamepkg_x86_x87_state()->status&~cw&63u){GamePkgX87State before=*gamepkg_x86_x87_state();completed=reads=0;assert(gamepkg_x86_run(generated[op],&fault)==1&&fault.vector==16&&fault.eip==0x10000&&fault.ebp==(modes[op]==4?1025u:0xabcdef12)&&completed==0&&reads==0&&memcmp(&before,gamepkg_x86_x87_state(),sizeof(before))==0);assert(gamepkg_x86_run(distinct_wait,&fault)==1&&fault.eip==0x20000&&fault.ebp==0x9876&&completed==0&&memcmp(&before,gamepkg_x86_x87_state(),sizeof(before))==0);++queued;}
    }
    for(unsigned value=0;value<65536;++value){
        input=original;GamePkgX87State initial;prepare(&input,&initial,18,0x37f,0,1,&rng);put64(memory,value);
        __asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);physical[18](memory);__asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(original): GAMEPKG_TEST_FX_CLOBBERS);*gamepkg_x86_x87_state()=initial;reads=completed=0;assert(gamepkg_x86_run(generated[18],&fault)==0&&completed==1&&reads==1);compare(&after,18,0x37f,0,1);++checks;
    }
    for(unsigned op=0;op<36;++op)for(unsigned n=1;n<=(widths[op]==10?3u:widths[op]==8?2u:1u);++n){input=original;GamePkgX87State initial;prepare(&input,&initial,op,0x37f,0,0,&rng);*gamepkg_x86_x87_state()=initial;fail_read=n;reads=completed=0;assert(gamepkg_x86_run(generated[op],&fault)==1&&fault.vector==13&&fault.eip==0xbad100&&completed==0&&reads==n&&memcmp(&initial,gamepkg_x86_x87_state(),sizeof(initial))==0);fail_read=0;}
    GamePkgX87State parent=*gamepkg_x86_x87_state();threads();assert(memcmp(&parent,gamepkg_x86_x87_state(),sizeof(parent))==0);fault_priority(&original);
    printf("x87 loads: %llu hardware/generated cases; %llu queued-fault/staged-read gates,8000 threaded cases and%u physical fault contexts passed\n",(unsigned long long)checks,(unsigned long long)queued,physical_faults);return 0;
}
