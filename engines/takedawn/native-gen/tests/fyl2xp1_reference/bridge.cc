/* Maintainer-only independent numerical reference; no Bochs CPU emulation.
 * Exact log2(1+x) identities use upstream binary128 integer multiplication,
 * independently of production helpers. They lie outside Intel's guaranteed
 * FYL2XP1 input interval, where host approximation, range flags and C1 are
 * not portable. Their result and exception flags come from this reference. */
#include "softfloat3e/include/softfloat.h"
extern extFloat80_t fyl2xp1(extFloat80_t, extFloat80_t, softfloat_status_t &);
static int exact_logarithm(uint64_t sig, uint32_t se) {
  unsigned e = se & 0x7fff;
  if ((se & 0x8000) && e == 16382) {
    uint64_t inverse = ~sig;
    if (!(inverse & (inverse + 1))) {
      int n = 0;
      for (uint64_t bit = UINT64_C(0x8000000000000000); sig & bit; bit >>= 1)
        ++n;
      return -n;
    }
  } else if (!(se & 0x8000) && e >= 16383 && e <= 16446) {
    unsigned n = e - 16383 + 1;
    if (sig == (UINT64_MAX << (64 - n)))
      return (int)n;
  }
  return 0;
}
static bool exact_product(uint64_t sig, uint32_t se, uint64_t bsig,
                          uint32_t bse, softfloat_status_t &s,
                          extFloat80_t &r) {
  int n = exact_logarithm(sig, se);
  if (n && (bse & 0x7fff) < 0x7fff && bsig &&
      (!(bse & 0x7fff) || (bsig >> 63))) {
    if (!(bse & 0x7fff))
      s.softfloat_exceptionFlags |= softfloat_flag_denormal;
    int32_t bexp = (bse & 0x7fff) ? (bse & 0x7fff) : 1;
    while (!(bsig >> 63)) {
      bsig <<= 1;
      --bexp;
    }
    // At most seven integer bits times 64 significand bits is exact in
    // binary128. Carry the multiplier's exponent to the final conversion.
    auto product = f128_mul_by_extF80(
        i32_to_f128(n), packToExtF80((bse & 0x8000) | 16383, bsig),
        s.softfloat_roundingMode, &s);
    r = f128_to_extF80(product, bexp - 16383, s.softfloat_roundingMode, &s);
    s.softfloat_exceptionFlags |= softfloat_flag_inexact;
    return true;
  }
  return false;
}
static softfloat_status_t status(uint32_t cw) {
  softfloat_status_t s = {};
  s.softfloat_exceptionMasks = (uint8_t)(cw & 63);
  s.softfloat_roundingMode = (uint8_t)((cw >> 10) & 3);
  s.extF80_roundingPrecision = 80;
  return s;
}
extern "C" int gamepkg_test_fyl2xp1_exact_reference(
    uint64_t sig, uint32_t se, uint64_t bsig, uint32_t bse, uint32_t cw,
    uint64_t *out_sig, uint32_t *out_se, uint32_t *out_flags) {
  auto s = status(cw);
  extFloat80_t r;
  if (!exact_product(sig, se, bsig, bse, s, r))
    return 0;
  *out_sig = r.signif;
  *out_se = r.signExp;
  *out_flags = s.softfloat_exceptionFlags;
  return 1;
}
extern "C" void gamepkg_test_fyl2xp1_reference(uint64_t sig, uint32_t se,
                                               uint64_t bsig, uint32_t bse,
                                               uint32_t cw, uint64_t *out_sig,
                                               uint32_t *out_se,
                                               uint32_t *out_up) {
  auto s = status(cw);
  extFloat80_t r;
  if (!exact_product(sig, se, bsig, bse, s, r)) {
    r = fyl2xp1(packToExtF80((uint16_t)se, sig),
                packToExtF80((uint16_t)bse, bsig), s);
  }
  *out_sig = r.signif;
  *out_se = r.signExp;
  *out_up = (s.softfloat_exceptionFlags >> 9) & 1;
}
