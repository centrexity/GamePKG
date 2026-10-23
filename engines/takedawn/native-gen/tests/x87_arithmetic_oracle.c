#define main unused_ext80_oracle_main
/* FXRSTOR overwrites compiler-held XMM temporaries as well as x87 state. */
#ifndef GAMEPKG_TEST_FX_CLOBBERS
#ifdef __x86_64__
#define GAMEPKG_TEST_FX_CLOBBERS "memory","xmm0","xmm1","xmm2","xmm3","xmm4","xmm5","xmm6","xmm7","xmm8","xmm9","xmm10","xmm11","xmm12","xmm13","xmm14","xmm15"
#else
#define GAMEPKG_TEST_FX_CLOBBERS "memory","xmm0","xmm1","xmm2","xmm3","xmm4","xmm5","xmm6","xmm7"
#endif
#endif
#include "ext80_oracle.c"
#undef main
#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_fault.h"
_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
static unsigned completed;
static void next_waiting(void){gamepkg_x86_x87_arithmetic(0,0,0,0,0,0x20000,0xc0,0x9876);++completed;}
static void (*const generated[55])(void);
static void (*const physical[55])(void);
static const unsigned opcodes[55];
static unsigned classify(GamePkgExt80 v){unsigned e=v.sign_exponent&0x7fff;return !e&&!v.significand?1:e&&e!=0x7fff&&(v.significand>>63)?0:2;}
static void install(GamePkgX87State *s,Image *image,unsigned cw,unsigned top,unsigned empties,unsigned op,uint64_t *rng){
    static const GamePkgExt80 inputs[]={
        {UINT64_C(0x8000000000000000),0x3fff},{UINT64_MAX,0x7ffe},{1,0},{0,0},{UINT64_C(0x8000000000000000),0x7fff},{UINT64_C(0xc000000000000001),0x7fff},{UINT64_C(0x8000000000000001),0x7fff},{UINT64_C(0x8000000000000000),1},{UINT64_C(0x8000000000000000),0x4000},{UINT64_C(0xc000000000000000),0xbfff},{UINT64_C(0x123456789),0xffff},{UINT64_C(0x4000000000000000),0x3fff},{UINT64_C(0x8000000000000000),0},{UINT64_C(0xc000000000000007),0xffff}
    };
    memset(s,0,sizeof(*s));s->control=(uint16_t)cw;s->status=(uint16_t)((top<<11)|0x4700);s->data_pointer=0x12345678;
    image->b[0]=(unsigned char)cw;image->b[1]=(unsigned char)(cw>>8);image->b[2]=(unsigned char)s->status;image->b[3]=(unsigned char)(s->status>>8);image->b[4]=0xff;
    for(unsigned i=0;i<8;++i){unsigned p=(top+i)&7;GamePkgExt80 v;
        if((op+cw+top+empties+i)%3)v=inputs[(op+cw+top+i+empties)%(sizeof(inputs)/sizeof(*inputs))];else v=(GamePkgExt80){random64(rng)|UINT64_C(0x8000000000000000),(uint16_t)random64(rng)};
        s->significand[p]=v.significand;s->sign_exponent[p]=v.sign_exponent;unsigned tag=classify(v);
        if((i==0&&(empties&1))||(i==1&&(empties&2))){tag=3;image->b[4]&=(unsigned char)~(1u<<p);}s->tags|=(uint16_t)(tag<<(2*p));put80(image->b+32+16*i,v);
    }
}
int main(void){Image original,input,after;__asm__ volatile("fxsave %0":"=m"(original)::"memory");uint64_t rng=UINT64_C(0x123456789abcdef),count=0,queued=0;
    for(unsigned op=0;op<55;++op)for(unsigned masks=0;masks<64;++masks)for(unsigned pc=0;pc<4;++pc){for(unsigned rc=0;rc<4;++rc)for(unsigned top=0;top<8;++top)for(unsigned empty=0;empty<4;++empty){
        input=original;unsigned cw=0x40|masks|(pc<<8)|(rc<<10);GamePkgX87State initial;install(&initial,&input,cw,top,empty,op,&rng);
        __asm__ volatile("fxrstor %0"::"m"(input): GAMEPKG_TEST_FX_CLOBBERS);physical[op]();__asm__ volatile("fxsave %0":"=m"(after)::"memory");__asm__ volatile("fxrstor %0"::"m"(original): GAMEPKG_TEST_FX_CLOBBERS);
        *gamepkg_x86_x87_state()=initial;g_eflags=0x8d7;g_eax=0x1234;g_esp=0x800;g_ebp=0x777;completed=0;GamePkgX86Fault fault;
        assert(gamepkg_x86_run(generated[op],&fault)==0&&completed==1&&g_eflags==0x8d7&&g_eax==0x1234&&g_esp==0x800);
        GamePkgX87State *actual=gamepkg_x86_x87_state();unsigned sw=after.b[2]|((unsigned)after.b[3]<<8),newtop=(sw>>11)&7,valid=0;
        if((actual->status&~0x4500u)!=(sw&~0x4500u)){fprintf(stderr,"x87 op%u opcode%03x cw%04x top%u empty%u status actual%04x expected%04x\n",op,opcodes[op],cw,top,empty,actual->status,sw);assert(0);}
        for(unsigned i=0;i<8;++i){unsigned p=(newtop+i)&7;uint64_t sig=get64(after.b+32+16*i);unsigned se=after.b[40+16*i]|((unsigned)after.b[41+16*i]<<8);if(actual->significand[p]!=sig||actual->sign_exponent[p]!=se){fprintf(stderr,"x87 op%u cw%04x top%u empty%u p%u actual%04x:%016llx expected%04x:%016llx sw%04x\n",op,cw,top,empty,p,actual->sign_exponent[p],(unsigned long long)actual->significand[p],se,(unsigned long long)sig,sw);assert(0);}if(((actual->tags>>(2*p))&3)!=3)valid|=1u<<p;}
        assert(valid==after.b[4]&&actual->instruction_pointer==0x10000&&actual->opcode==opcodes[op]&&actual->data_pointer==initial.data_pointer);
        if(sw&~cw&63u){GamePkgX87State old=*actual;completed=0;assert(gamepkg_x86_run(next_waiting,&fault)==1&&fault.vector==16&&fault.eip==0x20000&&fault.ebp==0x9876&&completed==0&&memcmp(actual,&old,sizeof(old))==0);++queued;}++count;
    }}
    printf("x87 arithmetic: %llu hardware/generated state cases; %llu deferred faults\n",(unsigned long long)count,(unsigned long long)queued);return 0;
}
