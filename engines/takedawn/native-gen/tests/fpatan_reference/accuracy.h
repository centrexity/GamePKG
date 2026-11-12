#ifdef GAMEPKG_FPATAN_MPFR
#include <mpfr.h>
static _Thread_local unsigned accuracy_ready;
static _Thread_local mpfr_t ax, aln, ay, az, ae;
static void mp_raw(mpfr_t v, GamePkgExt80 a) {
  mpfr_set_uj(v, a.significand, MPFR_RNDN);
  unsigned e = a.sign_exponent & 0x7fff;
  mpfr_mul_2si(v, v, (long)(e ? e : 1) - 16383 - 63, MPFR_RNDN);
  if (a.sign_exponent & 0x8000)
    mpfr_neg(v, v, MPFR_RNDN);
}
static void accuracy(GamePkgExt80 input, GamePkgExt80 multiplier,
                     GamePkgExt80 output, unsigned cw, unsigned sw) {
  if (!accuracy_ready) {
    mpfr_inits2(512, ax, aln, ay, az, ae, (mpfr_ptr)0);
    mpfr_const_log2(aln, MPFR_RNDN);
    accuracy_ready = 1;
  }
  mp_raw(ax, input);
  mp_raw(az, multiplier);
  mpfr_div(aln, az, ax, MPFR_RNDN);
  mpfr_atan2(ay, az, ax, MPFR_RNDN);
  mp_raw(az, output);
  unsigned wrapped = (sw & 24u) & ~cw;
  if ((output.sign_exponent & 0x7fff) == 0x7fff)
    return;
  if (wrapped)
    mpfr_mul_2si(az, az, (wrapped & 8u) ? 24576 : -24576, MPFR_RNDN);
  mpfr_sub(ae, az, ay, MPFR_RNDN);
  mpfr_abs(ae, ae, MPFR_RNDN);
  long k = mpfr_get_exp(ay) - 1;
  /* atan(q)<q for tiny positive q, even if MPFR512 absorbs its curvature. */
  if (!(input.sign_exponent & 0x8000) && mpfr_cmpabs(ay, aln) == 0 &&
      mpfr_min_prec(aln) == 1)
    --k;
  if (!wrapped && k < -16382)
    k = -16382;
  mpfr_mul_2si(ae, ae, 63 - k, MPFR_RNDN);
  if (mpfr_cmp_d(ae, ((cw >> 10) & 3) ? 1.5 : 1.0) >= 0) {
    mpfr_fprintf(stderr,
                 "FPATAN accuracy failure input%04x:%016llx y%04x:%016llx "
                 "output%04x:%016llx CW%x SW%x error%.20Rg ulp\n",
                 input.sign_exponent, (unsigned long long)input.significand,
                 multiplier.sign_exponent,
                 (unsigned long long)multiplier.significand,
                 output.sign_exponent, (unsigned long long)output.significand,
                 cw, sw, ae);
    abort();
  }
}
static void accuracy_clear(void) {
  if (accuracy_ready) {
    mpfr_clears(ax, aln, ay, az, ae, (mpfr_ptr)0);
    accuracy_ready = 0;
  }
}
/* Independent reference for exact_tiny()'s dyadic q, with <=64 significant
 * bits and |q|<2^-40. atan(q) is strictly below q in magnitude by less than
 * q^3/3, too little to cross a distinct raw80 rounding boundary. A 128-bit
 * predecessor supplies that strict lower tail, independently of the runtime's
 * binary128 helpers. Round with MPFR to 64 bits or the fixed subnormal grid.
 * This also derives UE, wrapped results and C1; Intel/AMD approximations can
 * disagree on all three at the minimum-normal boundary, not just C1.
 */
static GamePkgExt80 tiny_reference(GamePkgExt80 input, GamePkgExt80 multiplier,
                                    unsigned cw, unsigned old_status,
                                    unsigned *sw) {
  mpfr_t x, q, scaled, rounded;
  mpfr_inits2(128, x, q, scaled, (mpfr_ptr)0);
  mpfr_init2(rounded, 64);
  mp_raw(x, input);
  mp_raw(q, multiplier);
  assert(mpfr_div(q, q, x, MPFR_RNDN) == 0 && mpfr_min_prec(q) <= 64);
  unsigned sign = mpfr_signbit(q) != 0, rc = (cw >> 10) & 3;
  mpfr_abs(q, q, MPFR_RNDN);
  mpfr_nextbelow(q);
  mpfr_rnd_t mode = rc == 0 ? MPFR_RNDN
                     : ((rc == 1 && sign) || (rc == 2 && !sign))
                         ? MPFR_RNDU : MPFR_RNDD;
  mpfr_set(rounded, q, mode);
  unsigned underflow = 0;
  GamePkgExt80 value;
  if (mpfr_get_exp(rounded) < -16381 && (cw & 16u)) {
    mpfr_mul_2si(scaled, q, 16445, MPFR_RNDN);
    value.significand = (uint64_t)mpfr_get_uj(scaled, mode);
    value.sign_exponent = (uint16_t)((sign << 15) |
                                        (value.significand >> 63));
    /* Tininess follows the rounded unbounded 64-bit result. Rounding again
     * on the subnormal grid can carry into minimum normal without clearing UE. */
    underflow = 1;
    mp_raw(scaled, value);
    mpfr_abs(scaled, scaled, MPFR_RNDN);
  } else {
    underflow = mpfr_get_exp(rounded) < -16381;
    mpfr_exp_t exponent = mpfr_get_exp(rounded);
    mpfr_mul_2si(scaled, rounded, 64 - exponent, MPFR_RNDN);
    value.significand = (uint64_t)mpfr_get_uj(scaled, MPFR_RNDN);
    value.sign_exponent = (uint16_t)((sign << 15) |
                           (unsigned)(exponent + 16382 +
                                      (underflow ? 24576 : 0)));
    mpfr_set(scaled, rounded, MPFR_RNDN);
  }
  unsigned up = mpfr_cmp(scaled, q) > 0;
  assert(*sw & 32u); /* Every committed nonzero finite arctangent is inexact. */
  *sw = (*sw & ~(16u | 0x200u | 0x8080u)) | (old_status & 16u) |
          (underflow ? 16u : 0u) | (up << 9);
  if (*sw & ~cw & 63u)
    *sw |= 0x8080u;
  mpfr_clears(x, q, scaled, rounded, (mpfr_ptr)0);
  return value;
}
#else
static void accuracy(GamePkgExt80 input, GamePkgExt80 multiplier,
                     GamePkgExt80 output, unsigned cw, unsigned sw) {
  (void)input;
  (void)multiplier;
  (void)output;
  (void)cw;
  (void)sw;
}
static void accuracy_clear(void) {}
#endif
