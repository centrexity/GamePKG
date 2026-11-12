#include "gamepkg_x86_x87.h"
#include <stdlib.h>
uint64_t gamepkg_x86_mmx_lanes(uint64_t a,uint64_t b,uint32_t op,uint32_t control){
    if(op>10 || control>255 || (op<7&&control) || (op==10&&control))abort();
    uint64_t result=0;
    if(op==7){for(unsigned i=0;i<4;++i)result|=((a>>(((control>>(2*i))&3)*16))&65535)<<(16*i);return result;}
    if(op==8){unsigned shift=(control&3)*16;uint64_t mask=UINT64_C(65535)<<shift;return (a&~mask)|((b&65535)<<shift);}
    if(op==9)return (a>>((control&3)*16))&65535;
    if(op==10){for(unsigned i=0;i<8;++i)result|=((a>>(8*i+7))&1)<<i;return result;}
    unsigned width=op==1||op==4||op==5?16:8;
    uint64_t mask=(UINT64_C(1)<<width)-1;
    for(unsigned shift=0;shift<64;shift+=width){uint64_t x=(a>>shift)&mask,y=(b>>shift)&mask,z;
        if(op<=1)z=(x+y+1)>>1;
        else if(op==2)z=x<y?x:y;
        else if(op==3)z=x>y?x:y;
        else if(op==4)z=(x^32768)<(y^32768)?x:y;
        else if(op==5)z=(x^32768)>(y^32768)?x:y;
        else {result+=x>y?x-y:y-x;continue;}
        result|=z<<shift;
    }return result;
}
