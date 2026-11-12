#ifndef GAMEPKG_X86_X87_DECODE_H
#define GAMEPKG_X86_X87_DECODE_H
#include "gamepkg_x86_ext80.h"
/* Exact bit decoding. Arithmetic must retain signaling NaNs until the binary
 * operation selects its result; FLD quiets them at the load boundary instead.
 * Normalizing an IEEE subnormal loses its source classification, so report DE
 * separately for the caller's instruction-specific exception priority. */
static inline GamePkgExt80
gamepkg_x87_decode_float(uint64_t bits, unsigned fraction_bits,
                         unsigned exponent_bits, unsigned bias, unsigned quiet,
                         unsigned *raised) {
  uint64_t fraction_mask = (UINT64_C(1) << fraction_bits) - 1;
  uint64_t frac = bits & fraction_mask;
  unsigned exponent =
      (unsigned)(bits >> fraction_bits) & ((1u << exponent_bits) - 1);
  unsigned sign = (unsigned)(bits >> (fraction_bits + exponent_bits));
  GamePkgExt80 value = {0, (uint16_t)(sign << 15)};
  if (exponent == ((1u << exponent_bits) - 1)) {
    value.sign_exponent |= 0x7fff;
    value.significand =
        UINT64_C(0x8000000000000000) | (frac << (63 - fraction_bits));
    if (frac && !(frac & (UINT64_C(1) << (fraction_bits - 1)))) {
      *raised = 1;
      if (quiet)
        value.significand |= UINT64_C(0x4000000000000000);
    }
  } else if (exponent || frac) {
    int power = exponent ? (int)exponent - (int)bias : 1 - (int)bias;
    if (exponent)
      frac |= UINT64_C(1) << fraction_bits;
    else {
      *raised = 2;
      while (!(frac & (UINT64_C(1) << fraction_bits))) {
        frac <<= 1;
        --power;
      }
    }
    value.significand = frac << (63 - fraction_bits);
    value.sign_exponent |= (uint16_t)(power + 0x3fff);
  }
  return value;
}
static inline GamePkgExt80 gamepkg_x87_decode_integer(uint64_t bits,
                                                      unsigned width) {
  uint64_t mask = width == 64 ? UINT64_MAX : (UINT64_C(1) << width) - 1;
  bits &= mask;
  unsigned sign = (unsigned)(bits >> (width - 1));
  uint64_t magnitude = sign ? (0u - bits) & mask : bits;
  if (!magnitude)
    return (GamePkgExt80){0, 0};
  unsigned top = 0;
  uint64_t scan = magnitude;
  while (scan >> 1) {
    scan >>= 1;
    ++top;
  }
  return (GamePkgExt80){magnitude << (63 - top),
                        (uint16_t)((sign << 15) | (0x3fff + top))};
}
#endif
