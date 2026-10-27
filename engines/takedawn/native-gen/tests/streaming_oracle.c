/* Synthetic streaming/cache instruction oracle with a concrete checked RAM
 * backend. Actual CPU state/bytes are compared, not a second copy of lowering. */
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
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_x87.h"
#ifdef _WIN32
#include <windows.h>
#else
#include <signal.h>
#include <ucontext.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <sys/resource.h>
#include <unistd.h>
#endif
_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
typedef struct{uint32_t u[4];}RecompXmm;
static RecompXmm xmm[8];
#define g_xmm0 xmm[0]
#define g_xmm1 xmm[1]
#define g_xmm2 xmm[2]
#define g_xmm3 xmm[3]
#define g_xmm4 xmm[4]
#define g_xmm5 xmm[5]
#define g_xmm6 xmm[6]
#define g_xmm7 xmm[7]
static uint32_t local_ebp;static unsigned completed;
typedef struct{_Alignas(16)unsigned char b[512];}Image;
static void (*const generated[@FORMS@])(void);
static void (*const physical[@FORMS@])(unsigned char*);
static const unsigned kinds[@FORMS@],modes[@FORMS@],sources[@FORMS@],masks[@FORMS@];
static unsigned get16(const unsigned char*b){return b[0]|((unsigned)b[1]<<8);}
static uint32_t get32(const unsigned char*b){uint32_t v=0;for(unsigned i=0;i<4;++i)v|=(uint32_t)b[i]<<(8*i);return v;}
static uint64_t get64(const unsigned char*b){return get32(b)|((uint64_t)get32(b+4)<<32);}
static void put32(unsigned char*b,uint32_t v){for(unsigned i=0;i<4;++i)b[i]=(unsigned char)(v>>(8*i));}
static void put64(unsigned char*b,uint64_t v){put32(b,(uint32_t)v);put32(b+4,(uint32_t)(v>>32));}
static uint64_t random64(uint64_t*r){*r^=*r<<13;*r^=*r>>7;*r^=*r<<17;return *r;}
static void import(const Image*image){GamePkgX87State*s=gamepkg_x86_x87_state();memset(s,0,sizeof(*s));s->control=(uint16_t)get16(image->b);s->status=(uint16_t)get16(image->b+2);s->instruction_pointer=0x40000;s->data_pointer=0x50000;s->opcode=0x123;unsigned top=(s->status>>11)&7;for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;s->significand[p]=get64(image->b+32+16*i);s->sign_exponent[p]=(uint16_t)get16(image->b+40+16*i);if(!(image->b[4]&(1u<<p)))s->tags|=(uint16_t)(3u<<(2*p));for(unsigned lane=0;lane<4;++lane)xmm[i].u[lane]=get32(image->b+160+16*i+4*lane);}}
static void compare(const Image*after,unsigned form){const GamePkgX87State*s=gamepkg_x86_x87_state();unsigned top=(s->status>>11)&7,valid=0;int state_matches=s->control==get16(after->b)&&s->status==get16(after->b+2)&&s->instruction_pointer==0x40000&&s->data_pointer==0x50000&&s->opcode==0x123&&gamepkg_x86_get_mxcsr()==get32(after->b+24);if(!state_matches)fprintf(stderr,"stream form%u kind%u mode%u source%u mask%u: control %04x/%04x status %04x/%04x ip %08x/00040000 data %08x/00050000 opcode %04x/0123 mxcsr %08x/%08x (generated/hardware)\n",form,kinds[form],modes[form],sources[form],masks[form],s->control,get16(after->b),s->status,get16(after->b+2),s->instruction_pointer,s->data_pointer,s->opcode,gamepkg_x86_get_mxcsr(),get32(after->b+24));assert(state_matches);for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;if(((s->tags>>(2*p))&3)!=3)valid|=1u<<p;if(s->significand[p]!=get64(after->b+32+16*i)||s->sign_exponent[p]!=get16(after->b+40+16*i)){fprintf(stderr,"stream form%u kind%u reg%u changed\n",form,kinds[form],p);assert(0);}for(unsigned lane=0;lane<4;++lane)assert(xmm[i].u[lane]==get32(after->b+160+16*i+4*lane));}assert(valid==after->b[4]);}
static void prepare(Image*input,const Image*original,unsigned form,unsigned top,unsigned sample,uint64_t*rng){assert(sources[form]<8);*input=*original;input->b[0]=0x7f;input->b[1]=3;unsigned sw=(top<<11)|0x4700;input->b[2]=(unsigned char)sw;input->b[3]=(unsigned char)(sw>>8);input->b[4]=(unsigned char)(sample%3==0?0:sample%3==1?255:0x5a);put32(input->b+24,((sample&63)<<7)|((top&3)<<13)|((sample&1)<<15)|(sample&63));for(unsigned i=0;i<8;++i){put64(input->b+32+16*i,random64(rng));input->b[40+16*i]=(unsigned char)(i+sample);input->b[41+16*i]=(unsigned char)(0x3f+(i&1));for(unsigned lane=0;lane<4;++lane)put32(input->b+160+16*i+4*lane,(uint32_t)random64(rng));}if(kinds[form]==2){uint64_t mask=random64(rng)&UINT64_C(0x7f7f7f7f7f7f7f7f);for(unsigned i=0;i<8;++i)if(sample&(1u<<i))mask|=UINT64_C(128)<<(8*i);put64(input->b+32+16*((masks[form]+8-top)&7),mask);}}
static void registers(unsigned form,uint32_t address){g_eax=0x12345678;g_ecx=0x9876;g_edx=0x7654;g_esp=0x800;g_ebp=0x777;g_ebx=0;g_esi=address;g_edi=address;g_fs_base=0;local_ebp=0xabcdef12;g_eflags=0x8d7;
 if(kinds[form]==2){if(modes[form]==1){g_fs_base=0xfffffff0;g_edi+=16;}if(modes[form]==2)g_edi|=0x10000;}
 else{if(modes[form]==1){g_fs_base=0xfffffff0;g_esi+=16;}if(modes[form]==2){g_ebx=0xffff;g_esi=0x10000|((address+1)&65535);}if(modes[form]==3)local_ebp=address;if(modes[form]==5){g_edi=0x20000001;g_esi-=40;}}
}
static uint64_t checks;
static void one(const Image*original,unsigned form,unsigned top,unsigned sample,uint64_t*rng){Image input,before,after;prepare(&input,original,form,top,sample,rng);unsigned offset=kinds[form]==1?16*(sample&1):sample&15;if(kinds[form]!=2&&modes[form]==4)offset=0;_Alignas(16)unsigned char hardware[80],actual[80];for(unsigned i=0;i<80;++i)hardware[i]=actual[i]=(unsigned char)(i^sample);
 __asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);__asm__ volatile("fxsave %0":"=m"(before)::"memory");physical[form](hardware+offset);__asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(*original): GAMEPKG_TEST_FX_CLOBBERS);GamePkgX86Region r={1024,sizeof(actual),3,actual};GamePkgX86Memory*m=gamepkg_x86_memory_create(&r,1,1);assert(m);gamepkg_x86_memory_bind(m);import(&before);registers(form,1024+offset);gamepkg_x86_set_mxcsr(get32(before.b+24),0,0);completed=0;GamePkgX86Fault f;assert(gamepkg_x86_run(generated[form],&f)==0&&completed==1&&g_eax==0x12345678&&g_ecx==0x9876&&g_edx==0x7654&&g_esp==0x800&&g_eflags==0x8d7&&f.fault_address==0&&f.error_code==0);compare(&after,form);if(memcmp(actual,hardware,80)){fprintf(stderr,"stream form%u kind%u sample%u bytes differ\n",form,kinds[form],sample);assert(0);}gamepkg_x86_memory_bind(0);gamepkg_x86_memory_release(m);++checks;
}
#ifndef _WIN32
typedef struct{unsigned vector,error;uintptr_t address;Image before,after;}Report;
static Report captured;static int channel;
static void handler(int signal,siginfo_t*info,void*raw){(void)signal;ucontext_t*u=raw;captured.vector=(unsigned)u->uc_mcontext.gregs[REG_TRAPNO];captured.error=(unsigned)u->uc_mcontext.gregs[REG_ERR];captured.address=(uintptr_t)info->si_addr;const unsigned char*fp=(const unsigned char*)u->uc_mcontext.fpregs;
#ifdef __x86_64__
 memcpy(captured.after.b,fp,512);
#else
 if(get16(fp+110)!=0){_exit(91);}memcpy(captured.after.b,fp+112,512);captured.after.b[0]=fp[0];captured.after.b[1]=fp[1];captured.after.b[2]=fp[4];captured.after.b[3]=fp[5];captured.after.b[4]=0;unsigned tags=get32(fp+8);for(unsigned p=0;p<8;++p)if(((tags>>(2*p))&3)!=3)captured.after.b[4]|=(unsigned char)(1u<<p);for(unsigned i=0;i<8;++i)memcpy(captured.after.b+32+16*i,fp+28+10*i,10);
#endif
 if(write(channel,&captured,sizeof(captured))!=(ssize_t)sizeof(captured)){_exit(92);}_exit(0);
}
static unsigned physical_checks;
static void fault_cases(const Image*original,uint64_t*rng){
 /* Faulting MOVNTQ has a host-dependent TOP transition: the Intel family6,
  * model207 stepping2 CI host resets TOP on #PF with tags still empty; AMD
  * family26/model68 and EPYC9V74 preserve it. Use TOP0 for this physical
  * comparison, retaining exact status/tag/register checks rather than masking
  * the discrepancy after capture. This does not qualify nonzero-TOP #PF
  * behavior on an original Xbox CPU. The generated adapter's explicit rollback
  * policy is checked separately at every TOP in software_faults below; successful
  * instructions still compare against hardware at every TOP in one(). */
 const unsigned forms[]={0,48,96,288,296};
 unsigned scenarios=4;
#if !defined(__x86_64__)
 scenarios=6;
#endif
 for(unsigned index=0;index<5;++index)for(unsigned pending=0;pending<2;++pending)for(unsigned scenario=0;scenario<scenarios;++scenario)for(unsigned selector=0;selector<(index==2&&!pending&&scenario<2?256u:1u);++selector){unsigned form=forms[index];Image input;prepare(&input,original,form,kinds[form]==0?0:5,index==2&&!pending&&scenario<2?selector:scenario?255:0,rng);if(pending){input.b[0]=0x7e;input.b[2]|=0x81;input.b[3]|=0x80;}long page=sysconf(_SC_PAGESIZE);assert(page>=16);unsigned char*map=mmap(0,(size_t)page*2,PROT_READ|PROT_WRITE,MAP_SHARED|MAP_ANONYMOUS,-1,0);assert(map!=MAP_FAILED);memset(map,0xa5,(size_t)page*2);unsigned char*target=scenario==0?map+page-4:scenario==1?map+page:scenario==2?map+1:scenario==4?(unsigned char*)(uintptr_t)0xfffffffcu:scenario==5?(unsigned char*)(uintptr_t)0xfffffff8u:map;int fd[2];assert(pipe(fd)==0);pid_t child=fork();assert(child>=0);
  if(!child){close(fd[0]);channel=fd[1];struct rlimit core={0,0};assert(setrlimit(RLIMIT_CORE,&core)==0);if(scenario<2){assert(mprotect(map+page,(size_t)page,scenario==0?PROT_NONE:PROT_READ)==0);}
  /* Force a present read-only PTE; otherwise Linux can report a lazy,
   * nonpresent write fault for the same VMA permission denial. */
  if(scenario==1){(void)*(volatile unsigned char*)(map+page);}
  struct sigaction a={0};a.sa_sigaction=handler;a.sa_flags=SA_SIGINFO;sigemptyset(&a.sa_mask);assert(sigaction(SIGSEGV,&a,0)==0&&sigaction(SIGFPE,&a,0)==0);captured.vector=captured.error=0;captured.address=0;__asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);__asm__ volatile("fxsave %0":"=m"(captured.before)::"memory");physical[form](target);__asm__ volatile("fxsave %0":"=m"(captured.after)::"memory");__asm__ volatile("fxrstor %0"::"m"(*original): GAMEPKG_TEST_FX_CLOBBERS);if(write(fd[1],&captured,sizeof(captured))!=(ssize_t)sizeof(captured)){_exit(93);}_exit(0);}
  close(fd[1]);Report result;assert(read(fd[0],&result,sizeof(result))==(ssize_t)sizeof(result));close(fd[0]);int status;assert(waitpid(child,&status,0)==child&&WIFEXITED(status)&&WEXITSTATUS(status)==0);unsigned char actual[32];memset(actual,0xa5,sizeof(actual));uint32_t guest=scenario==0?1036u:scenario==2?1025u:scenario==4?0xfffffffcu:scenario==5?0xfffffff8u:1024u;GamePkgX86Region regions[2]={{scenario==0?guest:1024u,scenario==0?4u:32u,scenario==1?1u:3u,actual},{1028,28,3,actual+4}};GamePkgX86Memory*m=gamepkg_x86_memory_create(regions,1,1);assert(m);gamepkg_x86_memory_bind(m);import(&result.before);registers(form,guest);gamepkg_x86_set_mxcsr(get32(result.before.b+24),0,0);completed=0;GamePkgX86Fault f;int error=gamepkg_x86_run(generated[form],&f);assert(error==(result.vector!=0)&&completed==(result.vector?0u:1u));if(error){if(f.vector!=result.vector||f.eip!=0x10000||f.ebp!=local_ebp){fprintf(stderr,"stream fault kind%u form%u old%u scenario%u CPU vector%u model%u eip%x ebp%x expected%x\n",kinds[form],form,pending,scenario,result.vector,f.vector,f.eip,f.ebp,local_ebp);assert(0);}if(f.vector==14){if((result.error&7)!=f.error_code||f.fault_address!=(scenario==0?guest+4u:guest)){fprintf(stderr,"stream physical form%u kind%u pending%u scenario%u CPU PFerror%u address%llx, model error%u addr%x\n",form,kinds[form],pending,scenario,result.error&7,(unsigned long long)result.address,f.error_code,f.fault_address);assert(0);}}else assert(f.fault_address==0&&f.error_code==0);}if(gamepkg_x86_x87_state()->status!=get16(result.after.b+2))fprintf(stderr,"physical fault context: pending%u scenario%u selector%u vector%u error%u before-status%04x after-status%04x before-tags%02x after-tags%02x\n",pending,scenario,selector,result.vector,result.error,get16(result.before.b+2),get16(result.after.b+2),result.before.b[4],result.after.b[4]);compare(&result.after,form);if(scenario<4){assert(memcmp(actual+(scenario==2?1:0),target,scenario==0?4u:scenario==2?16u:32u)==0);}gamepkg_x86_memory_bind(0);gamepkg_x86_memory_release(m);assert(munmap(map,(size_t)page*2)==0);++physical_checks;
 }
}
#else
static unsigned physical_checks;
static void fault_cases(const Image*original,uint64_t*rng){(void)original;(void)rng;}
#endif
static void software_faults(const Image*original,uint64_t*rng){
 for(unsigned form=0;form<@FORMS@;++form)if(kinds[form]<3)for(unsigned top=0;top<8;++top){Image input;prepare(&input,original,form,top,0,rng);unsigned char actual[32];memset(actual,0xa5,sizeof(actual));GamePkgX86Region r={1024,4,3,actual};GamePkgX86Memory*m=gamepkg_x86_memory_create(&r,1,1);assert(m);gamepkg_x86_memory_bind(m);import(&input);registers(form,1024);gamepkg_x86_set_mxcsr(get32(input.b+24),0,0);GamePkgX87State before=*gamepkg_x86_x87_state();completed=0;GamePkgX86Fault f;assert(gamepkg_x86_run(generated[form],&f)==1&&f.vector==14&&f.fault_address==1028&&f.error_code==6&&completed==0);for(unsigned i=0;i<32;++i)assert(actual[i]==0xa5);if(kinds[form]==2){before.tags=0;before.status&=(uint16_t)~0x3800u;}assert(memcmp(&before,gamepkg_x86_x87_state(),sizeof(before))==0);gamepkg_x86_memory_bind(0);gamepkg_x86_memory_release(m);
 }
 /* Logical offsets cross the flat32 segment limit before RAM admission.
  * A nonzero FS base must not hide an overflowing logical operand. */
 for(unsigned form=0;form<288;++form)if((kinds[form]==2&&modes[form]<2)||(kinds[form]<2&&(modes[form]==0||modes[form]==1||modes[form]==3||modes[form]==5)))for(unsigned top=0;top<8;++top){Image input;prepare(&input,original,form,top,0,rng);import(&input);registers(form,0xfffffffcu);if(modes[form]==1){g_fs_base=16;if(kinds[form]==2)g_edi=0xfffffffcu;else g_esi=0xfffffffcu;}gamepkg_x86_set_mxcsr(get32(input.b+24),0,0);GamePkgX87State before=*gamepkg_x86_x87_state();if(kinds[form]==2){before.tags=0;before.status&=(uint16_t)~0x3800u;}completed=0;GamePkgX86Fault f;assert(gamepkg_x86_run(generated[form],&f)==1&&f.vector==13&&f.fault_address==0&&f.error_code==0&&completed==0&&memcmp(&before,gamepkg_x86_x87_state(),sizeof(before))==0);}
 /* Misaligned packed stores must fail before the RAM adapter is accessed. */
 for(unsigned form=48;form<96;++form)if(modes[form]!=4)for(unsigned offset=1;offset<16;++offset){Image input;prepare(&input,original,form,3,0,rng);import(&input);registers(form,1024+offset);gamepkg_x86_set_mxcsr(get32(input.b+24),0,0);GamePkgX87State before=*gamepkg_x86_x87_state();completed=0;GamePkgX86Fault f;assert(gamepkg_x86_run(generated[form],&f)==1&&f.vector==13&&f.fault_address==0&&completed==0&&memcmp(&before,gamepkg_x86_x87_state(),sizeof(before))==0);}
}
int main(void){Image original;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef);for(unsigned form=0;form<@FORMS@;++form)for(unsigned top=0;top<8;++top)for(unsigned sample=0;sample<(kinds[form]==2?256u:32u);++sample)one(&original,form,top,sample,&rng);software_faults(&original,&rng);fault_cases(&original,&rng);printf("streaming: %llu hardware/generated byte/state cases, checked-span rollback,alignment and%u physical fault/priority cases passed\n",(unsigned long long)checks,physical_checks);return 0;}
