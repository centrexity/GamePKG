#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_x87_decode.h"
#include <stdlib.h>
static unsigned classify(GamePkgExt80 v){unsigned e=v.sign_exponent&0x7fff;return !e&&!v.significand?1:e&&e!=0x7fff&&(v.significand>>63)?0:2;}
void gamepkg_x86_x87_load(uint32_t kind,uint32_t low,uint32_t high,uint32_t upper,
    uint32_t ip,uint32_t opcode,uint32_t address,uint32_t ebp){
    if(kind>14 || opcode>0x7ff || (kind==6&&low>7) || ((kind==2||kind==14)&&upper>65535))abort();
    gamepkg_x86_x87_wait(ip,ebp);
    GamePkgX87State *s=gamepkg_x86_x87_state();unsigned top=(s->status>>11)&7,dst=(top+7)&7,raised=0,c1=0;
    GamePkgExt80 value={0,0};gamepkg_x86_x87_record(s,ip,opcode);if(kind<=5||kind==14)gamepkg_x86_x87_record_data(s,address);
    if(((s->tags>>(2*dst))&3)!=3){raised=0x41;c1=1;value=(GamePkgExt80){UINT64_C(0xc000000000000000),0xffff};}
    else if(kind==6){unsigned src=(top+low)&7;if(((s->tags>>(2*src))&3)==3){raised=0x41;value=(GamePkgExt80){UINT64_C(0xc000000000000000),0xffff};}else value=(GamePkgExt80){s->significand[src],s->sign_exponent[src]};}
    else if(kind==0)value=gamepkg_x87_decode_float(low,23,8,127,1,&raised);
    else if(kind==1)value=gamepkg_x87_decode_float((uint64_t)low|((uint64_t)high<<32),52,11,1023,1,&raised);
    else if(kind==2)value=(GamePkgExt80){(uint64_t)low|((uint64_t)high<<32),(uint16_t)upper};
    else if(kind<=5)value=gamepkg_x87_decode_integer((uint64_t)low|((uint64_t)high<<32),kind==3?16:kind==4?32:64);
    else if(kind==14){
        /* Eighteen packed digits and one sign bit. Bits72..78 are ignored.
         * Intel leaves invalid A..F digit results undefined, without an
         * invalid-operand check. Accumulation stays within uint64 even for
         * those encodings; valid BCD integers fit exactly in extended80. */
        uint64_t magnitude=0,place=1;
        for(unsigned i=0;i<18;++i){
            uint32_t word=i<8?low:i<16?high:upper;
            unsigned digit=(word>>((i%8)*4))&15;
            magnitude+=place*digit;
            place*=10;
        }
        value=gamepkg_x87_decode_integer(magnitude,64);
        value.sign_exponent|=(uint16_t)(upper&0x8000);
    }
    else {
        static const uint64_t floor_sig[]={UINT64_C(0x8000000000000000),UINT64_C(0xd49a784bcd1b8afe),UINT64_C(0xb8aa3b295c17f0bb),UINT64_C(0xc90fdaa22168c234),UINT64_C(0x9a209a84fbcff798),UINT64_C(0xb17217f7d1cf79ab),0};
        static const uint16_t exponents[]={0x3fff,0x4000,0x3fff,0x4000,0x3ffd,0x3ffe,0};
        static const unsigned nearest_up[]={0,0,1,1,1,1,0};unsigned which=kind-7,rc=(s->control>>10)&3;
        value=(GamePkgExt80){floor_sig[which],exponents[which]};if((rc==0&&nearest_up[which])||(rc==2&&which>0&&which<6))++value.significand;
    }
    s->status=(uint16_t)((s->status&~0x200u)|raised|(c1?0x200:0));if(raised&~s->control&63u)s->status|=0x8080;
    /* A newly unmasked denormal load still pushes its exact extended value.
     * Invalid operand/stack faults suppress the push when IM is clear. */
    if((raised&1)&&!(s->control&1))return;
    s->significand[dst]=value.significand;s->sign_exponent[dst]=value.sign_exponent;
    s->tags=(uint16_t)((s->tags&~(3u<<(2*dst)))|(classify(value)<<(2*dst)));
    s->status=(uint16_t)((s->status&~0x3800u)|(dst<<11));
}
