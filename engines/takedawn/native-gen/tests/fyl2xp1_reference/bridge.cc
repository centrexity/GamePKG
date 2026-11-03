/* Maintainer-only pinned upstream numerical profile; no Bochs CPU emulation. */
#include "softfloat3e/include/softfloat.h"
extern extFloat80_t fyl2xp1(extFloat80_t, extFloat80_t, softfloat_status_t &);
extern "C" void gamepkg_test_fyl2xp1_reference(uint64_t sig, uint32_t se,
                                               uint64_t bsig, uint32_t bse,
                                               uint32_t cw, uint64_t *out_sig,
                                               uint32_t *out_se,
                                               uint32_t *out_up) {
  softfloat_status_t s = {};
  s.softfloat_exceptionMasks = (uint8_t)(cw & 63);
  s.softfloat_roundingMode = (uint8_t)((cw >> 10) & 3);
  s.extF80_roundingPrecision = 80;
  extFloat80_t a = packToExtF80((uint16_t)se, sig);
  auto r = fyl2xp1(a, packToExtF80((uint16_t)bse, bsig), s);
  *out_sig = r.signif;
  *out_se = r.signExp;
  *out_up = (s.softfloat_exceptionFlags >> 9) & 1;
}
