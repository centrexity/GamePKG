/* Asset-free hardware corpus replay through the production integer service. */
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include "gamepkg_x86_x87.h"
_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Expected a big-endian target
#endif
_Static_assert(sizeof(void*)==4,"expected actual 32-bit pointers");
#endif
static uint32_t word(FILE *f){unsigned char b[4];assert(fread(b,1,4,f)==4);uint32_t v=0;for(unsigned i=0;i<4;++i)v|=(uint32_t)b[i]<<(8*i);return v;}
static uint64_t quad(FILE *f){uint64_t lo=word(f),hi=word(f);return lo|(hi<<32);}
int main(int argc,char **argv){assert(argc==2);FILE *f=fopen(argv[1],"rb");assert(f);char magic[8];assert(fread(magic,1,8,f)==8&&memcmp(magic,"GPMLAN01",8)==0);unsigned count=word(f);assert(count>0&&count<=2000000);
    for(unsigned i=0;i<count;++i){unsigned op=word(f),control=word(f);assert(op<=10&&control<=255&&((op>=7&&op<=9)||control==0));uint64_t a=quad(f),b=quad(f),expected=quad(f),actual=gamepkg_x86_mmx_lanes(a,b,op,control);
        if(actual!=expected){fprintf(stderr,"lane row%u op%u control%u b%016llx got%016llx expected%016llx\n",i,op,control,(unsigned long long)b,(unsigned long long)actual,(unsigned long long)expected);return 1;}
    }assert(fgetc(f)==EOF&&!ferror(f)&&fclose(f)==0);printf("portable MMX lanes: %u exact hardware cases\n",count);return 0;
}
