/* Maintainer-only independent numerical reference; no Bochs CPU emulation.
 * Ordinary logarithms use the pinned upstream approximation. Powers of two
 * use the runtime's documented exact-logarithm contract, independently formed
 * with upstream binary128 arithmetic rather than the production helpers. */
#include "softfloat3e/include/softfloat.h"
extern extFloat80_t fyl2x(extFloat80_t, extFloat80_t, softfloat_status_t &);
extern "C" void gamepkg_test_fyl2x_reference(uint64_t sig, uint32_t se,
                                             uint64_t bsig, uint32_t bse,
                                             uint32_t cw, uint64_t *out_sig,
                                             uint32_t *out_se,
                                             uint32_t *out_up) {
  softfloat_status_t s = {};
  s.softfloat_exceptionMasks = (uint8_t)(cw & 63);
  s.softfloat_roundingMode = (uint8_t)((cw >> 10) & 3);
  s.extF80_roundingPrecision = 80;
  extFloat80_t a = packToExtF80((uint16_t)se, sig);
  extFloat80_t r;
  if (!(se & 0x8000) && (se & 0x7fff) < 0x7fff && sig &&
      !(sig & (sig - 1)) && (bse & 0x7fff) < 0x7fff && bsig) {
    int32_t exponent = (se & 0x7fff) ? (se & 0x7fff) : 1;
    while (!(sig >> 63)) {
      sig <<= 1;
      --exponent;
    }
    int32_t bexp = (bse & 0x7fff) ? (bse & 0x7fff) : 1;
    while (!(bsig >> 63)) {
      bsig <<= 1;
      --bexp;
    }
    // log2(x) is an integer of at most 15 bits. Its product with the
    // 64-bit significand is exact in binary128; only the final conversion
    // rounds. Carry y's exponent separately to avoid intermediate range loss.
    auto logarithm = i32_to_f128(exponent - 16383);
    auto product = f128_mul_by_extF80(
        logarithm, packToExtF80((bse & 0x8000) | 16383, bsig),
        s.softfloat_roundingMode, &s);
    r = f128_to_extF80(product, bexp - 16383, s.softfloat_roundingMode, &s);
  } else {
    r = fyl2x(a, packToExtF80((uint16_t)bse, bsig), s);
  }
  *out_sig = r.signif;
  *out_se = r.signExp;
  *out_up = (s.softfloat_exceptionFlags >> 9) & 1;
}
