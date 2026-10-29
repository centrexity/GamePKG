#ifdef GAMEPKG_F2XM1_MPFR
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
static void accuracy(GamePkgExt80 input, GamePkgExt80 output, unsigned cw,
                     unsigned sw) {
  if (!accuracy_ready) {
    mpfr_inits2(512, ax, aln, ay, az, ae, (mpfr_ptr)0);
    mpfr_const_log2(aln, MPFR_RNDN);
    accuracy_ready = 1;
  }
  mp_raw(ax, input);
  mpfr_mul(ay, ax, aln, MPFR_RNDN);
  mpfr_expm1(ay, ay, MPFR_RNDN);
  mp_raw(az, output);
  unsigned wrapped = (sw & 16u) && !(cw & 16u);
  if (wrapped)
    mpfr_mul_2si(az, az, -24576, MPFR_RNDN);
  mpfr_sub(ae, az, ay, MPFR_RNDN);
  mpfr_abs(ae, ae, MPFR_RNDN);
  long k = mpfr_get_exp(ay) - 1;
  if (!wrapped && k < -16382)
    k = -16382;
  mpfr_mul_2si(ae, ae, 63 - k, MPFR_RNDN);
  if (mpfr_cmp_d(ae, ((cw >> 10) & 3) ? 1.5 : 1.0) >= 0) {
    mpfr_fprintf(
        stderr,
        "F2XM1 accuracy failure input%04x:%016llx CW%x error%.20Rg ulp\n",
        input.sign_exponent, (unsigned long long)input.significand, cw, ae);
    abort();
  }
}
static void accuracy_clear(void) {
  if (accuracy_ready) {
    mpfr_clears(ax, aln, ay, az, ae, (mpfr_ptr)0);
    accuracy_ready = 0;
  }
}
#else
static void accuracy(GamePkgExt80 input, GamePkgExt80 output, unsigned cw,
                     unsigned sw) {
  (void)input;
  (void)output;
  (void)cw;
  (void)sw;
}
static void accuracy_clear(void) {}
#endif
