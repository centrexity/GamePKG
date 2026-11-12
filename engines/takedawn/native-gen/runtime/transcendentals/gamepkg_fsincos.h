#ifndef GAMEPKG_FSINCOS_H
#define GAMEPKG_FSINCOS_H
#include "../gamepkg_x86_f128.h"
/* Numerical standalone sin/cos for finite nonzero |x|<2^63. which0=sin,1=cos.
 */
int gamepkg_fsincos_evaluate(GamePkgExt80 input, uint32_t which,
                             uint32_t control, GamePkgExt80Result *out);
/* Separate combined-instruction polynomial and tangent reconstruction paths. */
int gamepkg_fsincos_pair(GamePkgExt80 input, uint32_t control,
                         GamePkgExt80Result *sine, GamePkgExt80Result *cosine);
int gamepkg_fptan_evaluate(GamePkgExt80 input, uint32_t control,
                           GamePkgExt80Result *out);
#endif
