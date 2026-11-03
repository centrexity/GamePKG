/* Asset-free real-instruction oracle for mixed MMX/SSE conversion state. */
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
#include "gamepkg_x86_sse.h"
#include "gamepkg_x86_fault.h"
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <signal.h>
#include <ucontext.h>
#include <sys/wait.h>
#include <sys/resource.h>
#include <sys/mman.h>
#include <unistd.h>
#endif
_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
typedef struct{uint32_t u[4];}RecompXmm;
static _Thread_local RecompXmm xmm[8];
#define g_xmm0 xmm[0]
#define g_xmm1 xmm[1]
#define g_xmm2 xmm[2]
#define g_xmm3 xmm[3]
#define g_xmm4 xmm[4]
#define g_xmm5 xmm[5]
#define g_xmm6 xmm[6]
#define g_xmm7 xmm[7]
static _Thread_local unsigned reads,completed,fail_read;
static _Thread_local uint64_t memory;
uint32_t xbox_MmioLoad(uint32_t address,int width){assert(width==4&&(address==1025||address==1029));if(++reads==fail_read)gamepkg_x86_general_protection(0xbad100,0xbad101);return address==1025?(uint32_t)memory:(uint32_t)(memory>>32);}
typedef struct{_Alignas(16) unsigned char b[512];}Image;
static unsigned get16(const unsigned char*b){return b[0]|((unsigned)b[1]<<8);}
static uint32_t get32(const unsigned char*b){uint32_t v=0;for(unsigned i=0;i<4;++i)v|=(uint32_t)b[i]<<(8*i);return v;}
static uint64_t get64(const unsigned char*b){return get32(b)|((uint64_t)get32(b+4)<<32);}
static void put32(unsigned char*b,uint32_t v){for(unsigned i=0;i<4;++i)b[i]=(unsigned char)(v>>(8*i));}
static void put64(unsigned char*b,uint64_t v){put32(b,(uint32_t)v);put32(b+4,(uint32_t)(v>>32));}
static uint64_t random64(uint64_t*r){*r^=*r<<13;*r^=*r>>7;*r^=*r<<17;return *r;}
static void (*const generated[96])(void);
static void (*const physical[96])(const void*);
static const unsigned kinds[96],modes[96],destinations[96],sources[96];
static const uint32_t edges[]={0,1,0x80000000,0x80000001,0x7fffffff,0x00ffffff,0x01000001,0xfeffffff,0x007fffff,0x00800000,0x3effffff,0x3f000000,0x3f000001,0x3fc00000,0x40200000,0xbeffffff,0xbf000000,0xbf000001,0xbfc00000,0x4effffff,0x4f000000,0x4f000001,0xceffffff,0xcf000000,0xcf000001,0x7f800000,0xff800000,0x7f800001,0xff800001,0x7fc00001,0xffc00001,0xffffffff};
static void import(const Image*image){GamePkgX87State*s=gamepkg_x86_x87_state();memset(s,0,sizeof(*s));s->control=(uint16_t)get16(image->b);s->status=(uint16_t)get16(image->b+2);s->instruction_pointer=0x40000;s->data_pointer=0x50000;s->opcode=0x123;unsigned top=(s->status>>11)&7;
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;s->significand[p]=get64(image->b+32+16*i);s->sign_exponent[p]=(uint16_t)get16(image->b+40+16*i);if(!(image->b[4]&(1u<<p)))s->tags|=(uint16_t)(3u<<(2*p));for(unsigned lane=0;lane<4;++lane)xmm[i].u[lane]=get32(image->b+160+16*i+4*lane);}
}
static void compare(const Image*image,unsigned form,unsigned vector){const GamePkgX87State*s=gamepkg_x86_x87_state();unsigned top=(s->status>>11)&7,valid=0;
    if(s->control!=get16(image->b)||s->status!=get16(image->b+2)){fprintf(stderr,"mixed form%u vector%u CW/SW got%04x/%04x expected%04x/%04x\n",form,vector,s->control,s->status,get16(image->b),get16(image->b+2));assert(0);}
    assert(s->instruction_pointer==0x40000&&s->data_pointer==0x50000&&s->opcode==0x123&&gamepkg_x86_get_mxcsr()==get32(image->b+24));
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;if(((s->tags>>(2*p))&3)!=3)valid|=1u<<p;
        assert(s->significand[p]==get64(image->b+32+16*i)&&s->sign_exponent[p]==get16(image->b+40+16*i));for(unsigned lane=0;lane<4;++lane)if(xmm[i].u[lane]!=get32(image->b+160+16*i+4*lane)){fprintf(stderr,"mixed form%u vector%u xmm%u lane%u got%08x expected%08x\n",form,vector,i,lane,xmm[i].u[lane],get32(image->b+160+16*i+4*lane));assert(0);}}
    assert(valid==image->b[4]);
}
static void prepare(Image*input,const Image*original,unsigned form,unsigned top,unsigned sample,uint32_t csr,uint64_t value,uint64_t*rng){*input=*original;input->b[0]=0x7f;input->b[1]=3;unsigned sw=(top<<11)|0x4700;input->b[2]=(unsigned char)sw;input->b[3]=(unsigned char)(sw>>8);input->b[4]=(unsigned char)(sample%3==0?0:sample%3==1?255:0x5a);put32(input->b+24,csr);
    for(unsigned i=0;i<8;++i){put64(input->b+32+16*i,random64(rng));input->b[40+16*i]=(unsigned char)(sample+i);input->b[41+16*i]=(unsigned char)(0x3f+(i&1));for(unsigned lane=0;lane<4;++lane)put32(input->b+160+16*i+4*lane,(uint32_t)random64(rng));}
    if(!modes[form]){if(!kinds[form])put64(input->b+32+16*((sources[form]+8-top)&7),value);else put64(input->b+160+16*sources[form],value);}memory=value;
}
static void registers(unsigned form){g_eax=0x12345678;g_esp=0x800;g_esi=1025;g_ebx=g_fs_base=0;g_eflags=0x8d7;g_ebp=0x777;if(modes[form]==2){g_fs_base=0xfffffff0;g_esi+=16;}if(modes[form]==3){g_ebx=0xffff;g_esi=0x10402;}}
static void generated_check(const Image*before,const Image*expected,unsigned form,uint32_t csr,unsigned vector){import(before);registers(form);gamepkg_x86_set_mxcsr(csr,0,0);reads=completed=0;GamePkgX86Fault fault;int error=gamepkg_x86_run(generated[form],&fault);
    assert(error==(vector!=0)&&completed==(vector?0u:1u)&&reads==(modes[form]&&vector!=16?2u:0u)&&g_eax==0x12345678&&g_esp==0x800&&g_eflags==0x8d7);if(vector)assert(fault.vector==vector&&fault.eip==0x10000&&fault.ebp==0xabcdef12&&fault.mxcsr==get32(expected->b+24));compare(expected,form,vector);
}
static FILE*corpus;static uint32_t corpus_rows;
static void record(unsigned op,uint32_t csr,uint64_t value,uint64_t result,uint32_t flags){if(!corpus)return;unsigned char row[32]={0};row[0]=(unsigned char)op;row[4]=(unsigned char)((csr>>13)&3);put64(row+8,value);put64(row+16,result);put32(row+24,flags);assert(fwrite(row,1,32,corpus)==32);++corpus_rows;}
static void one(const Image*original,unsigned form,unsigned top,unsigned sample,uint32_t csr,uint64_t value,uint64_t*rng){Image requested,before,after,entered;prepare(&requested,original,form,top,sample,(csr&~63u)|0x1f80,value,rng);
    __asm__ volatile("fxrstor %0"::"m"(requested): GAMEPKG_TEST_FX_CLOBBERS);__asm__ volatile("fxsave %0":"=m"(before)::"memory");physical[form](&memory);__asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(before): GAMEPKG_TEST_FX_CLOBBERS);if(kinds[form]||!modes[form])__asm__ volatile("movd %%mm0,%%eax":::"eax","memory");__asm__ volatile("fxsave %0":"=m"(entered)::"memory");__asm__ volatile("fxrstor %0"::"m"(*original): GAMEPKG_TEST_FX_CLOBBERS);
    unsigned flags=get32(after.b+24)&63,vector=0;uint64_t result=kinds[form]?get64(after.b+32+16*destinations[form]):get64(after.b+160+16*destinations[form]);record(kinds[form],csr,value,result,flags);
    if((flags&1)&&!(csr&0x80)){flags=1;vector=19;}else if((flags&32)&&!(csr&0x1000))vector=19;
    Image expected=vector?entered:after;put32(expected.b+24,csr|flags);generated_check(&before,&expected,form,csr,vector);
}
static void pending_cases(const Image*original,uint64_t*rng){
 for(unsigned form=0;form<96;++form){Image requested,before,after;prepare(&requested,original,form,5,1,0x1f80,UINT64_C(0x7fffffff01000001),rng);requested.b[0]=0x7e;requested.b[2]|=0x81;requested.b[3]|=0x80;
  if(!kinds[form]&&modes[form]){__asm__ volatile("fxrstor %0"::"m"(requested): GAMEPKG_TEST_FX_CLOBBERS);__asm__ volatile("fxsave %0":"=m"(before)::"memory");physical[form](&memory);__asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(*original): GAMEPKG_TEST_FX_CLOBBERS);generated_check(&before,&after,form,0x1f80,0);}
  else{before=requested;after=requested;generated_check(&before,&after,form,0x1f80,16);}
 }
}
#ifndef _WIN32
typedef struct{unsigned vector;Image before,after;}Report;
static int channel;static Report captured;
static void handler(int signal,siginfo_t*info,void*raw){(void)signal;(void)info;ucontext_t*u=raw;captured.vector=(unsigned)u->uc_mcontext.gregs[REG_TRAPNO];const unsigned char*fp=(const unsigned char*)u->uc_mcontext.fpregs;
#ifdef __x86_64__
    memcpy(captured.after.b,fp,512);
#else
    if(get16(fp+110)!=0){_exit(91);}memcpy(captured.after.b,fp+112,512);captured.after.b[0]=fp[0];captured.after.b[1]=fp[1];captured.after.b[2]=fp[4];captured.after.b[3]=fp[5];captured.after.b[4]=0;unsigned tags=get32(fp+8);for(unsigned p=0;p<8;++p)if(((tags>>(2*p))&3)!=3)captured.after.b[4]|=(unsigned char)(1u<<p);for(unsigned i=0;i<8;++i)memcpy(captured.after.b+32+16*i,fp+28+10*i,10);
#endif
    if(write(channel,&captured,sizeof(captured))!=(ssize_t)sizeof(captured)){_exit(92);}_exit(0);
}
static unsigned physical_checks,physical_memory_checks;
static void faults(const Image*original,uint64_t*rng){
    for(unsigned op=0;op<3;++op)for(unsigned mode=0;mode<2;++mode)for(unsigned pending=0;pending<2;++pending)for(unsigned masks=0;masks<4;++masks)for(unsigned rc=0;rc<4;++rc)for(unsigned ftz=0;ftz<2;++ftz)for(unsigned sample=0;sample<4;++sample)for(unsigned topindex=0;topindex<2;++topindex){unsigned form=op*32+mode,top=topindex?7:0;uint32_t csr=0x1f80|(rc<<13)|(ftz<<15);if(!(masks&1))csr&=~0x80u;if(!(masks&2))csr&=~0x1000u;static const uint64_t integers[]={UINT64_C(0x0100000100000001),UINT64_C(0x7fffffff80000000),UINT64_C(0x0000000001000001),0};static const uint64_t floats[]={UINT64_C(0x3fc000007fc00001),UINT64_C(0x7fc000013fc00000),UINT64_C(0x0000000000000001),UINT64_C(0x3f8000007f800000)};uint64_t value=op?floats[sample]:integers[sample];Image requested;prepare(&requested,original,form,top,sample,csr,value,rng);if(pending){requested.b[0]=0x7e;requested.b[2]|=0x81;requested.b[3]|=0x80;}
        int fd[2];assert(pipe(fd)==0);pid_t child=fork();assert(child>=0);if(child==0){close(fd[0]);channel=fd[1];struct rlimit core={0,0};assert(setrlimit(RLIMIT_CORE,&core)==0);struct sigaction action={0};action.sa_sigaction=handler;action.sa_flags=SA_SIGINFO;sigemptyset(&action.sa_mask);assert(sigaction(SIGFPE,&action,0)==0);captured.vector=0;__asm__ volatile("fxrstor %0"::"m"(requested): GAMEPKG_TEST_FX_CLOBBERS);__asm__ volatile("fxsave %0":"=m"(captured.before)::"memory");physical[form](&memory);__asm__ volatile("fxsave %0":"=m"(captured.after)::"memory");__asm__ volatile("fxrstor %0"::"m"(*original): GAMEPKG_TEST_FX_CLOBBERS);if(write(fd[1],&captured,sizeof(captured))!=(ssize_t)sizeof(captured))_exit(93);_exit(0);}
        close(fd[1]);Report result;assert(read(fd[0],&result,sizeof(result))==(ssize_t)sizeof(result));close(fd[0]);int status;assert(waitpid(child,&status,0)==child&&WIFEXITED(status)&&WEXITSTATUS(status)==0);generated_check(&result.before,&result.after,form,csr,result.vector);++physical_checks;
    }
}
static void memory_faults(const Image*original,uint64_t*rng){
 for(unsigned op=0;op<3;++op)for(unsigned pending=0;pending<2;++pending){unsigned form=op*32+1;Image requested;prepare(&requested,original,form,5,0,0,UINT64_MAX,rng);if(pending){requested.b[0]=0x7e;requested.b[2]|=0x81;requested.b[3]|=0x80;}int fd[2];assert(pipe(fd)==0);pid_t child=fork();assert(child>=0);
  if(child==0){close(fd[0]);channel=fd[1];struct rlimit core={0,0};assert(setrlimit(RLIMIT_CORE,&core)==0);long page=sysconf(_SC_PAGESIZE);assert(page>=8);unsigned char*map=mmap(0,(size_t)page*2,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);assert(map!=MAP_FAILED&&mprotect(map+page,(size_t)page,PROT_NONE)==0);memset(map+page-4,0xff,4);struct sigaction action={0};action.sa_sigaction=handler;action.sa_flags=SA_SIGINFO;sigemptyset(&action.sa_mask);assert(sigaction(SIGFPE,&action,0)==0&&sigaction(SIGSEGV,&action,0)==0);captured.vector=0;__asm__ volatile("fxrstor %0"::"m"(requested): GAMEPKG_TEST_FX_CLOBBERS);__asm__ volatile("fxsave %0":"=m"(captured.before)::"memory");physical[form](map+page-4);_exit(93);}
  close(fd[1]);Report result;assert(read(fd[0],&result,sizeof(result))==(ssize_t)sizeof(result));close(fd[0]);int status;assert(waitpid(child,&status,0)==child&&WIFEXITED(status)&&WEXITSTATUS(status)==0);unsigned vector=pending&&op?16:14;assert(result.vector==vector);import(&result.before);registers(form);gamepkg_x86_set_mxcsr(0,0,0);reads=completed=0;fail_read=2;GamePkgX86Fault fault;assert(gamepkg_x86_run(generated[form],&fault)==1&&fault.vector==(vector==14?13u:16u)&&fault.eip==(vector==14?0xbad100u:0x10000u)&&reads==(vector==14?2u:0u)&&completed==0);fail_read=0;compare(&result.after,form,vector);++physical_memory_checks;
 }
}


#else
static unsigned physical_checks,physical_memory_checks;
static void faults(const Image*original,uint64_t*rng){(void)original;(void)rng;}
static void memory_faults(const Image*original,uint64_t*rng){(void)original;(void)rng;}
#endif
#ifdef _WIN32
static DWORD WINAPI worker(void*raw){
#else
static void*worker(void*raw){
#endif
    unsigned tag=(unsigned)(uintptr_t)raw;assert(gamepkg_x86_x87_state()->tags==0xffff&&gamepkg_x86_get_mxcsr()==0x1f80);Image original;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef)+tag;for(unsigned i=0;i<1000;++i){one(&original,(i+tag)%96,i&7,i,0x1f80|((i&3)<<13),random64(&rng),&rng);}return 0;
}
static void threads(void){
#ifdef _WIN32
    HANDLE h[8];for(unsigned i=0;i<8;++i){h[i]=CreateThread(0,0,worker,(void*)(uintptr_t)i,0,0);assert(h[i]);}assert(WaitForMultipleObjects(8,h,1,INFINITE)==WAIT_OBJECT_0);for(unsigned i=0;i<8;++i)CloseHandle(h[i]);
#else
    pthread_t h[8];for(unsigned i=0;i<8;++i)assert(pthread_create(&h[i],0,worker,(void*)(uintptr_t)i)==0);for(unsigned i=0;i<8;++i)assert(pthread_join(h[i],0)==0);
#endif
}
int main(void){Image original;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef),checks=0;const char*path=getenv("GAMEPKG_TAKEDAWN_MMX_CONVERSION_CORPUS");if(path){corpus=fopen(path,"wb");assert(corpus&&fwrite("GPMCVI01\0\0\0\0",1,12,corpus)==12);}
    for(unsigned form=0;form<96;++form)for(unsigned rc=0;rc<4;++rc)for(unsigned ftz=0;ftz<2;++ftz)for(unsigned top=0;top<8;++top)for(unsigned sample=0;sample<64;++sample){uint64_t value=sample<32?(uint64_t)edges[sample]|((uint64_t)edges[(sample+11)&31]<<32):random64(&rng);one(&original,form,top,sample,0x1f80|(rc<<13)|(ftz<<15)|((sample&1)?63u:0),value,&rng);++checks;}
    for(unsigned op=0;op<3;++op)for(unsigned masks=0;masks<64;++masks)for(unsigned rc=0;rc<4;++rc)for(unsigned sample=0;sample<32;++sample){unsigned form=op*32+sample;uint64_t value=(uint64_t)edges[sample]|((uint64_t)edges[(sample+13)&31]<<32);one(&original,form,sample&7,sample,(masks<<7)|(rc<<13),value,&rng);++checks;}
    if(corpus){assert(corpus_rows==checks&&fseek(corpus,8,SEEK_SET)==0);unsigned char count[8];put64(count,corpus_rows);assert(fwrite(count,1,4,corpus)==4&&fclose(corpus)==0);corpus=0;}
    unsigned failures=0;for(unsigned form=0;form<96;++form)if(modes[form])for(unsigned fail=1;fail<=2;++fail){Image input;prepare(&input,&original,form,3,0,0x1f80,UINT64_MAX,&rng);import(&input);registers(form);gamepkg_x86_set_mxcsr(0x1f80,0,0);GamePkgX87State before=*gamepkg_x86_x87_state();RecompXmm before_xmm[8];memcpy(before_xmm,xmm,sizeof(xmm));reads=completed=0;fail_read=fail;GamePkgX86Fault fault;assert(gamepkg_x86_run(generated[form],&fault)==1&&fault.vector==13&&fault.eip==0xbad100&&reads==fail&&completed==0&&gamepkg_x86_get_mxcsr()==0x1f80&&memcmp(&before,gamepkg_x86_x87_state(),sizeof(before))==0&&memcmp(before_xmm,xmm,sizeof(xmm))==0);fail_read=0;++failures;}
    GamePkgX87State parent=*gamepkg_x86_x87_state();uint32_t parent_csr=gamepkg_x86_get_mxcsr();threads();assert(memcmp(&parent,gamepkg_x86_x87_state(),sizeof(parent))==0&&gamepkg_x86_get_mxcsr()==parent_csr);pending_cases(&original,&rng);faults(&original,&rng);memory_faults(&original,&rng);printf("mixed conversion: %llu hardware/generated cases,%u staged-read failures,8000 thread cases,%u physical fault/state cases,%u memory-priority cases passed\n",(unsigned long long)checks,failures,physical_checks,physical_memory_checks);return 0;
}
