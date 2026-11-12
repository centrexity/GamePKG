#include "gamepkg_x86_f128.h"
#include "softfloat/platform.h"
#include "softfloat/source/include/primitiveTypes.h"
#include "softfloat/source/include/softfloat.h"
#include <string.h>
_Static_assert(sizeof(float128_t) == 16, "binary128 memory words");
_Static_assert(sizeof(uint32_t) == 4, "32-bit logical words");
typedef struct {
  uint_fast8_t round, tiny, flags, precision;
} Saved;
static Saved begin(unsigned rc) {
  Saved old = {softfloat_roundingMode, softfloat_detectTininess,
               softfloat_exceptionFlags, extF80_roundingPrecision};
  static const uint_fast8_t modes[] = {softfloat_round_near_even,
                                       softfloat_round_min, softfloat_round_max,
                                       softfloat_round_minMag};
  softfloat_roundingMode = modes[rc];
  softfloat_detectTininess = softfloat_tininess_afterRounding;
  softfloat_exceptionFlags = 0;
  extF80_roundingPrecision = 80;
  return old;
}
static void end(Saved old) {
  softfloat_roundingMode = old.round;
  softfloat_detectTininess = old.tiny;
  softfloat_exceptionFlags = old.flags;
  extF80_roundingPrecision = old.precision;
}
static float128_t unpack(GamePkgF128 v) {
  uint32_t words[4];
  for (unsigned i = 0; i < 4; ++i)
    words[indexWord(4, i)] = v.words[i];
  float128_t f;
  memcpy(&f, words, 16);
  return f;
}
static GamePkgF128 pack(float128_t f) {
  uint32_t words[4];
  memcpy(words, &f, 16);
  GamePkgF128 v;
  for (unsigned i = 0; i < 4; ++i)
    v.words[i] = words[indexWord(4, i)];
  return v;
}
static uint32_t raised(void) {
  unsigned v = softfloat_exceptionFlags;
  return ((v & softfloat_flag_invalid) ? 1u : 0u) |
         ((v & softfloat_flag_infinite) ? 4u : 0u) |
         ((v & softfloat_flag_overflow) ? 8u : 0u) |
         ((v & softfloat_flag_underflow) ? 16u : 0u) |
         ((v & softfloat_flag_inexact) ? 32u : 0u);
}
int gamepkg_x86_f128_calculate(GamePkgF128 a, GamePkgF128 b, uint32_t op,
                               uint32_t rc, GamePkgF128Result *r) {
  if (!r || op > 3 || rc > 3)
    return -1;
  Saved old = begin(rc);
  float128_t x = unpack(a), y = unpack(b), z;
  switch (op) {
  case 0:
    f128M_add(&x, &y, &z);
    break;
  case 1:
    f128M_sub(&x, &y, &z);
    break;
  case 2:
    f128M_mul(&x, &y, &z);
    break;
  default:
    f128M_div(&x, &y, &z);
    break;
  }
  GamePkgF128Result out = {pack(z), raised()};
  end(old);
  *r = out;
  return 0;
}
int gamepkg_x86_f128_from_ext80(GamePkgExt80 a, GamePkgF128Result *r) {
  if (!r)
    return -1;
  if ((a.sign_exponent & 0x7fff) && !(a.significand >> 63)) {
    *r = (GamePkgF128Result){{{0, 0, 0, 0xffff8000}}, 1};
    return 0;
  }
  Saved old = begin(0);
  extFloat80_t x = {.signif = a.significand, .signExp = a.sign_exponent};
  float128_t z;
  extF80M_to_f128M(&x, &z);
  GamePkgF128Result out = {pack(z), raised()};
  end(old);
  *r = out;
  return 0;
}
GamePkgF128 gamepkg_x86_f128_from_u32(uint32_t value) {
  Saved old = begin(0);
  float128_t out;
  ui32_to_f128M(value, &out);
  GamePkgF128 r = pack(out);
  end(old);
  return r;
}
int gamepkg_x86_f128_to_ext80(GamePkgF128 a, uint32_t rc, GamePkgF128To80 *r) {
  if (!r || rc > 3)
    return -1;
  Saved old = begin(rc);
  float128_t x = unpack(a);
  extFloat80_t z;
  f128M_to_extF80M(&x, &z);
  GamePkgF128To80 out = {{z.signif, z.signExp}, raised()};
  end(old);
  *r = out;
  return 0;
}
