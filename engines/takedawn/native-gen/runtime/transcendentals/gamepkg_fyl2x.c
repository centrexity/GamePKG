/* SPDX-License-Identifier: LGPL-2.1-or-later
 * Numerical evaluation adapted from Bochs fyl2x.cc. Original sources, revision,
 * notices and license are retained under upstream/. This C adapter uses
 * GamePKG's reviewed multiword helpers instead of the Bochs CPU/status ABI.
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

#include "gamepkg_fyl2x.h"
#define BX_CONST64 UINT64_C
#include "upstream/fyl2x_constants.h"
#include <stdlib.h>
static GamePkgF128 bits(uint64_t hi, uint64_t lo) {
  return (GamePkgF128){
      {(uint32_t)lo, (uint32_t)(lo >> 32), (uint32_t)hi, (uint32_t)(hi >> 32)}};
}
static GamePkgF128 calc(GamePkgF128 a, GamePkgF128 b, unsigned op,
                        unsigned rc) {
  GamePkgF128Result r;
  if (gamepkg_x86_f128_calculate(a, b, op, rc, &r))
    abort();
  return r.value;
}
static GamePkgF128 extended(GamePkgExt80 v) {
  GamePkgF128Result r;
  if (gamepkg_x86_f128_from_ext80(v, &r))
    abort();
  return r.value;
}
static GamePkgF128 narrow64(GamePkgF128 v, unsigned rc) {
  GamePkgF128To80 r;
  if (gamepkg_x86_f128_to_ext80(v, rc, &r))
    abort();
  return extended(r.value);
}
static GamePkgF128 mul67(GamePkgF128 a, GamePkgF128 b) {
  GamePkgF128 r = calc(a, b, 2, 3);
  r.words[0] = 0;
  r.words[1] &= 0xffffc000u;
  return r;
}
static GamePkgF128 add64(GamePkgF128 a, GamePkgF128 b) {
  return narrow64(calc(a, b, 0, 0), 0);
}
static GamePkgF128 chop67(GamePkgF128 r) {
  r.words[0] = 0;
  r.words[1] &= 0xffffc000u;
  return r;
}
static GamePkgF128 add67(GamePkgF128 a, GamePkgF128 b) {
  return chop67(calc(a, b, 0, 3));
}
static GamePkgF128 div67(GamePkgF128 a, GamePkgF128 b) {
  return chop67(calc(a, b, 3, 3));
}
static GamePkgF128 div64(GamePkgF128 a, GamePkgF128 b) {
  return narrow64(calc(a, b, 3, 0), 0);
}
static GamePkgF128 mul64(GamePkgF128 a, GamePkgF128 b) {
  return narrow64(calc(a, narrow64(b, 3), 2, 0), 0);
}
static GamePkgF128 integer(int32_t v) {
  GamePkgF128 r =
      gamepkg_x86_f128_from_u32(v < 0 ? 0u - (uint32_t)v : (uint32_t)v);
  if (v < 0)
    r.words[3] ^= 0x80000000u;
  return r;
}
static GamePkgF128 fyl2x_step5(GamePkgF128 wm, int n) {
  // top 64 bits of the mantissa (implicit 1 + fraction[111:49])
  uint64_t wsig = (BX_CONST64(1) << 63) |
                  ((((uint64_t)wm.words[3] << 32 | wm.words[2]) &
                    BX_CONST64(0x0000FFFFFFFFFFFF))
                   << 15) |
                  (((uint64_t)wm.words[1] << 32 | wm.words[0]) >> 49);

  int index = (int)((wsig >> 58) & 0x1F);

  // c = (1.b1 b2 b3 b4 b5 1) : keep the top 6 significand bits, force the
  // next bit to 1, zero the rest.
  uint64_t sigC = (wsig & (~BX_CONST64(0) << 58)) | (BX_CONST64(1) << 57);
  GamePkgF128 cm = extended((GamePkgExt80){sigC, 0x3fff});
  GamePkgF128 cmNeg = cm;
  cmNeg.words[3] ^= 0x80000000u;

  GamePkgF128 r = add64(wm, cmNeg); /* r = w - c        */
  GamePkgF128 s = add64(r, r);      /* s = 2r           */
  GamePkgF128 t = add64(wm, cm);    /* t = w + c        */
  GamePkgF128 u = div64(s, t);      /* u = 2(w-c)/(w+c) */

  GamePkgF128 d1 = bits(fyl2x_L_hi[index], fyl2x_L_lo[index]);
  GamePkgF128 d2 = bits(fyl2x_T_hi[index], fyl2x_T_lo[index]);
  GamePkgF128 e = add64(integer(n), d1); /* e = n + d1 */

  GamePkgF128 u1 =
      mul67(bits(FYL2X_L_INV_HI, FYL2X_L_INV_LO), u); /* u1 = (1/log2) * u */
  GamePkgF128 v = mul67(u, u);                        /* v  = u*u          */

  /* p = u1 + u * v*(A1 + v*(A2 + v*A3)) */
  GamePkgF128 p = mul67(v, bits(FYL2X_A3_HI, FYL2X_A3_LO));
  p = add64(p, bits(FYL2X_A2_HI, FYL2X_A2_LO));
  p = mul67(v, p);
  p = add64(p, bits(FYL2X_A1_HI, FYL2X_A1_LO));
  p = mul67(v, p);
  p = mul67(p, u);
  p = add64(u1, p);

  /* z = e + (d2 + p) */
  GamePkgF128 z = add67(d2, p);
  z = add67(e, z);
  return z;
}

// -------------------------------------------------------------------------
// |x-1| < 1/8 : polynomial in u.  'sarg' is (x-1) for FYL2X or x for
// FYL2XP1; 't' is the matching denominator (x+1 or x+2).  Returns log2().
// -------------------------------------------------------------------------
static GamePkgF128 fyl2x_poly(GamePkgF128 sarg, GamePkgF128 t) {
  GamePkgF128 s = mul67(bits(FYL2X_L_INV2_HI, FYL2X_L_INV2_LO), sarg);
  GamePkgF128 u = div67(s, t);
  GamePkgF128 v = mul64(u, u);
  GamePkgF128 w = mul67(v, v);

  /* p = v*(B1 + w*(B3 + w*B5)) */
  GamePkgF128 p = mul67(w, bits(FYL2X_B5_HI, FYL2X_B5_LO));
  p = add64(p, bits(FYL2X_B3_HI, FYL2X_B3_LO));
  p = mul67(w, p);
  p = add64(p, bits(FYL2X_B1_HI, FYL2X_B1_LO));
  p = mul67(v, p);

  /* q = w*(B2 + w*(B4 + w*B6)) */
  GamePkgF128 q = mul67(w, bits(FYL2X_B6_HI, FYL2X_B6_LO));
  q = add64(q, bits(FYL2X_B4_HI, FYL2X_B4_LO));
  q = mul67(w, q);
  q = add64(q, bits(FYL2X_B2_HI, FYL2X_B2_LO));
  q = mul67(w, q);

  /* z = u + u*(p + q) */
  GamePkgF128 pq = add64(p, q);
  GamePkgF128 z = mul67(u, pq);
  z = add67(u, z);
  return z;
}

/* Finite positive x!=1, finite nonzero y. Classifications and guest exceptions
 * belong to the caller; normalization keeps all intermediates in range. */
int gamepkg_fyl2x_evaluate(GamePkgExt80 x, GamePkgExt80 y, uint32_t rc,
                           GamePkgF128 *out, int32_t *adjust) {
  unsigned xe = x.sign_exponent & 0x7fff, ye = y.sign_exponent & 0x7fff;
  if (!out || !adjust || rc > 3 || (x.sign_exponent & 0x8000) ||
      !x.significand || !y.significand || xe == 0x7fff || ye == 0x7fff ||
      (xe && !(x.significand >> 63)) || (ye && !(y.significand >> 63)) ||
      (xe == 16383 && x.significand == UINT64_C(0x8000000000000000)))
    return -1;
  int32_t ex = xe ? (int32_t)xe : 1, ey = ye ? (int32_t)ye : 1;
  while (!(x.significand >> 63)) {
    x.significand <<= 1;
    --ex;
  }
  while (!(y.significand >> 63)) {
    y.significand <<= 1;
    --ey;
  }
  GamePkgF128 z;
  /* log2(2^n) is the exact integer n. Avoid table residuals at exact
   * powers, especially when their product is at an exponent boundary. */
  if (x.significand == UINT64_C(0x8000000000000000)) {
    z = integer(ex - 16383);
  } else if ((ex == 16382 && x.significand > UINT64_C(0xe000000000000000)) ||
             (ex == 16383 && x.significand < UINT64_C(0x9000000000000000))) {
    GamePkgF128 q = extended((GamePkgExt80){x.significand, (uint16_t)ex});
    z = fyl2x_poly(add64(q, bits(UINT64_C(0xbfff000000000000), 0)),
                   add67(q, bits(UINT64_C(0x3fff000000000000), 0)));
  } else
    z = fyl2x_step5(extended((GamePkgExt80){x.significand, 16383}), ex - 16383);
  *out =
      calc(z,
           extended((GamePkgExt80){
               y.significand, (uint16_t)((y.sign_exponent & 0x8000) | 16383)}),
           2, rc);
  *adjust = ey - 16383;
  return 0;
}

/* The log1p path keeps x separate from 1+x near zero, including values below
 * the binary128 exponent range, and delays both operand exponents to closing.
 */
int gamepkg_fyl2xp1_evaluate(GamePkgExt80 x, GamePkgExt80 y, uint32_t rc,
                             GamePkgF128 *out, int32_t *adjust) {
  unsigned xe = x.sign_exponent & 0x7fff, ye = y.sign_exponent & 0x7fff;
  if (!out || !adjust || rc > 3 || !x.significand || !y.significand ||
      xe == 0x7fff || ye == 0x7fff || (xe && !(x.significand >> 63)) ||
      (ye && !(y.significand >> 63)) ||
      ((x.sign_exponent & 0x8000) && xe >= 16383))
    return -1;
  int32_t ex = xe ? (int32_t)xe : 1, ey = ye ? (int32_t)ye : 1, extra = 0;
  while (!(x.significand >> 63)) {
    x.significand <<= 1;
    --ex;
  }
  while (!(y.significand >> 63)) {
    y.significand <<= 1;
    --ey;
  }
  GamePkgF128 z;
  if (ex >= 16380) {
    GamePkgF128 q = extended((GamePkgExt80){
        x.significand, (uint16_t)((x.sign_exponent & 0x8000) | ex)});
    GamePkgF128 w = add67(q, bits(UINT64_C(0x3fff000000000000), 0));
    int32_t n = (int32_t)((w.words[3] >> 16) & 0x7fff) - 16383;
    /* Exact 1+x=2^n must not acquire a table residual that changes the
     * product's range response. Above2^64 a raw64 source cannot encode2^n-1. */
    if (ex <= 16383 + 63 &&
        !(w.words[0] | w.words[1] | w.words[2] | (w.words[3] & 0xffffu)))
      z = integer(n);
    else {
      w.words[3] = (w.words[3] & 0xffffu) | 0x3fff0000u;
      z = fyl2x_step5(w, n);
    }
  } else if (ex < 16383 - 70) {
    z = mul67(
        bits(FYL2X_L_INV_HI, FYL2X_L_INV_LO),
        extended((GamePkgExt80){
            x.significand, (uint16_t)((x.sign_exponent & 0x8000) | 16383)}));
    extra = ex - 16383;
  } else {
    GamePkgF128 q = extended((GamePkgExt80){
        x.significand, (uint16_t)((x.sign_exponent & 0x8000) | ex)});
    z = fyl2x_poly(q, add67(q, bits(UINT64_C(0x4000000000000000), 0)));
  }
  *out =
      calc(z,
           extended((GamePkgExt80){
               y.significand, (uint16_t)((y.sign_exponent & 0x8000) | 16383)}),
           2, rc);
  *adjust = ey - 16383 + extra;
  return 0;
}
