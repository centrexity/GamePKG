#ifndef GAMEPKG_FYL2X_H
#define GAMEPKG_FYL2X_H
#include "../gamepkg_x86_f128.h"
/* Finite positive x!=1, finite nonzero y; returned value scales by2^adjust. */
int gamepkg_fyl2x_evaluate(GamePkgExt80 x, GamePkgExt80 y, uint32_t rc,
                           GamePkgF128 *out, int32_t *adjust);
/* Finite nonzero x>-1 and finite nonzero y. Source classification is external.
 */
int gamepkg_fyl2xp1_evaluate(GamePkgExt80 x, GamePkgExt80 y, uint32_t rc,
                             GamePkgF128 *out, int32_t *adjust);
#endif
