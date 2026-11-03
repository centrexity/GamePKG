/* SPDX-License-Identifier: LGPL-2.1-or-later
 * Numerical evaluation adapted from Bochs f2xm1.cc. Original sources, revision,
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

#include "gamepkg_f2xm1.h"
#include "upstream/f2xm1_constants.h"
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
static GamePkgF128 mul64(GamePkgF128 a, GamePkgF128 b) {
  return narrow64(calc(a, narrow64(b, 3), 2, 0), 0);
}
static GamePkgF128 coefficient(unsigned which, unsigned degree) {
  static const uint64_t ah[] = {0,          EXP_A_1_HI, EXP_A_2_HI, EXP_A_3_HI,
                                EXP_A_4_HI, EXP_A_5_HI, EXP_A_6_HI};
  static const uint64_t al[] = {0,          EXP_A_1_LO, EXP_A_2_LO, EXP_A_3_LO,
                                EXP_A_4_LO, EXP_A_5_LO, EXP_A_6_LO};
  static const uint64_t bh[] = {
      0,          EXP_B_1_HI, EXP_B_2_HI, EXP_B_3_HI, EXP_B_4_HI,  EXP_B_5_HI,
      EXP_B_6_HI, EXP_B_7_HI, EXP_B_8_HI, EXP_B_9_HI, EXP_B_10_HI, EXP_B_11_HI};
  static const uint64_t bl[] = {
      0,          EXP_B_1_LO, EXP_B_2_LO, EXP_B_3_LO, EXP_B_4_LO,  EXP_B_5_LO,
      EXP_B_6_LO, EXP_B_7_LO, EXP_B_8_LO, EXP_B_9_LO, EXP_B_10_LO, EXP_B_11_LO};
  return which ? bits(bh[degree], bl[degree]) : bits(ah[degree], al[degree]);
}
static GamePkgF128 polynomial(GamePkgF128 t, unsigned which, unsigned high,
                              unsigned low) {
  GamePkgF128 p = mul67(t, coefficient(which, high));
  for (unsigned i = high - 2;; i -= 2) {
    p = add64(coefficient(which, i), p);
    if (i == low)
      return p;
    p = mul67(t, p);
  }
}
int gamepkg_f2xm1_evaluate(GamePkgExt80 x, uint32_t rc, GamePkgF128 *out,
                           int32_t *adjust) {
  if (!out || !adjust || rc > 3)
    return -1;
  unsigned exp = x.sign_exponent & 0x7fff, sign = x.sign_exponent >> 15;
  if (!x.significand || exp >= 16383 || (exp && !(x.significand >> 63)))
    return -1;
  GamePkgF128 ln2 = bits(EXP_L2_HI, EXP_L2_LO), value;
  int32_t delta = 0;
  if (!exp || exp <= 16383 - 69) {
    int32_t normalized = exp ? (int32_t)exp : 1;
    uint64_t sig = x.significand;
    while (!(sig >> 63)) {
      sig <<= 1;
      --normalized;
    }
    value = calc(
        ln2, extended((GamePkgExt80){sig, (uint16_t)((sign << 15) | 16383)}), 2,
        rc);
    delta = normalized - 16383;
  } else if (exp >= 16381) {
    unsigned index = (sign << 5) | ((exp == 16381 ? 1u : 0u) << 4) |
                     (unsigned)((x.significand >> 59) & 15u);
    GamePkgExt80 c = {(x.significand & UINT64_C(0xf800000000000000)) |
                          UINT64_C(0x0400000000000000),
                      x.sign_exponent};
    GamePkgExt80Result residual;
    if (gamepkg_x86_ext80_calculate(x, c, 1, 0x37f | (rc << 10), &residual))
      abort();
    GamePkgF128 s = mul64(ln2, extended(residual.value)), t = mul67(s, s),
                d = bits(f2xm1_table_hi[index], f2xm1_table_lo[index]);
    GamePkgF128 d1 = add64(d, bits(UINT64_C(0x3fff000000000000), 0));
    GamePkgF128 p = mul67(s, mul67(t, polynomial(t, 0, 6, 2))),
                q = add64(s, mul67(t, polynomial(t, 0, 5, 1)));
    value = calc(d, mul67(d1, add64(p, q)), 0, rc);
  } else {
    GamePkgF128 xq = extended(x), s = mul67(ln2, xq), s1 = mul64(ln2, xq),
                t = mul67(s, s1), u = mul67(t, coefficient(1, 1));
    GamePkgF128 p = mul67(s, mul64(t, polynomial(t, 1, 10, 2))),
                q = mul67(t, mul64(t, polynomial(t, 1, 11, 3)));
    value = calc(s, add64(u, add64(p, q)), 0, rc);
  }
  *out = value;
  *adjust = delta;
  return 0;
}
