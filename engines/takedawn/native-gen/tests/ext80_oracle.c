/* Synthetic raw80-bit operands, never game data or host C float casts. */
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
#include <stdlib.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#include "gamepkg_x86_ext80.h"
#include "softfloat/platform.h"
#include "softfloat/source/include/softfloat.h"
_Thread_local uint_fast8_t gamepkg_softfloat_unbounded_inexact;
typedef struct {_Alignas(16) unsigned char b[512];}Image;
static void put64(unsigned char *b,uint64_t v){for(unsigned i=0;i<8;++i)b[i]=(unsigned char)(v>>(8*i));}
static uint64_t get64(const unsigned char *b){uint64_t v=0;for(unsigned i=0;i<8;++i)v|=(uint64_t)b[i]<<(8*i);return v;}
static void put80(unsigned char *b,GamePkgExt80 v){put64(b,v.significand);b[8]=(unsigned char)v.sign_exponent;b[9]=(unsigned char)(v.sign_exponent>>8);}
static GamePkgExt80Result hardware(GamePkgExt80 a,GamePkgExt80 b,unsigned op,unsigned cw){
    Image old,input,after;__asm__ volatile("fxsave %0":"=m"(old)::"memory");input=old;input.b[0]=(unsigned char)cw;input.b[1]=(unsigned char)(cw>>8);input.b[2]=input.b[3]=0;input.b[4]=3;put80(input.b+32,op==4?b:a);put80(input.b+48,b);__asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);
    switch(op){case 0:__asm__ volatile(".byte 0xd8,0xc1":::"memory");break;case 1:__asm__ volatile(".byte 0xd8,0xe1":::"memory");break;case 2:__asm__ volatile(".byte 0xd8,0xc9":::"memory");break;case 3:__asm__ volatile(".byte 0xd8,0xf1":::"memory");break;default:__asm__ volatile("fsqrt":::"memory");break;}
    __asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(old): GAMEPKG_TEST_FX_CLOBBERS);
    return (GamePkgExt80Result){.value={get64(after.b+32),(uint16_t)(after.b[40]|((unsigned)after.b[41]<<8))},.exceptions=after.b[2]&63u,.rounded_up=(after.b[3]>>1)&1u};
}
static uint64_t checks,range_checks;
static FILE *corpus;
static void word(uint32_t value){unsigned char bytes[4];for(unsigned i=0;i<4;++i)bytes[i]=(unsigned char)(value>>(8*i));assert(fwrite(bytes,1,4,corpus)==4);}
static void operand(GamePkgExt80 v){word((uint32_t)v.significand);word((uint32_t)(v.significand>>32));word(v.sign_exponent); }
static void check(GamePkgExt80 a,GamePkgExt80 b,unsigned op,unsigned cw){
    GamePkgExt80Result expected=hardware(a,b,op,cw),actual;
    softfloat_roundingMode=softfloat_round_odd;softfloat_detectTininess=softfloat_tininess_beforeRounding;softfloat_exceptionFlags=0x1f;extF80_roundingPrecision=64;gamepkg_softfloat_unbounded_inexact=0x55;
    assert(gamepkg_x86_ext80_calculate(a,b,op,cw,&actual)==0);
    if(actual.value.significand!=expected.value.significand || actual.value.sign_exponent!=expected.value.sign_exponent || actual.exceptions!=expected.exceptions || actual.rounded_up!=expected.rounded_up){fprintf(stderr,"ext80 op=%u cw=%04x a=%04x:%016llx b=%04x:%016llx actual=%04x:%016llx/%02x expected=%04x:%016llx/%02x\n",op,cw,a.sign_exponent,(unsigned long long)a.significand,b.sign_exponent,(unsigned long long)b.significand,actual.value.sign_exponent,(unsigned long long)actual.value.significand,actual.exceptions,expected.value.sign_exponent,(unsigned long long)expected.value.significand,expected.exceptions);fprintf(stderr,"C1 actual%u expected%u\n",actual.rounded_up,expected.rounded_up);assert(0);}
    if(actual.range){
        GamePkgExt80Result real=hardware(a,b,op,cw&~actual.range);
        uint32_t flags=(actual.exceptions&~(actual.range|32u))|actual.range|(actual.unbounded_inexact?32u:0u);
        if(real.value.significand!=actual.wrapped.significand || real.value.sign_exponent!=actual.wrapped.sign_exponent || real.exceptions!=flags || real.rounded_up!=actual.wrapped_rounded_up){fprintf(stderr,"range op%u cw%04x range%u actual%04x:%016llx/%02x c1%u expected%04x:%016llx/%02x c1%u\n",op,cw,actual.range,actual.wrapped.sign_exponent,(unsigned long long)actual.wrapped.significand,flags,actual.wrapped_rounded_up,real.value.sign_exponent,(unsigned long long)real.value.significand,real.exceptions,real.rounded_up);assert(0);}++range_checks;
    }
    if(!actual.range && (actual.value.sign_exponent&0x7fff)<=1){GamePkgExt80Result real=hardware(a,b,op,cw&~16u);assert(real.value.significand==actual.value.significand&&real.value.sign_exponent==actual.value.sign_exponent&&real.exceptions==actual.exceptions&&real.rounded_up==actual.rounded_up);}
    assert(softfloat_roundingMode==softfloat_round_odd && softfloat_detectTininess==softfloat_tininess_beforeRounding&&softfloat_exceptionFlags==0x1f&&extF80_roundingPrecision==64&&gamepkg_softfloat_unbounded_inexact==0x55);
    if(corpus){word(op);word(cw);operand(a);operand(b);operand(expected.value);word(expected.exceptions);word(expected.rounded_up);word(actual.range);operand(actual.wrapped);word(actual.unbounded_inexact);word(actual.wrapped_rounded_up);}++checks;
}
static uint64_t random64(uint64_t *v){*v^=*v<<13;*v^=*v>>7;*v^=*v<<17;return *v;}
#ifdef _WIN32
static DWORD WINAPI worker(void *raw){
#else
static void *worker(void *raw){
#endif
    unsigned tag=(unsigned)(uintptr_t)raw;assert(softfloat_roundingMode==softfloat_round_near_even&&softfloat_exceptionFlags==0&&extF80_roundingPrecision==80&&gamepkg_softfloat_unbounded_inexact==0);uint64_t random=UINT64_C(0x123456789abcdef)+tag;
    softfloat_roundingMode=softfloat_round_odd;softfloat_detectTininess=softfloat_tininess_beforeRounding;softfloat_exceptionFlags=0x1f;extF80_roundingPrecision=64;gamepkg_softfloat_unbounded_inexact=0x55;
    for(unsigned i=0;i<1000;++i){static const unsigned pcs[]={0,2,3};unsigned cw=0x7f|(pcs[(tag+i)%3]<<8)|(((tag+i)&3)<<10),op=i%5;GamePkgExt80 a={random64(&random)|UINT64_C(0x8000000000000000),(uint16_t)random64(&random)},b={random64(&random)|UINT64_C(0x8000000000000000),(uint16_t)random64(&random)};GamePkgExt80Result real=hardware(a,b,op,cw),actual;assert(gamepkg_x86_ext80_calculate(a,b,op,cw,&actual)==0&&actual.value.significand==real.value.significand&&actual.value.sign_exponent==real.value.sign_exponent&&actual.exceptions==real.exceptions&&actual.rounded_up==real.rounded_up);assert(softfloat_roundingMode==softfloat_round_odd&&softfloat_detectTininess==softfloat_tininess_beforeRounding&&softfloat_exceptionFlags==0x1f&&extF80_roundingPrecision==64&&gamepkg_softfloat_unbounded_inexact==0x55);}
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
    const char *corpus_path=getenv("GAMEPKG_TAKEDAWN_EXT80_CORPUS");if(corpus_path){corpus=fopen(corpus_path,"wb");assert(corpus);assert(fwrite("GPKX80v2",1,8,corpus)==8);word(0);}
    const GamePkgExt80 edges[]={
        {0,0},{0,0x8000},{UINT64_C(0x8000000000000000),0x3fff},{UINT64_C(0x8000000000000000),0xbfff},{UINT64_C(0x8000000000000001),0x3fff},{UINT64_MAX,0x3fff},{UINT64_C(0xc000000000000000),0x4000},
        {UINT64_C(0x8000000000000000),1},{UINT64_MAX,0x7ffe},{UINT64_MAX,0xfffe},{1,0},{1,0x8000},{UINT64_C(0x7fffffffffffffff),0},{UINT64_C(0x8000000000000000),0},{UINT64_MAX,0},
        {UINT64_C(0x8000000000000000),0x7fff},{UINT64_C(0x8000000000000000),0xffff},{UINT64_C(0xc000000000000001),0x7fff},{UINT64_C(0xc000000000000007),0xffff},{UINT64_C(0x8000000000000001),0x7fff},{UINT64_C(0x8000000000000007),0xffff},
        {0,1},{1,1},{UINT64_C(0x4000000000000000),0x3fff},{0,0x7fff},{1,0xffff}
    };
    uint64_t rng=0x123456789abcdef;
    for(unsigned op=0;op<5;++op)for(unsigned pc=0;pc<4;++pc){for(unsigned rc=0;rc<4;++rc){unsigned cw=0x7f|(pc<<8)|(rc<<10);
        for(unsigned i=0;i<sizeof(edges)/sizeof(*edges);++i)for(unsigned j=0;j<sizeof(edges)/sizeof(*edges);++j)check(edges[i],edges[j],op,cw);
        for(unsigned i=0;i<16384;++i){GamePkgExt80 a={random64(&rng)|UINT64_C(0x8000000000000000),(uint16_t)random64(&rng)},b={random64(&rng)|UINT64_C(0x8000000000000000),(uint16_t)random64(&rng)};check(a,b,op,cw);}
    }}
    GamePkgExt80Result untouched,copy;memset(&untouched,0xa5,sizeof(untouched));memcpy(&copy,&untouched,sizeof(copy));assert(gamepkg_x86_ext80_calculate(edges[0],edges[0],5,0x37f,&untouched)==-1&&memcmp(&copy,&untouched,sizeof(copy))==0);assert(gamepkg_x86_ext80_calculate(edges[0],edges[0],0,0x17f,0)==-1);
    threads();
    if(corpus){assert(checks<=UINT32_MAX && fseek(corpus,8,SEEK_SET)==0);word((uint32_t)checks);assert(fclose(corpus)==0);}
    printf("ext80: %llu raw hardware result/exception comparisons; private math state preserved;8000 threaded cases and%llu wrapped range cases passed\n",(unsigned long long)checks,(unsigned long long)range_checks);return 0;
}
