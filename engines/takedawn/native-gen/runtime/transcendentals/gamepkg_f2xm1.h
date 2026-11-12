#ifndef GAMEPKG_F2XM1_H
#define GAMEPKG_F2XM1_H
#include "../gamepkg_x86_f128.h"
/* Internal numerical evaluation only, for nonzero finite |x|<1. The returned
 * binary128 value is scaled by2^adjust before the sole destination rounding.
 * Source classification/DE/PE, stack/status/commit are the host adapter's job.
 */
int gamepkg_f2xm1_evaluate(GamePkgExt80 input, uint32_t rc, GamePkgF128 *result,
                           int32_t *adjust);
#endif
