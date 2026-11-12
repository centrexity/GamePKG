#include <mpfr.h>
static _Thread_local unsigned accuracy_ready;
static _Thread_local mpfr_t ax, ay, az, ae, ap, aq, ar, at;
static void mp_raw(mpfr_t v, GamePkgExt80 a) {
  mpfr_set_uj(v, a.significand, MPFR_RNDN);
  unsigned e = a.sign_exponent & 0x7fff;
  mpfr_mul_2si(v, v, (long)(e ? e : 1) - 16383 - 63, MPFR_RNDN);
  if (a.sign_exponent & 0x8000)
    mpfr_neg(v, v, MPFR_RNDN);
}
/* The architectural legacy reduction uses a 66-bit pi/2, rather than true pi.
 * Check polynomial accuracy after that exact reduction, and also bound error
 * against true sin/cos by its independently measured argument-reduction drift.
 */
static void accuracy(GamePkgExt80 x, GamePkgExt80 output, unsigned cw,
                     unsigned sw, unsigned which) {
  if (!accuracy_ready) {
    mpfr_inits2(512, ax, ay, az, ae, ap, aq, ar, at, (mpfr_ptr)0);
    accuracy_ready = 1;
  }
  mp_raw(ax, x);
  mpfr_abs(ax, ax, MPFR_RNDN);
  mpfr_set_uj(ap, UINT64_C(0xc90fdaa22168c234), MPFR_RNDN);
  mpfr_mul_2si(ap, ap, -63, MPFR_RNDN);
  mpfr_set_uj(at, 3, MPFR_RNDN);
  mpfr_mul_2si(at, at, -65, MPFR_RNDN);
  mpfr_add(ap, ap, at, MPFR_RNDN);
  mpfr_div(aq, ax, ap, MPFR_RNDN);
  mpfr_floor(aq, aq);
  mpfr_mul(ar, aq, ap, MPFR_RNDN);
  mpfr_sub(ar, ax, ar, MPFR_RNDN);
  mpfr_div_2ui(at, ap, 1, MPFR_RNDN);
  if (mpfr_greater_p(ar, at)) {
    mpfr_add_ui(aq, aq, 1, MPFR_RNDN);
    mpfr_sub(ar, ar, ap, MPFR_RNDN);
  }
  unsigned n = ((unsigned)mpfr_get_uj(aq, MPFR_RNDN) + which) & 3;
  if (n & 1)
    mpfr_cos(ay, ar, MPFR_RNDN);
  else
    mpfr_sin(ay, ar, MPFR_RNDN);
  if (n & 2)
    mpfr_neg(ay, ay, MPFR_RNDN);
  if (!which && (x.sign_exponent & 0x8000))
    mpfr_neg(ay, ay, MPFR_RNDN);
  mp_raw(az, output);
  unsigned wrapped = (sw & ~cw & 16u) != 0;
  if (wrapped)
    mpfr_mul_2si(az, az, -24576, MPFR_RNDN);
  mpfr_sub(ae, az, ay, MPFR_RNDN);
  mpfr_abs(ae, ae, MPFR_RNDN);
  long k = mpfr_get_exp(ay) - 1;
  /* At extreme magnitudes MPFR512 can absorb a nonzero curvature term.
   * Recover the mathematical binade immediately below its limiting power. */
  if ((n & 1) && mpfr_cmpabs_ui(ay, 1) == 0 && !mpfr_zero_p(ar))
    --k;
  else if (!(n & 1) && mpfr_cmpabs(ay, ar) == 0 && mpfr_min_prec(ar) == 1)
    --k;
  if (!wrapped && k < -16382)
    k = -16382;
  mpfr_mul_2si(ae, ae, 63 - k, MPFR_RNDN);
  double limit = ((cw >> 10) & 3) ? 1.5 : 1.0;
  if (mpfr_cmp_d(ae, limit) >= 0) {
    mpfr_fprintf(
        stderr,
        "sincos accuracy op%u x%04x:%016llx CW%x SW%x error%.20Rg ulp\n", which,
        x.sign_exponent, (unsigned long long)x.significand, cw, sw, ae);
    abort();
  }
  /* sin and cos are 1-Lipschitz. q*|true_pi/2-legacy_pi/2| plus the strict
   * local rounding bound therefore bounds the full mathematical error. */
  mpfr_const_pi(at, MPFR_RNDN);
  mpfr_div_2ui(at, at, 1, MPFR_RNDN);
  mpfr_sub(at, at, ap, MPFR_RNDN);
  mpfr_abs(at, at, MPFR_RNDN);
  mpfr_mul(at, at, aq, MPFR_RNDN);
  mpfr_set_d(ae, limit, MPFR_RNDN);
  mpfr_mul_2si(ae, ae, k - 63, MPFR_RNDN);
  mpfr_add(at, at, ae, MPFR_RNDN);
  mp_raw(ax, x);
  if (which)
    mpfr_cos(ay, ax, MPFR_RNDN);
  else
    mpfr_sin(ay, ax, MPFR_RNDN);
  mpfr_sub(ae, az, ay, MPFR_RNDN);
  mpfr_abs(ae, ae, MPFR_RNDN);
  assert(mpfr_lessequal_p(ae, at));
}
static void accuracy_clear(void) {
  if (accuracy_ready) {
    mpfr_clears(ax, ay, az, ae, ap, aq, ar, at, (mpfr_ptr)0);
    accuracy_ready = 0;
  }
}
