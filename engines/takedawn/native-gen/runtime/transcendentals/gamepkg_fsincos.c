/* SPDX-License-Identifier: LGPL-2.1-or-later
 * Numerical evaluation adapted from Bochs fsincos.cc. Original sources,
 * revision, notices and license are retained under upstream/. This C adapter
 * uses GamePKG's reviewed multiword helpers instead of the Bochs CPU/status
 * ABI.
 */
/*============================================================================
This source file is an extension to the SoftFloat IEC/IEEE Floating-point
Arithmetic Package, Release 2b, written for Bochs (x86 achitecture simulator)
floating point emulation.

THIS SOFTWARE IS DISTRIBUTED AS IS, FOR FREE.  Although reasonable effort has
been made to avoid it, THIS SOFTWARE MAY CONTAIN FAULTS THAT WILL AT TIMES
RESULT IN INCORRECT BEHAVIOR.  USE OF THIS SOFTWARE IS RESTRICTED TO PERSONS
AND ORGANIZATIONS WHO CAN AND WILL TAKE FULL RESPONSIBILITY FOR ALL LOSSES,
COSTS, OR OTHER PROBLEMS THEY INCUR DUE TO THE SOFTWARE, AND WHO FURTHERMORE
EFFECTIVELY INDEMNIFY JOHN HAUSER AND THE INTERNATIONAL COMPUTER SCIENCE
INSTITUTE (possibly via similar legal warning) AGAINST ALL LOSSES, COSTS, OR
OTHER PROBLEMS INCURRED BY THEIR CUSTOMERS AND CLIENTS DUE TO THE SOFTWARE.

Derivative works are acceptable, even for commercial purposes, so long as
(1) the source code for the derivative work includes prominent notice that
the work is derivative, and (2) the source code includes prominent notice with
these four paragraphs for those parts of this code that are retained.
=============================================================================*/

#include "gamepkg_fsincos.h"
#include <stdlib.h>
#define BX_CONST64 UINT64_C
#define FLOATX80_EXP_BIAS 16383
#define SC_SIGNBIT UINT64_C(0x8000000000000000)
typedef uint64_t Bit64u;
typedef int32_t Bit32s;
typedef struct {
  uint64_t v64, v0;
} Wide;
#define packFloat128(H, L) ((Wide){(H), (L)})
#include "upstream/fsincos_constants.h"
static GamePkgF128 logical(Wide a) {
  return (GamePkgF128){{(uint32_t)a.v0, (uint32_t)(a.v0 >> 32), (uint32_t)a.v64,
                        (uint32_t)(a.v64 >> 32)}};
}
static Wide wide(GamePkgF128 a) {
  return (Wide){(uint64_t)a.words[3] << 32 | a.words[2],
                (uint64_t)a.words[1] << 32 | a.words[0]};
}
static Wide operation(Wide a, Wide b, unsigned op, unsigned rc) {
  GamePkgF128Result r;
  if (gamepkg_x86_f128_calculate(logical(a), logical(b), op, rc, &r))
    abort();
  return wide(r.value);
}
static Wide extended(GamePkgExt80 a) {
  GamePkgF128Result r;
  if (gamepkg_x86_f128_from_ext80(a, &r))
    abort();
  return wide(r.value);
}
static Wide narrow64(Wide a, unsigned rc) {
  GamePkgF128To80 r;
  if (gamepkg_x86_f128_to_ext80(logical(a), rc, &r))
    abort();
  return extended(r.value);
}
static Wide chop67(Wide a) {
  a.v0 &= UINT64_C(0xffffc00000000000);
  return a;
}
static Wide f128_mul_67_chop(Wide a, Wide b, unsigned rc) {
  (void)rc;
  return chop67(operation(a, b, 2, 3));
}
static Wide f128_mul_e67(Wide a, Wide b, unsigned rc) {
  return f128_mul_67_chop(a, narrow64(b, 3), rc);
}
static Wide f128_add_67_chop(Wide a, Wide b, unsigned rc) {
  (void)rc;
  return chop67(operation(a, b, 0, 3));
}
static Wide f128_add_64_ne(Wide a, Wide b, unsigned rc) {
  (void)rc;
  return narrow64(operation(a, b, 0, 0), 0);
}
static Wide f128_mul_64_ne(Wide a, Wide b, unsigned rc) {
  (void)rc;
  return narrow64(operation(a, narrow64(b, 3), 2, 0), 0);
}
static int lt128(uint64_t ah, uint64_t al, uint64_t bh, uint64_t bl) {
  return ah < bh || (ah == bh && al < bl);
}
static void subtract(uint64_t ah, uint64_t al, uint64_t bh, uint64_t bl,
                     uint64_t *h, uint64_t *l) {
  *l = al - bl;
  *h = ah - bh - (al < bl);
}
/* Interpret hi:lo on the fixed /2^127 scale with the supplied biased exponent.
 * Reduction uses a 66-bit divisor; discarded low bits are identically zero. */
static Wide normalized(uint64_t hi, uint64_t lo, int32_t exp) {
  if (!(hi | lo))
    return (Wide){0, 0};
  while (!(hi >> 63)) {
    hi = (hi << 1) | (lo >> 63);
    lo <<= 1;
    --exp;
  }
  if (lo & 0x7fffu)
    abort();
  return (Wide){((uint64_t)exp << 48) |
                    ((hi >> 15) & UINT64_C(0x0000ffffffffffff)),
                (hi << 49) | (lo >> 15)};
}
static int sincos_reduce(int32_t exp, uint64_t sig, Wide *r, int *negative) {
  int32_t d = exp - 16383;
  uint64_t hi, lo;
  unsigned q = 0;
  if (d < -1) {
    *r = extended((GamePkgExt80){sig, (uint16_t)exp});
    *negative = 0;
    return 0;
  }
  const uint64_t ph = UINT64_C(0xc90fdaa22168c234),
                 pl = UINT64_C(0xc000000000000000);
  if (d <= 0) {
    hi = d ? sig >> 1 : sig;
    lo = d ? sig << 63 : 0;
    if (!lt128(hi, lo, ph, pl)) {
      subtract(hi, lo, ph, pl, &hi, &lo);
      q = 1;
    }
  } else {
    uint64_t yh = sig >> (64 - d), yl = sig << d;
    hi = lo = 0;
    for (unsigned i = 0; i < 192; ++i) {
      uint64_t bit = i < 64    ? ((yh >> (63 - i)) & 1)
                     : i < 128 ? ((yl >> (127 - i)) & 1)
                               : 0;
      unsigned carry = (unsigned)(hi >> 63);
      hi = (hi << 1) | (lo >> 63);
      lo = (lo << 1) | bit;
      q = (q << 1) & 3u;
      if (carry || !lt128(hi, lo, ph, pl)) {
        subtract(hi, lo, ph, pl, &hi, &lo);
        q |= 1;
      }
    }
  }
  uint64_t halfh = ph >> 1, halfl = (pl >> 1) | (ph << 63);
  *negative = 0;
  if (lt128(halfh, halfl, hi, lo)) {
    subtract(ph, pl, hi, lo, &hi, &lo);
    *negative = 1;
    ++q;
  }
  *r = normalized(hi, lo, 16383);
  return (int)(q & 3);
}
static const Wide f128_one = packFloat128(BX_CONST64(0x3fff000000000000), 0);

static const Wide sc_A[6] = {packFloat128(FSINCOS_A1_HI, FSINCOS_A1_LO),
                             packFloat128(FSINCOS_A2_HI, FSINCOS_A2_LO),
                             packFloat128(FSINCOS_A3_HI, FSINCOS_A3_LO),
                             packFloat128(FSINCOS_A4_HI, FSINCOS_A4_LO),
                             packFloat128(FSINCOS_A5_HI, FSINCOS_A5_LO),
                             packFloat128(FSINCOS_A6_HI, FSINCOS_A6_LO)};
static const Wide sc_B[6] = {packFloat128(FSINCOS_B1_HI, FSINCOS_B1_LO),
                             packFloat128(FSINCOS_B2_HI, FSINCOS_B2_LO),
                             packFloat128(FSINCOS_B3_HI, FSINCOS_B3_LO),
                             packFloat128(FSINCOS_B4_HI, FSINCOS_B4_LO),
                             packFloat128(FSINCOS_B5_HI, FSINCOS_B5_LO),
                             packFloat128(FSINCOS_B6_HI, FSINCOS_B6_LO)};
static const Wide sc_U[4] = {packFloat128(FSINCOS_U1_HI, FSINCOS_U1_LO),
                             packFloat128(FSINCOS_U2_HI, FSINCOS_U2_LO),
                             packFloat128(FSINCOS_U3_HI, FSINCOS_U3_LO),
                             packFloat128(FSINCOS_U4_HI, FSINCOS_U4_LO)};
static const Wide sc_V[4] = {packFloat128(FSINCOS_V1_HI, FSINCOS_V1_LO),
                             packFloat128(FSINCOS_V2_HI, FSINCOS_V2_LO),
                             packFloat128(FSINCOS_V3_HI, FSINCOS_V3_LO),
                             packFloat128(FSINCOS_V4_HI, FSINCOS_V4_LO)};

static Wide sc_sin_c(int i) {
  return packFloat128(fsincos_sin_c_hi[i], fsincos_sin_c_lo[i]);
}
static Wide sc_cos_c(int i) {
  return packFloat128(fsincos_cos_c_hi[i], fsincos_cos_c_lo[i]);
}

static Wide f128_negate(Wide v) {
  v.v64 ^= SC_SIGNBIT;
  return v;
}
static Wide f128_setsign(Wide v, int neg) {
  v.v64 = (v.v64 & ~SC_SIGNBIT) | (neg ? SC_SIGNBIT : 0);
  return v;
}
static Wide f128_xorsign(Wide v, int neg) {
  if (neg)
    v.v64 ^= SC_SIGNBIT;
  return v;
}

static void sincos_getc(Wide s, Wide *outc, int *outi) {
  Wide c;
  int index;
  Bit32s exp = (s.v64 >> 48) & 0x7FFF;
  Bit64u b1 = (s.v64 >> 47) & 1;
  Bit64u b2 = (s.v64 >> 46) & 1;

  index = (int)(((exp & 1) << 2) | (b1 << 1) | b2);

  /* keep the integer bit + b1 + b2, force b3 = 1, zero the rest */
  c.v64 = (s.v64 & ~((BX_CONST64(1) << 45) - 1)) | (BX_CONST64(1) << 45);
  c.v0 = 0;
  *outc = c;
  *outi = index;
}

/* p ~ sin(d), q ~ cos(d)-1, for the table path (|d| small) */
static void sincos_sincosd(Wide d, Wide *outp, Wide *outq, unsigned rc) {
  Wide p, q;
  Wide e = f128_mul_67_chop(d, d, rc);

  Wide p1 = sc_U[3];
  p1 = f128_add_64_ne(f128_mul_67_chop(e, p1, rc), sc_U[2], rc);
  p1 = f128_add_64_ne(f128_mul_67_chop(e, p1, rc), sc_U[1], rc);
  p1 = f128_add_64_ne(f128_mul_67_chop(e, p1, rc), sc_U[0], rc);
  p1 = f128_mul_67_chop(e, p1, rc);
  p = f128_add_64_ne(d, f128_mul_67_chop(p1, d, rc), rc);

  Wide q1 = sc_V[3];
  q1 = f128_add_64_ne(f128_mul_67_chop(e, q1, rc), sc_V[2], rc);
  q1 = f128_add_64_ne(f128_mul_67_chop(e, q1, rc), sc_V[1], rc);
  q1 = f128_add_64_ne(f128_mul_67_chop(e, q1, rc), sc_V[0], rc);
  q = f128_mul_64_ne(e, q1, rc);
  *outp = p;
  *outq = q;
}

/* Round  a + b  to a 64-bit (extF80) significand honouring the FPU mode; this
   is the closing step of every sin/cos reconstruction and is the only rounding
   that reaches SW.C1 (magnitude rounded up), so the round-up state left by the
   intermediate steps is cleared first.
   Corner case: when |b| is more than ~112 bits below |a|, f128_add absorbs it
   entirely (sum == a) and roundPackToF128 never flags a round-up - but the tiny
   addend still decided the direction, so it is applied by hand here: opposite
   signs mean |a+b| < |a| rounded up to |a|.  (This is what makes sin(tiny x)
   report C1 the way the hardware does.) */
static GamePkgExt80Result sc_add_e64(Wide a, Wide b, unsigned rc) {
  Wide sum = operation(a, b, 0, rc);
  GamePkgExt80Result r;
  GamePkgF128 q = logical(sum);
  if (gamepkg_x86_ext80_round_f128(q.words, 0, 0x37f | (rc << 10), &r))
    abort();
  if (sum.v64 == a.v64 && sum.v0 == a.v0 && ((b.v64 << 1) | b.v0) &&
      ((a.v64 ^ b.v64) & SC_SIGNBIT))
    r.rounded_up = 1;
  return r;
}

static GamePkgExt80Result sincos_finish(Wide head, Wide f, Wide g,
                                        unsigned rc) {
  Wide fg = f128_add_67_chop(f, g, rc);
  return sc_add_e64(head, fg, rc);
}

// =========================================================================
// FSIN / FCOS core.  Delivers the single result of a standalone FSIN or FCOS.
//   n1 = n mod 4 from the reduction ;  rNeg = sign of r ;
//   n2 = 0 for FSIN, 1 for FCOS  (FCOS is sin shifted by one quadrant) ;
//   k1 = sign(x)  (0 for FCOS, which reduces |x|).
//
// n mod 4 selects the quadrant:  L = n&1 picks sin(r) vs cos(r) ; k2 = (n>>1)&1
// and the argument sign combine into the overall result sign.
//   L = 0 :  result = (-1)^(k1^k2^rNeg) * sin(|r|)
//   L = 1 :  result = (-1)^(k1^k2)      * cos(|r|)
// See the file header for why this is separate from sc_fsincos().
// =========================================================================
static GamePkgExt80Result sc_fsin_fcos(Wide rabs, int rNeg, int n1, int n2,
                                       int k1, unsigned rc) {
  int n = (n1 + n2) & 3;
  int L = n & 1;
  int k2 = (n >> 1) & 1;
  int s1 = k1 ^ k2;
  int s2 = rNeg;

  Bit32s rexp = (rabs.v64 >> 48) & 0x7FFF;

  if (rexp >= FLOATX80_EXP_BIAS - 2) {
    /* |r| >= 1/4 : table-driven.  sin(|r|)/cos(|r|) = sin(c+d)/cos(c+d),
       combined from the sin(c),cos(c) table and the sin(d),cos(d) polys;
       the result sign is folded into sigma/gamma, and the head + (f+g)
       add is the single destination rounding. */
    Wide c;
    int index;
    sincos_getc(rabs, &c, &index);
    Wide sigma = sc_sin_c(index);
    Wide gamma = sc_cos_c(index);

    Wide d = f128_add_67_chop(rabs, f128_negate(c), rc); /* d = s' - c */
    Wide p, q;
    sincos_sincosd(d, &p, &q, rc);

    if (L == 0) {
      int s3 = s1 ^ s2;
      sigma = f128_setsign(sigma, s3);
      gamma = f128_setsign(gamma, s3);
      Wide f = f128_mul_67_chop(sigma, q, rc);
      Wide g = f128_mul_67_chop(gamma, p, rc);
      return sincos_finish(sigma, f, g, rc);
    } else {
      sigma = f128_setsign(sigma, s1);
      gamma = f128_setsign(gamma, s1);
      p = f128_negate(p);
      Wide f = f128_mul_67_chop(sigma, p, rc);
      Wide g = f128_mul_67_chop(gamma, q, rc);
      return sincos_finish(gamma, f, g, rc);
    }
  }

  /* |r| < 1/4 : direct polynomial.  Split odd/even powers into p and q over
     t = s*s, s = r*r, then
         sin(|r|) ~ |r| + |r|*(p + q)        (p,q from A1..A6)
         cos(|r|) ~ 1   + (p + q)            (p,q from B1..B6)   */
  Wide s = f128_mul_e67(rabs, rabs, rc); /* s = r*r  */
  Wide t = f128_mul_e67(s, s, rc);       /* t = s*s  */

  if (L == 0) {
    /* result = (-1)^(s1^s2) * sin(|r|),  the sign folded into r */
    Wide rr = f128_setsign(rabs, s1 ^ s2);

    /* p = s*(A1 + t*(A3 + t*A5)) ; q = t*(A2 + t*(A4 + t*A6)) */
    Wide p = f128_mul_67_chop(t, sc_A[4], rc);
    p = f128_add_64_ne(p, sc_A[2], rc);
    p = f128_mul_67_chop(t, p, rc);
    p = f128_add_64_ne(p, sc_A[0], rc);
    p = f128_mul_67_chop(s, p, rc);

    Wide q = f128_mul_67_chop(t, sc_A[5], rc);
    q = f128_add_64_ne(q, sc_A[3], rc);
    q = f128_mul_67_chop(t, q, rc);
    q = f128_add_64_ne(q, sc_A[1], rc);
    q = f128_mul_67_chop(t, q, rc);

    Wide f = f128_add_64_ne(p, q, rc);
    Wide rf = f128_mul_67_chop(rr, f, rc);
    return sc_add_e64(rr, rf, rc);
  } else {
    /* result = (-1)^s1 * cos(|r|),  cos being even */
    /* p = s*(B1 + t*(B3 + t*B5)) ; q = t*(B2 + t*(B4 + t*B6)) */
    Wide p = f128_mul_67_chop(t, sc_B[4], rc);
    p = f128_add_64_ne(p, sc_B[2], rc);
    p = f128_mul_67_chop(t, p, rc);
    p = f128_add_64_ne(p, sc_B[0], rc);
    p = f128_mul_67_chop(s, p, rc);

    Wide q = f128_mul_67_chop(t, sc_B[5], rc);
    q = f128_add_64_ne(q, sc_B[3], rc);
    q = f128_mul_67_chop(t, q, rc);
    q = f128_add_64_ne(q, sc_B[1], rc);
    q = f128_mul_67_chop(t, q, rc);

    Wide f = f128_add_67_chop(p, q, rc);
    Wide one = f128_setsign(f128_one, s1);
    return sc_add_e64(one, f128_xorsign(f, s1), rc); /* +-(1 + (p+q)) */
  }
}

int gamepkg_fsincos_evaluate(GamePkgExt80 x, uint32_t which, uint32_t control,
                             GamePkgExt80Result *out) {
  unsigned exp = x.sign_exponent & 0x7fff, sign = x.sign_exponent >> 15,
           rc = (control >> 10) & 3;
  if (!out || which > 1 || !x.significand || exp >= 16383 + 63 ||
      (exp && !(x.significand >> 63)))
    return -1;
  int32_t e = exp ? (int32_t)exp : 1;
  uint64_t sig = x.significand;
  while (!(sig >> 63)) {
    sig <<= 1;
    --e;
  }
  if (e < 16383 - 68) {
    /* sin(x)<x in magnitude and cos(x)<1, even when their corrections lie
     * below the intermediate format. A lower tail preserves directed rounding.
     */
    GamePkgF128 q = logical(
        which
            ? packFloat128(UINT64_C(0x3fff000000000000), 0)
            : extended((GamePkgExt80){sig, (uint16_t)((sign << 15) | 16383)}));
    for (unsigned i = 0; i < 4; ++i) {
      uint32_t old = q.words[i];
      --q.words[i];
      if (old)
        break;
    }
    return gamepkg_x86_ext80_round_f128(q.words, which ? 0 : e - 16383, control,
                                        out);
  }
  Wide r;
  int negative;
  int n = sincos_reduce(e, sig, &r, &negative);
  *out = sc_fsin_fcos(r, negative, n, (int)which, which ? 0 : (int)sign, rc);
  return 0;
}

static void sc_fsincos(Wide rabs, int rNeg, int n1, int k1,
                       GamePkgExt80Result *out_sin, GamePkgExt80Result *out_cos,
                       unsigned rc) {
  GamePkgExt80Result sinx, cosx;
  int k2 = n1 & 1;
  int k3 = (n1 >> 1) & 1;
  int sgn_sin = k1 ^ k3;
  int sgn_cos = k2 ^ k3;

  Wide sin_head, sin_tail, cos_head, cos_tail;

  Bit32s rexp = (rabs.v64 >> 48) & 0x7FFF;

  if (rexp >= FLOATX80_EXP_BIAS - 2) {
    /* |r| >= 1/4 : table-driven.
       sin(r): head = sigma,  tail = gamma*p + sigma*q   (then sign of r)
       cos(r): head = gamma,  tail = gamma*q - sigma*p                    */
    Wide c;
    int index;
    sincos_getc(rabs, &c, &index);
    Wide sigma = sc_sin_c(index);
    Wide gamma = sc_cos_c(index);

    Wide d = f128_add_67_chop(rabs, f128_negate(c), rc);
    Wide p, q;
    sincos_sincosd(d, &p, &q, rc);

    Wide gp = f128_mul_67_chop(gamma, p, rc);
    Wide sq = f128_mul_67_chop(sigma, q, rc);
    sin_tail = f128_add_67_chop(gp, sq, rc);
    if (rNeg)
      sin_tail = f128_negate(sin_tail);

    Wide sp = f128_mul_67_chop(sigma, p, rc);
    Wide gq = f128_mul_67_chop(gamma, q, rc);
    cos_tail = f128_add_67_chop(f128_negate(sp), gq, rc);

    sin_head = rNeg ? f128_negate(sigma) : sigma;
    cos_head = gamma;
  } else {
    /* |r| < 1/4 : direct polynomial (single Horner in s = r*r).
       sin(r): head = r,   tail = r * s*(A1 + s*(A2 + ... + s*A6))
       cos(r): head = 1,   tail =     s*(B1 + s*(B2 + ... + s*B6))        */
    Wide rsigned = rNeg ? f128_negate(rabs) : rabs;
    sin_head = rsigned;
    cos_head = f128_one;

    Wide s = f128_mul_e67(rabs, rabs, rc); /* s = r*r */

    Wide p = sc_A[5];
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[4], rc);
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[3], rc);
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[2], rc);
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[1], rc);
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[0], rc);
    p = f128_mul_64_ne(s, p, rc);
    sin_tail = f128_mul_67_chop(rsigned, p, rc);

    Wide qq = sc_B[5];
    qq = f128_add_64_ne(f128_mul_67_chop(s, qq, rc), sc_B[4], rc);
    qq = f128_add_64_ne(f128_mul_67_chop(s, qq, rc), sc_B[3], rc);
    qq = f128_add_64_ne(f128_mul_67_chop(s, qq, rc), sc_B[2], rc);
    qq = f128_add_64_ne(f128_mul_67_chop(s, qq, rc), sc_B[1], rc);
    qq = f128_add_64_ne(f128_mul_67_chop(s, qq, rc), sc_B[0], rc);
    cos_tail = f128_mul_67_chop(s, qq, rc);
  }

  /* reconstruction: n even -> sin(x) from sin(r), cos(x) from cos(r);
     n odd -> they swap.  Each result is  +-head +- tail  rounded once. */
  Wide s_head, s_tail, c_head, c_tail;
  if (k2 == 0) {
    s_head = sin_head;
    s_tail = sin_tail;
    c_head = cos_head;
    c_tail = cos_tail;
  } else {
    s_head = cos_head;
    s_tail = cos_tail;
    c_head = sin_head;
    c_tail = sin_tail;
  }

  sinx = sc_add_e64(f128_xorsign(s_head, sgn_sin),
                    f128_xorsign(s_tail, sgn_sin), rc);
  cosx = sc_add_e64(f128_xorsign(c_head, sgn_cos),
                    f128_xorsign(c_tail, sgn_cos), rc);
  *out_sin = sinx;
  *out_cos = cosx;
}

static void fptan_sin_cos(Wide rabs, int rNeg, Wide *out_f, Wide *out_g,
                          unsigned rc) {
  Wide f, g;
  Bit32s rexp = (rabs.v64 >> 48) & 0x7FFF;

  if (rexp >= FLOATX80_EXP_BIAS - 2) {
    /* |r| >= 1/4 : table-driven, f = sin(c+d), g = cos(c+d) */
    Wide c;
    int index;
    sincos_getc(rabs, &c, &index);
    Wide sigma = sc_sin_c(index);
    Wide gamma = sc_cos_c(index);

    Wide d = f128_add_67_chop(rabs, f128_negate(c), rc); /* d = |r| - c */
    Wide p, q;
    sincos_sincosd(d, &p, &q, rc); /* p ~ sin(d), q ~ cos(d)-1 */

    /* f = sin(c+d) = sigma*(1+q) + gamma*p */
    f = f128_mul_67_chop(gamma, p, rc);
    f = f128_add_67_chop(f, f128_mul_67_chop(sigma, q, rc), rc);
    f = f128_add_67_chop(sigma, f, rc);
    if (rNeg)
      f = f128_negate(f);

    /* g = cos(c+d) = gamma*(1+q) - sigma*p */
    g = f128_negate(f128_mul_67_chop(sigma, p, rc));
    g = f128_add_67_chop(g, f128_mul_67_chop(gamma, q, rc), rc);
    g = f128_add_67_chop(gamma, g, rc);
  } else {
    /* |r| < 1/4 : direct polynomial */
    Wide rsigned = rNeg ? f128_negate(rabs) : rabs;
    Wide s = f128_mul_e67(rabs, rabs, rc); /* s = r*r */

    Wide p = sc_A[5];
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[4], rc);
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[3], rc);
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[2], rc);
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[1], rc);
    p = f128_add_64_ne(f128_mul_67_chop(s, p, rc), sc_A[0], rc);
    p = f128_mul_64_ne(s, p, rc);
    f = f128_add_67_chop(rsigned, f128_mul_67_chop(rsigned, p, rc),
                         rc); /* r + r*p */

    Wide q = sc_B[5];
    q = f128_add_64_ne(f128_mul_67_chop(s, q, rc), sc_B[4], rc);
    q = f128_add_64_ne(f128_mul_67_chop(s, q, rc), sc_B[3], rc);
    q = f128_add_64_ne(f128_mul_67_chop(s, q, rc), sc_B[2], rc);
    q = f128_add_64_ne(f128_mul_67_chop(s, q, rc), sc_B[1], rc);
    q = f128_add_64_ne(f128_mul_67_chop(s, q, rc), sc_B[0], rc);
    q = f128_mul_67_chop(s, q, rc);
    g = f128_add_67_chop(f128_one, q, rc); /* 1 + q */
  }
  *out_f = f;
  *out_g = g;
}

int gamepkg_fsincos_pair(GamePkgExt80 x, uint32_t control,
                         GamePkgExt80Result *sine, GamePkgExt80Result *cosine) {
  unsigned exp = x.sign_exponent & 0x7fff, sign = x.sign_exponent >> 15,
           rc = (control >> 10) & 3;
  if (!sine || !cosine || sine == cosine || !x.significand ||
      exp >= 16383 + 63 || (exp && !(x.significand >> 63)))
    return -1;
  int32_t e = exp ? (int32_t)exp : 1;
  uint64_t sig = x.significand;
  while (!(sig >> 63)) {
    sig <<= 1;
    --e;
  }
  if (e < 16383 - 68) {
    GamePkgExt80Result a, b;
    if (gamepkg_fsincos_evaluate(x, 0, control, &a) ||
        gamepkg_fsincos_evaluate(x, 1, control, &b))
      abort();
    *sine = a;
    *cosine = b;
    return 0;
  }
  Wide r;
  int negative, n = sincos_reduce(e, sig, &r, &negative);
  sc_fsincos(r, negative, n, (int)sign, sine, cosine, rc);
  return 0;
}
int gamepkg_fptan_evaluate(GamePkgExt80 x, uint32_t control,
                           GamePkgExt80Result *out) {
  unsigned exp = x.sign_exponent & 0x7fff, sign = x.sign_exponent >> 15,
           rc = (control >> 10) & 3;
  if (!out || !x.significand || exp >= 16383 + 63 ||
      (exp && !(x.significand >> 63)))
    return -1;
  int32_t e = exp ? (int32_t)exp : 1;
  uint64_t sig = x.significand;
  while (!(sig >> 63)) {
    sig <<= 1;
    --e;
  }
  if (e < 16383 - 68) {
    /* Fixed P5/P6 tiny FPTAN profile: retain x. Modern approximations can
     * differ in the last bit/C1; source and range flags remain host-owned. */
    GamePkgF128 q = logical(
        extended((GamePkgExt80){sig, (uint16_t)((sign << 15) | 16383)}));
    return gamepkg_x86_ext80_round_f128(q.words, e - 16383, control, out);
  }
  Wide r, f, g;
  int negative, n = sincos_reduce(e, sig, &r, &negative);
  fptan_sin_cos(r, negative, &f, &g, rc);
  if (sign)
    f = f128_negate(f);
  Wide numerator = (n & 1) ? f128_negate(g) : f, denominator = (n & 1) ? f : g;
  GamePkgF128Result q;
  if (gamepkg_x86_f128_calculate(logical(numerator), logical(denominator), 3,
                                 rc, &q) ||
      gamepkg_x86_ext80_round_f128(q.value.words, 0, control, out))
    abort();
  /* If the closing wide divide already rounded onto an exact raw80 value,
   * preserve its rounding direction. Otherwise the narrowing step owns C1. */
  if (!(out->exceptions & 32u) && (q.exceptions & 32u)) {
    GamePkgF128Result truncated;
    if (gamepkg_x86_f128_calculate(logical(numerator), logical(denominator), 3,
                                   3, &truncated))
      abort();
    Wide a = wide(q.value), b = wide(truncated.value);
    a.v64 &= ~SC_SIGNBIT;
    b.v64 &= ~SC_SIGNBIT;
    out->rounded_up = a.v64 > b.v64 || (a.v64 == b.v64 && a.v0 > b.v0);
  }
  return 0;
}
