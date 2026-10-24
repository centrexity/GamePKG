#ifndef GAMEPKG_X86_EXT80_H
#define GAMEPKG_X86_EXT80_H
#include <stdint.h>
typedef struct GamePkgExt80 {uint64_t significand;uint16_t sign_exponent;}GamePkgExt80;
typedef struct GamePkgExt80Result {
    GamePkgExt80 value; uint32_t exceptions,rounded_up;
    GamePkgExt80 wrapped; uint32_t range,unbounded_inexact,wrapped_rounded_up;
}GamePkgExt80Result;
/* Non-committing arithmetic backend, not a guest instruction interpreter.
 * op0=add,1=subtract,2=multiply,3=divide,4=sqrt(right).
 * Implements default masked numerical results and reports newly raised x87
 * IE/DE/ZE/OE/UE/PE. All four PC bit patterns and RC settings are supported;
 * reserved PC01 follows the measured 64-bit-significand profile. Caller must
 * still implement
 * stack/tags, deferred delivery, FIP/FDP and instruction-specific commit/pop
 * semantics. rounded_up is the masked-result C1. range is8/16 for a rounded
 * unbounded exponent outside the extended range; wrapped and its C1 apply
 * when that range exception is unmasked. unbounded_inexact excludes losses
 * from clamping/denormalization. Wrapped fields are meaningful only if range. */
int gamepkg_x86_ext80_calculate(GamePkgExt80 left,GamePkgExt80 right,
    uint32_t operation,uint32_t control,GamePkgExt80Result *result);
/* FSCALE: truncate the right operand independently of RC, ignore PC, then
 * apply extended-range rounding and the same wrapped-range result protocol. */
int gamepkg_x86_ext80_scale(GamePkgExt80 left,GamePkgExt80 right,
    uint32_t control,GamePkgExt80Result *result);
/* One FPREM/FPREM1 step. nearest0 truncates,1 rounds quotient to nearest even;
 * PC/RC have no effect. condition returns the architectural C0/C1/C2/C3 bits. */
int gamepkg_x86_ext80_remainder(GamePkgExt80 left,GamePkgExt80 right,
    uint32_t nearest,GamePkgExt80Result *result,uint32_t *condition);
/* Round logical binary128 words with an exact binary exponent adjustment to
 * raw80, ignoring PC. Noncommitting; C1 and wrapped range protocol as above. */
int gamepkg_x86_ext80_round_f128(const uint32_t words[4],int32_t adjust,
    uint32_t control,GamePkgExt80Result *result);
#endif
