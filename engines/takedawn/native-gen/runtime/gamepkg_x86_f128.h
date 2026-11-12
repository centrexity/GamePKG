#ifndef GAMEPKG_X86_F128_H
#define GAMEPKG_X86_F128_H
#include "gamepkg_x86_ext80.h"
/* Reviewed internal numerical work space for transcendental evaluation.
 * Logical little-endian32-bit words; never a host compiler floating type.
 * This is not a guest instruction/SDK, x87 stack or execution-context ABI. */
typedef struct {
  uint32_t words[4];
} GamePkgF128;
typedef struct {
  GamePkgF128 value;
  uint32_t exceptions;
} GamePkgF128Result;
/*0:add,1:subtract,2:multiply,3:divide. rc0:nearest-even,1:down,2:up,3:zero.
 * IEEE raised flags use x87 IE/ZE/OE/UE/PE bits; no guest source DE decision.
 * Calls preserve every private SoftFloat control/flag and caller rounding mode.
 * Invalid parameters return-1 without changing result. Host-owned buffers only.
 */
int gamepkg_x86_f128_calculate(GamePkgF128 a, GamePkgF128 b, uint32_t op,
                               uint32_t rc, GamePkgF128Result *result);
int gamepkg_x86_f128_from_ext80(GamePkgExt80 input, GamePkgF128Result *result);
typedef struct {
  GamePkgExt80 value;
  uint32_t exceptions;
} GamePkgF128To80;
/* Numerical conversion only: no guest instruction commit/C1/wrapped-fault ABI.
 */
int gamepkg_x86_f128_to_ext80(GamePkgF128 input, uint32_t rc,
                              GamePkgF128To80 *result);
GamePkgF128 gamepkg_x86_f128_from_u32(uint32_t value);
#endif
