#ifndef GAMEPKG_FPATAN_H
#define GAMEPKG_FPATAN_H
#include "../gamepkg_x86_f128.h"
/* Finite nonzero inputs. Source flags and guest commit remain caller-owned. */
int gamepkg_fpatan_evaluate(GamePkgExt80 x, GamePkgExt80 y, uint32_t rc,
                            GamePkgF128 *out, int32_t *adjust);
#endif
