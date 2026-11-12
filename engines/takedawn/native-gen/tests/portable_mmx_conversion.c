/* Explicit-LE paired-conversion corpus; no x86 instructions or host FP. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "gamepkg_x86_sse.h"
#include "gamepkg_x86_fault.h"
_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Expected big endian
#endif
_Static_assert(sizeof(void*)==4,"expected 32-bit pointers");
#endif
static uint32_t word(FILE*f){unsigned char b[4];assert(fread(b,1,4,f)==4);uint32_t v=0;for(unsigned i=0;i<4;++i)v|=(uint32_t)b[i]<<(8*i);return v;}
static uint64_t quad(FILE*f){uint64_t lo=word(f),hi=word(f);return lo|(hi<<32);}
int main(int argc,char**argv){assert(argc==2);FILE*f=fopen(argv[1],"rb");assert(f);char magic[8];assert(fread(magic,1,8,f)==8&&memcmp(magic,"GPMCVI01",8)==0);unsigned count=word(f);assert(count>0&&count<=2000000);gamepkg_x86_set_mxcsr(0x7f80,0,0);
 for(unsigned i=0;i<count;++i){unsigned op=word(f),rc=word(f);assert(op<=2&&rc<=3);uint64_t value=quad(f),expected=quad(f);unsigned flags=word(f);assert((flags&~33u)==0&&word(f)==0);uint32_t f0,f1,lo=gamepkg_x86_convert_bits((uint32_t)value,op,rc,&f0),hi=gamepkg_x86_convert_bits((uint32_t)(value>>32),op,rc,&f1);uint64_t actual=(uint64_t)lo|((uint64_t)hi<<32);
  if(actual!=expected||(f0|f1)!=flags){fprintf(stderr,"mixed row%u op%u rc%u got%016llx/%u expected%016llx/%u\n",i,op,rc,(unsigned long long)actual,f0|f1,(unsigned long long)expected,flags);return 1;}assert(gamepkg_x86_get_mxcsr()==0x7f80);
 }assert(fgetc(f)==EOF&&!ferror(f)&&fclose(f)==0);printf("portable paired conversion: %u exact hardware cases\n",count);return 0;
}
