/* Synthetic MMX shift oracle: actual instructions, no game bytes. */
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
#include <stdlib.h>
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
static _Thread_local unsigned reads,completed,fail_read;
static _Thread_local uint64_t memory;
uint32_t xbox_MmioLoad(uint32_t address,int width){assert(width==4&&(address==1025||address==1029));if(++reads==fail_read)gamepkg_x86_general_protection(0xbad100,0xbad101);return address==1025?(uint32_t)memory:(uint32_t)(memory>>32);}
typedef struct {_Alignas(16) unsigned char b[512];}Image;
static unsigned get16(const unsigned char *b){return b[0]|((unsigned)b[1]<<8);}
static uint64_t get64(const unsigned char *b){uint64_t v=0;for(unsigned i=0;i<8;++i)v|=(uint64_t)b[i]<<(8*i);return v;}
static void put64(unsigned char *b,uint64_t v){for(unsigned i=0;i<8;++i)b[i]=(unsigned char)(v>>(8*i));}
static uint64_t random64(uint64_t *v){*v^=*v<<13;*v^=*v>>7;*v^=*v<<17;return *v;}
static void (*const generated[@FORMS@])(void);
static void (*const physical[@FORMS@])(const void*);
static const unsigned modes[@FORMS@],destinations[@FORMS@],sources[@FORMS@];
static FILE *corpus;
static uint32_t corpus_rows;
static const uint64_t edges[]={0,UINT64_MAX,UINT64_C(0x8000800080008000),UINT64_C(0x7fff7fff7fff7fff),UINT64_C(0x8000000080000000),UINT64_C(0x7fffffff7fffffff),UINT64_C(0x0123456789abcdef),UINT64_C(0xfedcba9876543210)};
static void import(const Image *image){GamePkgX87State *s=gamepkg_x86_x87_state();memset(s,0,sizeof(*s));s->control=(uint16_t)get16(image->b);s->status=(uint16_t)get16(image->b+2);s->instruction_pointer=0x40000;s->data_pointer=0x50000;s->opcode=0x123;unsigned top=(s->status>>11)&7;
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;s->significand[p]=get64(image->b+32+16*i);s->sign_exponent[p]=(uint16_t)get16(image->b+40+16*i);if(!(image->b[4]&(1u<<p)))s->tags|=(uint16_t)(3u<<(2*p));}
}
static void compare(const Image *after,unsigned form,unsigned sample){const GamePkgX87State *s=gamepkg_x86_x87_state();unsigned top=(s->status>>11)&7,valid=0;assert(destinations[form]<8);
    assert(s->control==get16(after->b)&&s->status==get16(after->b+2)&&s->instruction_pointer==0x40000&&s->data_pointer==0x50000&&s->opcode==0x123);
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;if(((s->tags>>(2*p))&3)!=3)valid|=1u<<p;
        if(s->significand[p]!=get64(after->b+32+16*i)||s->sign_exponent[p]!=get16(after->b+40+16*i)){fprintf(stderr,"shift form%u sample%u register%u got%016llx expected%016llx\n",form,sample,p,(unsigned long long)s->significand[p],(unsigned long long)get64(after->b+32+16*i));assert(0);}}
    assert(valid==after->b[4]);
}
static void prepare(Image *image,const Image *original,unsigned form,unsigned top,unsigned sample,uint64_t *rng){
    *image=*original;unsigned sw=(top<<11)|0x4700;image->b[0]=0x7f;image->b[1]=3;image->b[2]=(unsigned char)sw;image->b[3]=(unsigned char)(sw>>8);image->b[4]=(unsigned char)(sample%3==0?0:sample%3==1?0xff:0x5a);
    uint64_t values[8];for(unsigned i=0;i<8;++i)values[i]=sample<8?edges[(sample+i)&7]:random64(rng);
    uint64_t count=sample<256?sample:sample<264?UINT64_C(1)<<(sample-256+32):sample<272?(UINT64_C(1)<<(sample-264+56))|1:random64(rng);
    if(modes[form]<2){values[sources[form]]=count;}memory=count;
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;put64(image->b+32+16*i,values[p]);image->b[40+16*i]=(unsigned char)(sample+i);image->b[41+16*i]=(unsigned char)(0x3f+(i&1));}
    g_esi=1025;g_ebx=g_fs_base=0;g_eax=0x12345678;g_esp=0x800;g_eflags=0x8d7;
    if(modes[form]==3){g_fs_base=0xfffffff0;g_esi+=16;}if(modes[form]==4){g_ebx=0xffff;g_esi=0x10402;}
}
static void record(const Image *input,const Image *after,unsigned form){
    if(!corpus)return;
    unsigned top=(get16(input->b+2)>>11)&7,dst=destinations[form],slot=(dst+8-top)&7,which=form<320?form/40:(form-320)/256;
    static const unsigned widths[]={16,32,64,16,32,64,16,32};
    uint64_t count=modes[form]==5?(form-320)%256:modes[form]<2?get64(input->b+32+16*((sources[form]+8-top)&7)):memory;
    unsigned char row[32]={0};row[0]=(unsigned char)(which<3?0:which<6?1:2);row[4]=(unsigned char)widths[which];put64(row+8,get64(input->b+32+16*slot));put64(row+16,count);put64(row+24,get64(after->b+32+16*dst));assert(fwrite(row,1,sizeof(row),corpus)==sizeof(row));++corpus_rows;
}
static void one(const Image *original,unsigned form,unsigned top,unsigned sample,uint64_t *rng){Image input,after;prepare(&input,original,form,top,sample,rng);
    __asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);physical[form](&memory);__asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(*original): GAMEPKG_TEST_FX_CLOBBERS);import(&input);reads=completed=0;GamePkgX86Fault fault;
    assert(gamepkg_x86_run(generated[form],&fault)==0&&completed==1&&reads==(modes[form]>=2&&modes[form]<=4?2u:0u)&&g_eflags==0x8d7&&g_eax==0x12345678&&g_esp==0x800);compare(&after,form,sample);record(&input,&after,form);
}
#ifdef _WIN32
static DWORD WINAPI worker(void *raw){
#else
static void *worker(void *raw){
#endif
    unsigned tag=(unsigned)(uintptr_t)raw;GamePkgX87State *s=gamepkg_x86_x87_state();assert(s->control==0x37f&&s->tags==0xffff&&s->status==0);Image original;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef)+tag;
    for(unsigned i=0;i<1000;++i){one(&original,(i*17+tag)%@FORMS@,i&7,i%320,&rng);}return 0;
}
static void threads(void){
#ifdef _WIN32
    HANDLE h[8];for(unsigned i=0;i<8;++i){h[i]=CreateThread(0,0,worker,(void*)(uintptr_t)i,0,0);assert(h[i]);}assert(WaitForMultipleObjects(8,h,1,INFINITE)==WAIT_OBJECT_0);for(unsigned i=0;i<8;++i)CloseHandle(h[i]);
#else
    pthread_t h[8];for(unsigned i=0;i<8;++i)assert(pthread_create(&h[i],0,worker,(void*)(uintptr_t)i)==0);for(unsigned i=0;i<8;++i)assert(pthread_join(h[i],0)==0);
#endif
}
#ifndef _WIN32
static unsigned physical_checks;
static unsigned expected_vector;
static void handler(int signal,siginfo_t *info,void *raw){(void)info;ucontext_t *u=raw;_exit((unsigned)u->uc_mcontext.gregs[REG_TRAPNO]==expected_vector&&signal==(expected_vector==14?SIGSEGV:SIGFPE)?0:91);}
static void memory_priority(const Image *original){
    for(unsigned op=0;op<8;++op)for(unsigned pending=0;pending<2;++pending){pid_t child=fork();assert(child>=0);if(child==0){struct rlimit core={0,0};assert(setrlimit(RLIMIT_CORE,&core)==0);long page=sysconf(_SC_PAGESIZE);assert(page>=8);unsigned char *map=mmap(0,(size_t)page*2,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);assert(map!=MAP_FAILED&&mprotect(map+page,(size_t)page,PROT_NONE)==0);memset(map+page-4,0,4);Image input=*original;input.b[0]=pending?0x7e:0x7f;input.b[1]=3;input.b[2]=pending?1:0;input.b[3]=0x28;expected_vector=pending?16:14;struct sigaction action={0};action.sa_sigaction=handler;action.sa_flags=SA_SIGINFO;sigemptyset(&action.sa_mask);assert(sigaction(SIGSEGV,&action,0)==0&&sigaction(SIGFPE,&action,0)==0);__asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);physical[op*40+2](map+page-4);_exit(93);}int status;assert(waitpid(child,&status,0)==child&&WIFEXITED(status)&&WEXITSTATUS(status)==0);++physical_checks;}
}
#else
static unsigned physical_checks;
static void memory_priority(const Image *original){(void)original;}
#endif
int main(void){Image original;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef),checks=0;GamePkgX86Fault fault;
    const char *path=getenv("GAMEPKG_TAKEDAWN_MMX_SHIFT_CORPUS");if(path){corpus=fopen(path,"wb");assert(corpus);assert(fwrite("GPMSHF01\0\0\0\0",1,12,corpus)==12);}
    for(unsigned form=0;form<@FORMS@;++form)for(unsigned top=0;top<8;++top)for(unsigned sample=0;sample<(modes[form]==5?32u:320u);++sample){one(&original,form,top,sample,&rng);++checks;}
    if(corpus){assert(corpus_rows==checks&&fseek(corpus,8,SEEK_SET)==0);unsigned char count[8];put64(count,corpus_rows);assert(fwrite(count,1,4,corpus)==4&&fclose(corpus)==0);corpus=0;}
    for(unsigned form=0;form<@FORMS@;++form){Image input;prepare(&input,&original,form,3,0,&rng);import(&input);GamePkgX87State *s=gamepkg_x86_x87_state();s->control=0x37e;s->status|=1;GamePkgX87State old=*s;reads=completed=0;assert(gamepkg_x86_run(generated[form],&fault)==1&&fault.vector==16&&fault.eip==0x10000&&fault.ebp==0xabcdef12&&reads==0&&completed==0&&memcmp(&old,s,sizeof(old))==0);
        if(modes[form]>=2&&modes[form]<=4)for(unsigned failure=1;failure<=2;++failure){import(&input);old=*s;reads=completed=0;fail_read=failure;assert(gamepkg_x86_run(generated[form],&fault)==1&&fault.vector==13&&fault.eip==0xbad100&&fault.ebp==0xbad101&&reads==failure&&completed==0&&memcmp(&old,s,sizeof(old))==0);fail_read=0;}
    }
    GamePkgX87State parent=*gamepkg_x86_x87_state();threads();assert(memcmp(&parent,gamepkg_x86_x87_state(),sizeof(parent))==0);memory_priority(&original);printf("MMX shifts: %llu hardware/generated cases,2368 pending-fault forms,384 staged-read failures,8000 thread cases and%u physical memory-priority checks passed\n",(unsigned long long)checks,physical_checks);return 0;
}
