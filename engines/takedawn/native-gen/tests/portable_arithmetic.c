/* Asset-free consumer of the actual x86 arithmetic oracle's explicit LE
 * corpus. No x86 instructions or native floating-point expressions. */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_arithmetic.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error This qualification requires a real big-endian target
#endif
_Static_assert(sizeof(void*)==4,"This qualification requires a 32bit target");
#endif

_Thread_local uint32_t g_eax,g_ecx,g_edx,g_ebx,g_esp,g_ebp,g_esi,g_edi,g_eflags,g_fs_base;
_Thread_local int g_df;
static uint32_t left[4],right[4],result[4],operation,lanes;
static uint32_t le32(const unsigned char *bytes) {
    uint32_t value=0;
    for (unsigned i=0;i<4;++i) value |= (uint32_t)bytes[i]<<(8u*i);
    return value;
}
static void run(void) {
    gamepkg_x86_arithmetic_f32(left,right,result,lanes,operation,0x10000,0x1234);
}
int main(int argc,char **argv) {
    assert(argc==2);
    FILE *file=fopen(argv[1],"rb");assert(file);
    unsigned char header[12];
    assert(fread(header,1,sizeof(header),file)==sizeof(header));
    assert(memcmp(header,"GPKF32v2",8)==0);
    uint32_t count=le32(header+8);assert(count>0&&count<=1000000);
    for (uint32_t row=0;row<count;++row) {
        unsigned char bytes[68];uint32_t words[17];
        assert(fread(bytes,1,sizeof(bytes),file)==sizeof(bytes));
        for (unsigned i=0;i<17;++i) words[i]=le32(bytes+4*i);
        operation=words[0];lanes=words[1];uint32_t initial=words[2];
        assert(operation<=4&&(lanes==1||lanes==4)&&!(initial&~0xffbfu)&&words[3]<=1);
        for (unsigned i=0;i<4;++i) {left[i]=words[4+i];right[i]=words[8+i];result[i]=left[i];}
        gamepkg_x86_set_mxcsr(initial,0,0);g_eflags=0x8d7;
        GamePkgX86Fault fault;int status=gamepkg_x86_run(run,&fault);
        if (status!=(int)words[3] || memcmp(result,words+12,sizeof(result)) || gamepkg_x86_get_mxcsr()!=words[16]) {
            fprintf(stderr,"portable arithmetic mismatch row=%u op=%u lanes=%u status=%x/%x\n",row,operation,lanes,gamepkg_x86_get_mxcsr(),words[16]);return 1;
        }
        if (status) assert(fault.vector==19&&fault.eip==0x10000&&fault.ebp==0x1234&&fault.mxcsr==words[16]);
        assert(g_eflags==0x8d7);
    }
    assert(fgetc(file)==EOF&&!ferror(file)&&fclose(file)==0);
    printf("portable arithmetic: %u exact x86 corpus cases passed; pointer_bits=%u\n",count,(unsigned)(sizeof(void*)*8));
    return 0;
}
