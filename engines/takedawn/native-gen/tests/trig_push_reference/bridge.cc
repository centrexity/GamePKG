#include "softfloat3e/include/softfloat.h"
extern int fsincos(extFloat80_t, extFloat80_t *, extFloat80_t *,
                   softfloat_status_t &);
extern int fptan(extFloat80_t &, softfloat_status_t &);
extern "C" void gamepkg_test_trig_push_reference(uint64_t sig, uint32_t se,
                                                 uint32_t cw, uint32_t which,
                                                 uint64_t *as, uint32_t *ae,
                                                 uint64_t *bs, uint32_t *be,
                                                 uint32_t *up) {
  softfloat_status_t s = {};
  s.softfloat_exceptionMasks = cw & 63;
  s.softfloat_roundingMode = (cw >> 10) & 3;
  s.extF80_roundingPrecision = 80;
  auto a = packToExtF80((uint16_t)se, sig),
       b = packToExtF80(0x3fff, 0x8000000000000000ULL);
  if (which)
    fptan(a, s);
  else
    fsincos(a, &a, &b, s);
  *as = a.signif;
  *ae = a.signExp;
  *bs = b.signif;
  *be = b.signExp;
  *up = (s.softfloat_exceptionFlags >> 9) & 1;
  if (!which) {
    /* Hardware reports C1 for the sine lane. The original helper retains
     * cosine C1; independently re-evaluate its sine with truncation to observe
     * whether destination rounding increased the selected result magnitude. */
    softfloat_status_t t = {};
    t.softfloat_exceptionMasks = cw & 63;
    t.softfloat_roundingMode = 3;
    t.extF80_roundingPrecision = 80;
    auto z = packToExtF80((uint16_t)se, sig), c = z;
    fsincos(z, &z, &c, t);
    *up = (a.signExp & 0x7fff) > (z.signExp & 0x7fff) ||
          ((a.signExp & 0x7fff) == (z.signExp & 0x7fff) && a.signif > z.signif);
  }
}
