#include "gamepkg_x86_ext80.h"
#include "softfloat/platform.h"
#include "softfloat/source/include/internals.h"
#include "softfloat/source/include/softfloat.h"
typedef struct Observation {
  uint32_t words[3];
  int32_t exponent;
  uint32_t sign, valid, enabled;
} Observation;
static _Thread_local Observation observation;
void gamepkg_softfloat_ext80_observe(uint32_t sign, int32_t exponent,
                                     const uint32_t *words) {
  if (!observation.enabled)
    return;
  for (unsigned i = 0; i < 3; ++i)
    observation.words[i] = words[i];
  observation.sign = sign;
  observation.exponent = exponent;
  observation.valid = 1;
}
static GamePkgExt80 reround(const Observation *raw, int32_t exponent,
                            uint_fast8_t rounding, uint32_t *inexact) {
  uint32_t words[3];
  for (unsigned i = 0; i < 3; ++i)
    words[i] = raw->words[i];
  struct extFloat80M out;
  softfloat_exceptionFlags = 0;
  softfloat_roundingMode = rounding;
  softfloat_roundPackMToExtF80M(raw->sign, exponent, words,
                                extF80_roundingPrecision, &out);
  *inexact = (softfloat_exceptionFlags & softfloat_flag_inexact) != 0;
  return (GamePkgExt80){out.signif, out.signExp};
}
static uint32_t greater_magnitude(GamePkgExt80 a, GamePkgExt80 b) {
  unsigned ae = a.sign_exponent & 0x7fff, be = b.sign_exponent & 0x7fff;
  return ae > be || (ae == be && a.significand > b.significand);
}
static int unsupported(GamePkgExt80 value) {
  return (value.sign_exponent & 0x7fff) && !(value.significand >> 63);
}
static int ext_nan(GamePkgExt80 value) {
  return (value.sign_exponent & 0x7fff) == 0x7fff &&
         (value.significand & UINT64_C(0x7fffffffffffffff));
}
static int denormal(GamePkgExt80 value) {
  return !(value.sign_exponent & 0x7fff) && value.significand;
}
int gamepkg_x86_ext80_calculate(GamePkgExt80 left, GamePkgExt80 right,
                                uint32_t operation, uint32_t control,
                                GamePkgExt80Result *result) {
  uint32_t pc = (control >> 8u) & 3u;
  if (!result || operation > 4)
    return -1;
  GamePkgExt80Result staged = {0};
  Observation old_observation = observation;
  if (unsupported(right) || (operation != 4 && unsupported(left))) {
    staged.value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
    staged.exceptions = 1;
    *result = staged;
    return 0;
  }
  uint_fast8_t old_round = softfloat_roundingMode,
               old_tiny = softfloat_detectTininess;
  uint_fast8_t old_flags = softfloat_exceptionFlags,
               old_precision = extF80_roundingPrecision;
  static const uint_fast8_t rounds[] = {
      softfloat_round_near_even, softfloat_round_min, softfloat_round_max,
      softfloat_round_minMag};
  softfloat_roundingMode = rounds[(control >> 10u) & 3u];
  softfloat_detectTininess = softfloat_tininess_afterRounding;
  extF80_roundingPrecision = pc == 0 ? 32 : pc == 2 ? 64 : 80;
  softfloat_exceptionFlags = 0;
  observation = (Observation){.enabled = 1};
  extFloat80_t a = {.signif = left.significand, .signExp = left.sign_exponent};
  extFloat80_t b = {.signif = right.significand,
                    .signExp = right.sign_exponent},
               out;
  switch (operation) {
  case 0:
    extF80M_add(&a, &b, &out);
    break;
  case 1:
    extF80M_sub(&a, &b, &out);
    break;
  case 2:
    extF80M_mul(&a, &b, &out);
    break;
  case 3:
    extF80M_div(&a, &b, &out);
    break;
  default:
    extF80M_sqrt(&b, &out);
    break;
  }
  uint_fast8_t raised = softfloat_exceptionFlags;
  staged.value = (GamePkgExt80){out.signif, out.signExp};
  staged.exceptions = ((raised & softfloat_flag_invalid) ? 1u : 0u) |
                      ((raised & softfloat_flag_infinite) ? 4u : 0u) |
                      ((raised & softfloat_flag_overflow) ? 8u : 0u) |
                      ((raised & softfloat_flag_underflow) ? 16u : 0u) |
                      ((raised & softfloat_flag_inexact) ? 32u : 0u);
  /* NaNs/invalid and zero-division precede denormal handling. The hardware
   * corpus tests the priority and pseudo-denormal behavior independently. */
  if (!(staged.exceptions & 5u) && !ext_nan(right) &&
      (operation == 4 || !ext_nan(left)) &&
      (denormal(right) || (operation != 4 && denormal(left))))
    staged.exceptions |= 2u;
  staged.unbounded_inexact = (staged.exceptions & 32u) != 0;
  Observation raw = observation;
  observation.enabled = 0;
  if (raw.valid) {
    uint32_t unused;
    if (staged.exceptions & 32u) {
      GamePkgExt80 truncated =
          reround(&raw, raw.exponent, softfloat_round_minMag, &unused);
      staged.rounded_up = greater_magnitude(staged.value, truncated);
    }
    if ((staged.exceptions & 8u) || raw.exponent <= 0) {
      int32_t adjusted =
          raw.exponent + ((staged.exceptions & 8u) ? -24576 : 24576);
      staged.wrapped = reround(&raw, adjusted, rounds[(control >> 10u) & 3u],
                               &staged.unbounded_inexact);
      GamePkgExt80 truncated =
          reround(&raw, adjusted, softfloat_round_minMag, &unused);
      staged.wrapped_rounded_up = greater_magnitude(staged.wrapped, truncated);
      if (staged.exceptions & 8u)
        staged.range = 8;
      else if ((staged.wrapped.sign_exponent & 0x7fff) <= 24576u)
        staged.range = 16;
    }
  }
  observation = old_observation;
  softfloat_roundingMode = old_round;
  softfloat_detectTininess = old_tiny;
  softfloat_exceptionFlags = old_flags;
  extF80_roundingPrecision = old_precision;
  *result = staged;
  return 0;
}

/* FSCALE is exact exponent addition before final extended-range rounding.
 * Its scale operand truncates toward zero independently of RC; PC is ignored.
 * A finite scale with magnitude >=65536 cannot be rescued by one architectural
 * exponent wrap, so saturation at that bound preserves numerical responses. */
int gamepkg_x86_ext80_scale(GamePkgExt80 a, GamePkgExt80 b, uint32_t control,
                            GamePkgExt80Result *result) {
  if (!result)
    return -1;
  if (unsupported(a) || unsupported(b) || ext_nan(a) || ext_nan(b))
    return gamepkg_x86_ext80_calculate(a, b, 0, (control & ~0x300u) | 0x300u,
                                       result);
  GamePkgExt80Result z = {0};
  unsigned ae = a.sign_exponent & 0x7fff, be = b.sign_exponent & 0x7fff;
  unsigned az = !ae && !a.significand, ai = ae == 0x7fff, bi = be == 0x7fff;
  z.exceptions = (denormal(a) || denormal(b)) ? 2u : 0;
  if (bi) {
    unsigned negative = b.sign_exponent >> 15;
    if ((az && !negative) || (ai && negative)) {
      z.value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
      z.exceptions = 1;
    } else
      z.value = (GamePkgExt80){
          negative ? 0 : UINT64_C(0x8000000000000000),
          (uint16_t)((a.sign_exponent & 0x8000) | (negative ? 0 : 0x7fff))};
    *result = z;
    return 0;
  }
  if (az || ai) {
    z.value = a;
    *result = z;
    return 0;
  }
  int32_t e = (int32_t)be - 16383, scale = 0;
  if (e >= 16)
    scale = 65536;
  else if (e >= 0)
    scale = (int32_t)(b.significand >> (63 - (unsigned)e));
  if (b.sign_exponent & 0x8000)
    scale = -scale;
  int32_t exponent = ae ? (int32_t)ae : 1;
  uint64_t sig = a.significand;
  while (!(sig >> 63)) {
    sig <<= 1;
    --exponent;
  }
  Observation raw = {
      .exponent = exponent + scale, .sign = a.sign_exponent >> 15, .valid = 1};
  raw.words[indexWord(3, 2)] = (uint32_t)(sig >> 32);
  raw.words[indexWord(3, 1)] = (uint32_t)sig;
  Observation old_observation = observation;
  observation.enabled = 0;
  uint_fast8_t old_round = softfloat_roundingMode,
               old_tiny = softfloat_detectTininess;
  uint_fast8_t old_flags = softfloat_exceptionFlags,
               old_precision = extF80_roundingPrecision;
  static const uint_fast8_t rounds[] = {
      softfloat_round_near_even, softfloat_round_min, softfloat_round_max,
      softfloat_round_minMag};
  uint_fast8_t rc = rounds[(control >> 10) & 3];
  softfloat_detectTininess = softfloat_tininess_afterRounding;
  extF80_roundingPrecision = 80;
  uint32_t unused;
  z.value = reround(&raw, raw.exponent, rc, &unused);
  unsigned raised = softfloat_exceptionFlags;
  z.exceptions |= ((raised & softfloat_flag_overflow) ? 8u : 0u) |
                  ((raised & softfloat_flag_underflow) ? 16u : 0u) |
                  ((raised & softfloat_flag_inexact) ? 32u : 0u);
  z.unbounded_inexact = (z.exceptions & 32u) != 0;
  if (z.exceptions & 32u) {
    GamePkgExt80 trunc =
        reround(&raw, raw.exponent, softfloat_round_minMag, &unused);
    z.rounded_up = greater_magnitude(z.value, trunc);
  }
  if ((z.exceptions & 8u) || raw.exponent <= 0) {
    int32_t adjusted = raw.exponent + ((z.exceptions & 8u) ? -24576 : 24576);
    z.wrapped = reround(&raw, adjusted, rc, &z.unbounded_inexact);
    GamePkgExt80 trunc =
        reround(&raw, adjusted, softfloat_round_minMag, &unused);
    z.wrapped_rounded_up = greater_magnitude(z.wrapped, trunc);
    /* A second range failure after the single 24576-exponent response
     * commits signed infinity/zero with PE, independently of RC. It is
     * not another denormalizing or directed-rounding arithmetic step. */
    if (adjusted > 0x7ffe || adjusted <= 0) {
      unsigned overflow = adjusted > 0x7ffe;
      z.wrapped = (GamePkgExt80){
          overflow ? UINT64_C(0x8000000000000000) : 0,
          (uint16_t)((a.sign_exponent & 0x8000) | (overflow ? 0x7fff : 0))};
      z.unbounded_inexact = 1;
      z.wrapped_rounded_up = overflow;
    }
    if (z.exceptions & 8u)
      z.range = 8;
    else if ((z.wrapped.sign_exponent & 0x7fff) <= 24576u)
      z.range = 16;
  }
  observation = old_observation;
  softfloat_roundingMode = old_round;
  softfloat_detectTininess = old_tiny;
  softfloat_exceptionFlags = old_flags;
  extF80_roundingPrecision = old_precision;
  *result = z;
  return 0;
}

/* One architectural partial-remainder step. The 32+(D mod32) reduction profile
 * is within Intel's implementation-dependent32..63-bit step specification.
 * All division is unsigned64 shift/subtract; quotient flags use magnitude. */
int gamepkg_x86_ext80_remainder(GamePkgExt80 a, GamePkgExt80 b,
                                uint32_t nearest, GamePkgExt80Result *result,
                                uint32_t *condition) {
  if (!result || !condition || nearest > 1)
    return -1;
  GamePkgExt80Result z = {0};
  *condition = 0;
  if (unsupported(a) || unsupported(b) || ext_nan(a) || ext_nan(b))
    return gamepkg_x86_ext80_calculate(a, b, 0, 0x37f, result);
  unsigned ae = a.sign_exponent & 0x7fff, be = b.sign_exponent & 0x7fff;
  if (ae == 0x7fff || (!be && !b.significand)) {
    z.value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
    z.exceptions = 1;
    *result = z;
    return 0;
  }
  z.exceptions = (denormal(a) || denormal(b)) ? 2u : 0;
  if (!ae && !a.significand) {
    z.value = a;
    *result = z;
    return 0;
  }
  int32_t ea = ae ? (int32_t)ae : 1, eb = be ? (int32_t)be : 1;
  uint64_t x = a.significand, y = b.significand;
  while (!(x >> 63)) {
    x <<= 1;
    --ea;
  }
  if (be == 0x7fff) {
    z.value = (GamePkgExt80){x, (uint16_t)((a.sign_exponent & 0x8000) |
                                           (ea > 0 ? (unsigned)ea : 0))};
    if (ea <= 0) {
      z.value = a;
      z.range = 16;
      z.wrapped = (GamePkgExt80){
          x, (uint16_t)((a.sign_exponent & 0x8000) | (unsigned)(ea + 24576))};
    }
    *result = z;
    return 0;
  }
  while (!(y >> 63)) {
    y <<= 1;
    --eb;
  }
  int32_t d = ea - eb, exponent = ea;
  unsigned q = 0, flip = 0;
  uint64_t r = x;
  if (d < 0) {
    if (nearest && d == -1 && x > y) {
      r = y - (x - y);
      q = 1;
      flip = 1;
    }
  } else {
    unsigned partial = d >= 64,
             steps = partial ? 32u + ((unsigned)d & 31u) : (unsigned)d;
    if (r >= y) {
      r -= y;
      q = 1;
    }
    for (unsigned i = 0; i < steps; ++i) {
      unsigned carry = (unsigned)(r >> 63);
      r <<= 1;
      unsigned bit = carry || r >= y;
      if (bit)
        r -= y;
      q = ((q << 1) | bit) & 7u;
    }
    exponent = eb + (partial ? d - (int32_t)steps : 0);
    if (partial)
      *condition = 0x400;
    else if (nearest) {
      unsigned carry = (unsigned)(r >> 63);
      uint64_t twice = r << 1;
      if (carry || twice > y || (twice == y && (q & 1))) {
        r = y - r;
        q = (q + 1) & 7;
        flip = 1;
      }
    }
  }
  if (d < 64)
    *condition = ((q & 4) ? 0x100u : 0u) | ((q & 2) ? 0x4000u : 0u) |
                 ((q & 1) ? 0x200u : 0u);
  unsigned sign = (a.sign_exponent & 0x8000) ^ (flip ? 0x8000 : 0);
  if (!r) {
    z.value = (GamePkgExt80){0, (uint16_t)(a.sign_exponent & 0x8000)};
    *result = z;
    return 0;
  }
  while (!(r >> 63)) {
    r <<= 1;
    --exponent;
  }
  if (exponent <= 0) {
    unsigned shift = (unsigned)(1 - exponent);
    z.value = (GamePkgExt80){shift < 64 ? r >> shift : 0, (uint16_t)sign};
    z.range = 16;
    z.wrapped =
        (GamePkgExt80){r, (uint16_t)(sign | (unsigned)(exponent + 24576))};
  } else
    z.value = (GamePkgExt80){r, (uint16_t)(sign | (unsigned)exponent)};
  *result = z;
  return 0;
}

int gamepkg_x86_ext80_round_f128(const uint32_t words[4], int32_t adjust,
                                 uint32_t control, GamePkgExt80Result *result) {
  if (!words || !result || adjust < -65536 || adjust > 65536)
    return -1;
  uint32_t sig[4] = {words[0], words[1], words[2], words[3] & 0xffffu};
  unsigned sign = words[3] >> 31, exponent = (words[3] >> 16) & 0x7fff;
  GamePkgExt80Result z = {0};
  if (exponent == 0x7fff) {
    unsigned nan = (sig[0] | sig[1] | sig[2] | sig[3]) != 0;
    uint64_t payload =
        ((uint64_t)sig[3] << 47) | ((uint64_t)sig[2] << 15) | (sig[1] >> 17);
    z.value = (GamePkgExt80){UINT64_C(0x8000000000000000) | payload |
                                 (nan ? UINT64_C(0x4000000000000000) : 0),
                             (uint16_t)((sign << 15) | 0x7fff)};
    z.exceptions = nan && !(sig[3] & 0x8000) ? 1 : 0;
    *result = z;
    return 0;
  }
  int32_t exp = exponent ? (int32_t)exponent : 1;
  if (exponent)
    sig[3] |= 0x10000;
  else if (!(sig[0] | sig[1] | sig[2] | sig[3])) {
    z.value = (GamePkgExt80){0, (uint16_t)(sign << 15)};
    *result = z;
    return 0;
  }
  while (!(sig[3] & 0x10000u)) {
    unsigned carry = 0;
    for (unsigned i = 0; i < 4; ++i) {
      unsigned next = sig[i] >> 31;
      sig[i] = (sig[i] << 1) | carry;
      carry = next;
    }
    --exp;
  }
  Observation raw = {.exponent = exp + adjust, .sign = sign, .valid = 1};
  raw.words[indexWord(3, 2)] = (sig[3] << 15) | (sig[2] >> 17);
  raw.words[indexWord(3, 1)] = (sig[2] << 15) | (sig[1] >> 17);
  raw.words[indexWord(3, 0)] =
      (sig[1] << 15) | (sig[0] >> 17) | ((sig[0] & 0x1ffffu) != 0);
  Observation old_observation = observation;
  observation.enabled = 0;
  uint_fast8_t old_round = softfloat_roundingMode,
               old_tiny = softfloat_detectTininess,
               old_flags = softfloat_exceptionFlags,
               old_precision = extF80_roundingPrecision;
  static const uint_fast8_t rounds[] = {
      softfloat_round_near_even, softfloat_round_min, softfloat_round_max,
      softfloat_round_minMag};
  uint_fast8_t rc = rounds[(control >> 10) & 3];
  softfloat_detectTininess = softfloat_tininess_afterRounding;
  extF80_roundingPrecision = 80;
  uint32_t unused;
  z.value = reround(&raw, raw.exponent, rc, &unused);
  unsigned raised = softfloat_exceptionFlags;
  z.exceptions = ((raised & softfloat_flag_overflow) ? 8u : 0u) |
                 ((raised & softfloat_flag_underflow) ? 16u : 0u) |
                 ((raised & softfloat_flag_inexact) ? 32u : 0u);
  z.unbounded_inexact = (z.exceptions & 32) != 0;
  if (z.exceptions & 32u) {
    GamePkgExt80 trunc =
        reround(&raw, raw.exponent, softfloat_round_minMag, &unused);
    z.rounded_up = greater_magnitude(z.value, trunc);
  }
  if ((z.exceptions & 8u) || raw.exponent <= 0) {
    int32_t adjusted = raw.exponent + ((z.exceptions & 8u) ? -24576 : 24576);
    z.wrapped = reround(&raw, adjusted, rc, &z.unbounded_inexact);
    GamePkgExt80 trunc =
        reround(&raw, adjusted, softfloat_round_minMag, &unused);
    z.wrapped_rounded_up = greater_magnitude(z.wrapped, trunc);
    if (z.exceptions & 8u)
      z.range = 8;
    else if ((z.wrapped.sign_exponent & 0x7fff) <= 24576u)
      z.range = 16;
  }
  observation = old_observation;
  softfloat_roundingMode = old_round;
  softfloat_detectTininess = old_tiny;
  softfloat_exceptionFlags = old_flags;
  extF80_roundingPrecision = old_precision;
  *result = z;
  return 0;
}
