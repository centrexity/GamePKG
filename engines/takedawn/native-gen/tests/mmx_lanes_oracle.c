/* Synthetic SSE-era MMX oracle; no game bytes or host floating point. */
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
#include <stdlib.h>
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
static _Thread_local unsigned reads,completed,fail_read;
static _Thread_local uint64_t memory;
static _Thread_local uint32_t observed_ebp;
uint32_t xbox_MmioLoad(uint32_t address,int width){assert((width==2||width==4)&&(address==1025||address==1029));if(++reads==fail_read)gamepkg_x86_general_protection(0xbad100,0xbad101);uint32_t v=address==1025?(uint32_t)memory:(uint32_t)(memory>>32);return width==2?v&65535:v;}
typedef struct {_Alignas(16) unsigned char b[512];}Image;
static unsigned get16(const unsigned char *b){return b[0]|((unsigned)b[1]<<8);}
static uint64_t get64(const unsigned char *b){uint64_t v=0;for(unsigned i=0;i<8;++i)v|=(uint64_t)b[i]<<(8*i);return v;}
static void put64(unsigned char *b,uint64_t v){for(unsigned i=0;i<8;++i)b[i]=(unsigned char)(v>>(8*i));}
static uint64_t random64(uint64_t *v){*v^=*v<<13;*v^=*v>>7;*v^=*v<<17;return *v;}
static void (*const generated[@FORMS@])(void);
static uint32_t (*const physical[@FORMS@])(const void*,uint32_t);
static const unsigned kinds[@FORMS@],modes[@FORMS@],destinations[@FORMS@],sources[@FORMS@],controls[@FORMS@];
static const uint64_t edges[]={0,UINT64_MAX,UINT64_C(0x8000800080008000),UINT64_C(0x7fff7fff7fff7fff),UINT64_C(0x8000000080000000),UINT64_C(0x7fffffff7fffffff),UINT64_C(0x007f008000ff0100),UINT64_C(0xff7fff80ff00feff)};
static uint32_t gpr(unsigned reg){switch(reg){case 0:return g_eax;case 1:return g_ecx;case 2:return g_edx;case 3:return g_ebx;case 4:return g_esp;case 5:return observed_ebp;case 6:return g_esi;default:assert(reg==7);return g_edi;}}
static uint64_t mm(const Image *image,unsigned reg){unsigned top=(get16(image->b+2)>>11)&7;return get64(image->b+32+16*((reg+8-top)&7));}
static void import(const Image *image){GamePkgX87State *s=gamepkg_x86_x87_state();memset(s,0,sizeof(*s));s->control=(uint16_t)get16(image->b);s->status=(uint16_t)get16(image->b+2);s->instruction_pointer=0x40000;s->data_pointer=0x50000;s->opcode=0x123;unsigned top=(s->status>>11)&7;
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;s->significand[p]=get64(image->b+32+16*i);s->sign_exponent[p]=(uint16_t)get16(image->b+40+16*i);if(!(image->b[4]&(1u<<p)))s->tags|=(uint16_t)(3u<<(2*p));}
}
static void compare(const Image *after,unsigned form){const GamePkgX87State *s=gamepkg_x86_x87_state();unsigned top=(s->status>>11)&7,valid=0;
    assert(s->control==get16(after->b)&&s->status==get16(after->b+2)&&s->instruction_pointer==0x40000&&s->data_pointer==0x50000&&s->opcode==0x123);
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;if(((s->tags>>(2*p))&3)!=3)valid|=1u<<p;
        if(s->significand[p]!=get64(after->b+32+16*i)||s->sign_exponent[p]!=get16(after->b+40+16*i)){fprintf(stderr,"lane form%u op%u reg%u got%016llx expected%016llx\n",form,kinds[form],p,(unsigned long long)s->significand[p],(unsigned long long)get64(after->b+32+16*i));assert(0);}}
    assert(valid==after->b[4]);
}
static void prepare(Image *input,const Image *original,unsigned form,unsigned top,unsigned sample,uint64_t *rng,int custom,uint64_t a,uint64_t b){
    *input=*original;unsigned sw=(top<<11)|0x4700;input->b[0]=0x7f;input->b[1]=3;input->b[2]=(unsigned char)sw;input->b[3]=(unsigned char)(sw>>8);input->b[4]=(unsigned char)(sample%3==0?0:sample%3==1?255:0x5a);uint64_t values[8];for(unsigned i=0;i<8;++i)values[i]=sample<8?edges[(sample+i)&7]:random64(rng);memory=sample<8?edges[(sample+3)&7]:random64(rng);
    if(custom){values[destinations[form]]=a;values[sources[form]]=b;memory=b;}
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;put64(input->b+32+16*i,values[p]);input->b[40+16*i]=(unsigned char)(sample+i);input->b[41+16*i]=(unsigned char)(0x3f+(i&1));}
    g_eax=0x12348001;g_ecx=0x9876ffff;g_edx=0x76540000;g_ebx=0x54327fff;g_esp=0x32108000;g_ebp=0x55555555;observed_ebp=0xabcdef12;g_esi=1025;g_edi=0xfedc007f;g_fs_base=0;g_eflags=0x8d7;
}
static FILE *corpus;static uint32_t corpus_rows;
static void record(const Image *input,const Image *after,unsigned form,uint32_t in,uint32_t out){if(!corpus)return;unsigned op=kinds[form],src=sources[form],dst=destinations[form];uint64_t a,b=0,expected;
    if(op<7){a=mm(input,dst);b=modes[form]==2?memory:mm(input,src);}else if(op==7){a=modes[form]==2?memory:mm(input,src);b=a;}else if(op==8){a=mm(input,dst);b=modes[form]==2?memory&65535:in;}else a=mm(input,src);
    expected=op>=9?out:mm(after,dst);unsigned char row[32]={0};row[0]=(unsigned char)op;row[4]=(unsigned char)controls[form];put64(row+8,a);put64(row+16,b);put64(row+24,expected);assert(fwrite(row,1,sizeof(row),corpus)==sizeof(row));++corpus_rows;
}
static void one(const Image *original,unsigned form,unsigned top,unsigned sample,uint64_t *rng,int custom,uint64_t a,uint64_t b){Image input,after;prepare(&input,original,form,top,sample,rng,custom,a,b);uint32_t old[8];for(unsigned i=0;i<8;++i)old[i]=gpr(i);uint32_t in=old[sources[form]],out;
    __asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);out=physical[form](&memory,in);__asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(*original): GAMEPKG_TEST_FX_CLOBBERS);import(&input);reads=completed=0;GamePkgX86Fault fault;assert(gamepkg_x86_run(generated[form],&fault)==0&&completed==1&&reads==(modes[form]==2?(kinds[form]==8?1u:2u):0u)&&g_eflags==0x8d7&&g_ebp==0x55555555);compare(&after,form);
    for(unsigned i=0;i<8;++i){assert(gpr(i)==(kinds[form]>=9&&i==destinations[form]?out:old[i]));}record(&input,&after,form,in,out);
}
#ifdef _WIN32
static DWORD WINAPI worker(void *raw){
#else
static void *worker(void *raw){
#endif
    unsigned tag=(unsigned)(uintptr_t)raw;GamePkgX87State *s=gamepkg_x86_x87_state();assert(s->control==0x37f&&s->tags==0xffff&&s->status==0);Image original;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef)+tag;for(unsigned i=0;i<1000;++i){one(&original,(i*17+tag)%@FORMS@,i&7,i%16,&rng,0,0,0);}return 0;
}
static void threads(void){
#ifdef _WIN32
    HANDLE h[8];for(unsigned i=0;i<8;++i){h[i]=CreateThread(0,0,worker,(void*)(uintptr_t)i,0,0);assert(h[i]);}assert(WaitForMultipleObjects(8,h,1,INFINITE)==WAIT_OBJECT_0);for(unsigned i=0;i<8;++i)CloseHandle(h[i]);
#else
    pthread_t h[8];for(unsigned i=0;i<8;++i)assert(pthread_create(&h[i],0,worker,(void*)(uintptr_t)i)==0);for(unsigned i=0;i<8;++i)assert(pthread_join(h[i],0)==0);
#endif
}
#ifndef _WIN32
static unsigned physical_checks,expected_vector;
static void handler(int signal,siginfo_t *info,void *raw){(void)info;ucontext_t *u=raw;_exit((unsigned)u->uc_mcontext.gregs[REG_TRAPNO]==expected_vector&&signal==(expected_vector==14?SIGSEGV:SIGFPE)?0:91);}
static void memory_priority(const Image *original){
    const unsigned forms[]={2,168+2,168+768+1};
    for(unsigned index=0;index<3;++index)for(unsigned pending=0;pending<2;++pending){unsigned form=forms[index];pid_t child=fork();assert(child>=0);if(child==0){struct rlimit core={0,0};assert(setrlimit(RLIMIT_CORE,&core)==0);long page=sysconf(_SC_PAGESIZE);assert(page>=8);unsigned char *map=mmap(0,(size_t)page*2,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);assert(map!=MAP_FAILED&&mprotect(map+page,(size_t)page,PROT_NONE)==0);unsigned bytes=kinds[form]==8?2:4;memset(map+page-bytes,0xff,bytes);Image input=*original;input.b[0]=pending?0x7e:0x7f;input.b[1]=3;input.b[2]=pending?1:0;input.b[3]=0x28;expected_vector=pending?16:kinds[form]==8?0:14;struct sigaction action={0};action.sa_sigaction=handler;action.sa_flags=SA_SIGINFO;sigemptyset(&action.sa_mask);assert(sigaction(SIGSEGV,&action,0)==0&&sigaction(SIGFPE,&action,0)==0);__asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);(void)physical[form](map+page-bytes,0);_exit(!pending&&kinds[form]==8?0:93);}int status;assert(waitpid(child,&status,0)==child&&WIFEXITED(status)&&WEXITSTATUS(status)==0);++physical_checks;}
}
#else
static unsigned physical_checks;
static void memory_priority(const Image *original){(void)original;}
#endif
int main(void){Image original;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef),checks=0;GamePkgX86Fault fault;const char *path=getenv("GAMEPKG_TAKEDAWN_MMX_LANES_CORPUS");if(path){corpus=fopen(path,"wb");assert(corpus&&fwrite("GPMLAN01\0\0\0\0",1,12,corpus)==12);}
    for(unsigned form=0;form<@FORMS@;++form)for(unsigned top=0;top<8;++top)for(unsigned sample=0;sample<16;++sample){one(&original,form,top,sample,&rng,0,0,0);++checks;}
    const unsigned byte_ops[]={0,2,3,6},word_ops[]={1,4,5};
    for(unsigned o=0;o<4;++o)for(unsigned x=0;x<256;++x)for(unsigned y=0;y<256;++y)for(unsigned mode=0;mode<3;mode+=2){one(&original,byte_ops[o]*24+mode,0,0,&rng,1,UINT64_C(0x0101010101010101)*x,UINT64_C(0x0101010101010101)*y);++checks;}
    for(unsigned o=0;o<3;++o)for(unsigned x=0;x<65536;++x)for(unsigned pair=0;pair<2;++pair)for(unsigned mode=0;mode<3;mode+=2){one(&original,word_ops[o]*24+mode,0,0,&rng,1,UINT64_C(0x0001000100010001)*x,UINT64_C(0x0001000100010001)*(pair?x^65535:32768));++checks;}
    if(corpus){assert(corpus_rows==checks&&fseek(corpus,8,SEEK_SET)==0);unsigned char count[8];put64(count,corpus_rows);assert(fwrite(count,1,4,corpus)==4&&fclose(corpus)==0);corpus=0;}
    unsigned failures=0;
    for(unsigned form=0;form<@FORMS@;++form){Image input;prepare(&input,&original,form,3,0,&rng,0,0,0);import(&input);GamePkgX87State *s=gamepkg_x86_x87_state();s->control=0x37e;s->status|=1;GamePkgX87State old=*s;reads=completed=0;assert(gamepkg_x86_run(generated[form],&fault)==1&&fault.vector==16&&fault.eip==0x10000&&fault.ebp==0xabcdef12&&reads==0&&completed==0&&memcmp(&old,s,sizeof(old))==0);
        if(modes[form]==2)for(unsigned failure=1;failure<=(kinds[form]==8?1u:2u);++failure){import(&input);old=*s;reads=completed=0;fail_read=failure;assert(gamepkg_x86_run(generated[form],&fault)==1&&fault.vector==13&&fault.eip==0xbad100&&fault.ebp==0xbad101&&reads==failure&&completed==0&&memcmp(&old,s,sizeof(old))==0);fail_read=0;++failures;}
    }
    GamePkgX87State parent=*gamepkg_x86_x87_state();threads();assert(memcmp(&parent,gamepkg_x86_x87_state(),sizeof(parent))==0);memory_priority(&original);printf("MMX lanes: %llu hardware/generated cases,%u pending forms,%u read failures,8000 thread cases,%u physical memory-priority checks passed\n",(unsigned long long)checks,@FORMS@,failures,physical_checks);return 0;
}
