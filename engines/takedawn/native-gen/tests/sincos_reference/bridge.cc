#include "softfloat3e/include/softfloat.h"
extern int fsin(extFloat80_t &, softfloat_status_t &);
extern int fcos(extFloat80_t &, softfloat_status_t &);
extern "C" void gamepkg_test_sincos_reference(uint64_t sig, uint32_t se,
                                              uint32_t cw, uint32_t which,
                                              uint64_t *os, uint32_t *oe,
                                              uint32_t *up) {
  softfloat_status_t s = {};
  s.softfloat_exceptionMasks = cw & 63;
  s.softfloat_roundingMode = (cw >> 10) & 3;
  s.extF80_roundingPrecision = 80;
  auto a = packToExtF80((uint16_t)se, sig);
  if (which)
    fcos(a, s);
  else
    fsin(a, s);
  *os = a.signif;
  *oe = a.signExp;
  *up = (s.softfloat_exceptionFlags >> 9) & 1;
}
