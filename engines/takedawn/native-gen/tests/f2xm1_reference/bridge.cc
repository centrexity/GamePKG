/* Maintainer-only pinned upstream numerical profile; no Bochs CPU emulation. */
#include "softfloat3e/include/softfloat.h"
extern extFloat80_t f2xm1(extFloat80_t, softfloat_status_t &);
extern "C" void gamepkg_test_f2xm1_reference(uint64_t sig, uint32_t se,
                                             uint32_t cw, uint64_t *out_sig,
                                             uint32_t *out_se,
                                             uint32_t *out_up) {
  softfloat_status_t s = {};
  s.softfloat_exceptionMasks = (uint8_t)(cw & 63);
  s.softfloat_roundingMode = (uint8_t)((cw >> 10) & 3);
  s.extF80_roundingPrecision = 80;
  extFloat80_t a = packToExtF80((uint16_t)se, sig);
  auto r = f2xm1(a, s);
  *out_sig = r.signif;
  *out_se = r.signExp;
  *out_up = (s.softfloat_exceptionFlags >> 9) & 1;
}
