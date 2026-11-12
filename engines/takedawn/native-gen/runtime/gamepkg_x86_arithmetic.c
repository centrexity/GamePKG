#include "gamepkg_x86_arithmetic.h"
#include "gamepkg_x86_fault.h"
#include "softfloat/platform.h"
#include "softfloat/source/include/softfloat.h"
#include <stdlib.h>
_Thread_local uint_fast8_t gamepkg_softfloat_unbounded_inexact;

static uint32_t preconditions(uint32_t a,uint32_t b,uint32_t operation) {
    uint32_t aa=a&0x7fffffffu,bb=b&0x7fffffffu;
    if (operation==4) {
        if (bb>0x7f800000u) return (b&0x400000u)?0:1;
        if ((b&0x80000000u) && bb) return 1;
        return bb && bb<0x800000u ? 2:0;
    }
    if (aa>0x7f800000u || bb>0x7f800000u) {
        return ((aa>0x7f800000u && !(a&0x400000u)) ||
                (bb>0x7f800000u && !(b&0x400000u))) ? 1:0;
    }
    if (operation<2 && aa==0x7f800000u && bb==0x7f800000u &&
        ((a^b)>>31u)==(operation==0?1u:0u)) return 1;
    if (operation==2 && ((!aa && bb==0x7f800000u) ||
                         (!bb && aa==0x7f800000u))) return 1;
    if (operation==3) {
        if ((!aa && !bb) || (aa==0x7f800000u && bb==0x7f800000u)) return 1;
        /* Divide-by-zero, like invalid, takes precedence over a denormal in
         * the same lane. Infinity divided by zero is already infinity. */
        if (!bb && aa && aa<0x7f800000u) return 4;
    }
    return ((aa && aa<0x800000u) || (bb && bb<0x800000u)) ? 2:0;
}

static float32_t calculate(uint32_t a,uint32_t b,uint32_t operation) {
    float32_t left={a},right={b};
    switch (operation) {
        case 0:return f32_add(left,right);
        case 1:return f32_sub(left,right);
        case 2:return f32_mul(left,right);
        case 3:return f32_div(left,right);
        case 4:return f32_sqrt(right);
        default:abort();
    }
}

void gamepkg_x86_arithmetic_f32(const uint32_t *left,const uint32_t *right,
    uint32_t *result,uint32_t lanes,uint32_t operation,uint32_t eip,uint32_t ebp) {
    if ((lanes!=1 && lanes!=4) || operation>4) abort();
    uint32_t initial=gamepkg_x86_get_mxcsr(),pre=0,post=0,staged[4];
    for (uint32_t i=0;i<lanes;++i) pre |= preconditions(left[i],right[i],operation);
    /* All packed pre-computation conditions precede every lane's result and
     * post-computation conditions. An unmasked precondition exits here. */
    gamepkg_x86_sse_status(pre,eip,ebp);

    uint_fast8_t old_round=softfloat_roundingMode;
    uint_fast8_t old_tiny=softfloat_detectTininess;
    uint_fast8_t old_flags=softfloat_exceptionFlags;
    uint_fast8_t old_unbounded=gamepkg_softfloat_unbounded_inexact;
    static const uint_fast8_t rounds[4]={softfloat_round_near_even,
        softfloat_round_min,softfloat_round_max,softfloat_round_minMag};
    softfloat_roundingMode=rounds[(initial>>13u)&3u];
    softfloat_detectTininess=softfloat_tininess_afterRounding;
    for (uint32_t i=0;i<lanes;++i) {
        softfloat_exceptionFlags=0;
        gamepkg_softfloat_unbounded_inexact=0;
        staged[i]=calculate(left[i],right[i],operation).v;
        uint_fast8_t raised=softfloat_exceptionFlags;
        uint32_t classified=((raised&softfloat_flag_invalid)?1u:0u) |
                            ((raised&softfloat_flag_infinite)?4u:0u);
        if (classified & ~pre) abort();
        uint32_t flags=((raised&softfloat_flag_overflow)?8u:0u) |
                       ((raised&softfloat_flag_underflow)?16u:0u) |
                       ((raised&softfloat_flag_inexact)?32u:0u);
        uint32_t magnitude=staged[i]&0x7fffffffu;
        int tiny=(flags&16u) || (magnitude && magnitude<0x800000u);
        /* Unmasked underflow reports exact tiny results too. Masked gradual
         * underflow requires lost accuracy; FTZ deliberately loses it. */
        if (tiny && !(initial&0x800u)) flags |= 16u;
        if (tiny && (initial&0x8800u)==0x8800u) {
            staged[i] &= 0x80000000u;
            flags |= 0x30u;
        }
        /* With unmasked overflow/underflow, precision reflects rounding at
         * unbounded exponent, not accuracy lost by range conversion. Other
         * lanes independently contribute precision to the packed event. */
        if (((flags&8u) && !(initial&0x400u)) ||
            ((flags&16u) && !(initial&0x800u))) {
            flags &= ~0x20u;
            if (gamepkg_softfloat_unbounded_inexact) flags |= 0x20u;
        }
        post |= flags;
    }
    /* Restore private library state before any guest fault can abandon this
     * frame. No host FP environment is read or changed by SoftFloat. */
    softfloat_roundingMode=old_round;
    softfloat_detectTininess=old_tiny;
    softfloat_exceptionFlags=old_flags;
    gamepkg_softfloat_unbounded_inexact=old_unbounded;
    gamepkg_x86_sse_status(post,eip,ebp);
    for (uint32_t i=0;i<lanes;++i) result[i]=staged[i];
}
