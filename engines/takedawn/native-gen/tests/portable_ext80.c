/* Replay an asset-free, explicit-LE hardware corpus without x86 instructions. */
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include "gamepkg_x86_ext80.h"
#include "softfloat/platform.h"
_Thread_local uint_fast8_t gamepkg_softfloat_unbounded_inexact;
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Expected a big-endian target
#endif
_Static_assert(sizeof(void*)==4,"expected actual32bit pointers");
#endif
static uint32_t word(FILE *file){unsigned char bytes[4];assert(fread(bytes,1,4,file)==4);uint32_t v=0;for(unsigned i=0;i<4;++i)v|=(uint32_t)bytes[i]<<(8*i);return v;}
static GamePkgExt80 operand(FILE *file){uint64_t lo=word(file),hi=word(file);uint32_t se=word(file);assert(se<=65535);return (GamePkgExt80){lo|(hi<<32),(uint16_t)se};}
int main(int argc,char **argv){assert(argc==2);FILE *file=fopen(argv[1],"rb");assert(file);char magic[8];assert(fread(magic,1,8,file)==8&&memcmp(magic,"GPKX80v2",8)==0);uint32_t count=word(file);assert(count>0&&count<=2000000);
    for(uint32_t i=0;i<count;++i){unsigned op=word(file),cw=word(file);assert(op<=4&&cw<=65535);GamePkgExt80 a=operand(file),b=operand(file),expected=operand(file);uint32_t flags=word(file),rounded_up=word(file);uint32_t range=word(file);GamePkgExt80 wrapped=operand(file);uint32_t inexact=word(file),wrapped_up=word(file);GamePkgExt80Result out;assert(flags<=63 && gamepkg_x86_ext80_calculate(a,b,op,cw,&out)==0);if(out.value.significand!=expected.significand || out.value.sign_exponent!=expected.sign_exponent || out.exceptions!=flags||out.rounded_up!=rounded_up || out.range!=range || out.unbounded_inexact!=inexact || (range && (out.wrapped.significand!=wrapped.significand || out.wrapped.sign_exponent!=wrapped.sign_exponent || out.wrapped_rounded_up!=wrapped_up))){fprintf(stderr,"row%u op%u cw%04x actual%04x:%016llx/%u expected%04x:%016llx/%u\n",i,op,cw,out.value.sign_exponent,(unsigned long long)out.value.significand,out.exceptions,expected.sign_exponent,(unsigned long long)expected.significand,flags);return 1;}}
    assert(fgetc(file)==EOF&& !ferror(file)&&fclose(file)==0);printf("portable ext80: %u exact hardware corpus cases\n",count);return 0;
}
