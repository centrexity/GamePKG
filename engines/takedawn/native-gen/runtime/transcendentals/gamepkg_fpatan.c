/* SPDX-License-Identifier: LGPL-2.1-or-later
 * Numerical evaluation adapted from Bochs fpatan.cc. Original sources,
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

#include "gamepkg_fpatan.h"
#define BX_CONST64 UINT64_C
typedef uint64_t Bit64u;
#include "upstream/fpatan_constants.h"
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
static GamePkgF128 mul64(GamePkgF128 a, GamePkgF128 b) {
  return narrow64(calc(a, narrow64(b, 3), 2, 0), 0);
}

/* For a sufficiently tiny exact dyadic ratio, atan(q) is infinitesimally
 * smaller in magnitude than q. Retain a nonzero lower tail for final directed
 * rounding and range selection. Non-dyadic inputs retain the pinned profile. */
static GamePkgF128 tiny_ratio(GamePkgF128 a, GamePkgF128 b) {
  GamePkgF128Result r;
  if (gamepkg_x86_f128_calculate(a, b, 3, 3, &r))
    abort();
  if (r.exceptions & 32u)
    return chop67(r.value);
  for (unsigned i = 0; i < 4; ++i) {
    uint32_t old = r.value.words[i];
    --r.value.words[i];
    if (old)
      break;
  }
  return r.value;
}
static GamePkgF128 neg(GamePkgF128 q) {
  q.words[3] ^= 0x80000000u;
  return q;
}
static GamePkgF128 coefficient(unsigned family, unsigned degree) {
  static const uint64_t ah[] = {0, FPATAN_A1_HI, FPATAN_A2_HI, FPATAN_A3_HI,
                                FPATAN_A4_HI},
                        al[] = {0, FPATAN_A1_LO, FPATAN_A2_LO, FPATAN_A3_LO,
                                FPATAN_A4_LO};
  static const uint64_t bh[] = {0,
                                FPATAN_B1_HI,
                                FPATAN_B2_HI,
                                FPATAN_B3_HI,
                                FPATAN_B4_HI,
                                FPATAN_B5_HI,
                                FPATAN_B6_HI},
                        bl[] = {0,
                                FPATAN_B1_LO,
                                FPATAN_B2_LO,
                                FPATAN_B3_LO,
                                FPATAN_B4_LO,
                                FPATAN_B5_LO,
                                FPATAN_B6_LO};
  return family ? bits(bh[degree], bl[degree]) : bits(ah[degree], al[degree]);
}
/* Exact bounded reduction: 32v=k*u+r, 0<=k<=32, 0<=r<u. Unsigned carry
 * preserves the 65th shift bit. No floating intermediate or quotient estimate.
 */
static GamePkgExt80 reduce(uint64_t v, uint64_t u, int32_t bias, unsigned *k) {
  int32_t d = bias + 5 - 16383;
  if (d < 0) {
    *k = 0;
    return (GamePkgExt80){v, (uint16_t)(bias + 5)};
  }
  uint64_t r = v;
  unsigned q = 0;
  if (r >= u) {
    r -= u;
    q = 1;
  }
  for (int32_t i = 0; i < d; ++i) {
    unsigned carry = (unsigned)(r >> 63);
    r <<= 1;
    unsigned bit = carry || r >= u;
    if (bit)
      r -= u;
    q = (q << 1) | bit;
  }
  *k = q;
  if (!r)
    return (GamePkgExt80){0, 0};
  int32_t e = 16383;
  while (!(r >> 63)) {
    r <<= 1;
    --e;
  }
  return (GamePkgExt80){r, (uint16_t)e};
}
int gamepkg_fpatan_evaluate(GamePkgExt80 x, GamePkgExt80 y, uint32_t rc,
                            GamePkgF128 *out, int32_t *adjust) {
  unsigned xe = x.sign_exponent & 0x7fff, ye = y.sign_exponent & 0x7fff,
           sx = x.sign_exponent >> 15, sy = y.sign_exponent >> 15;
  if (!out || !adjust || rc > 3 || !x.significand || !y.significand ||
      xe == 0x7fff || ye == 0x7fff || (xe && !(x.significand >> 63)) ||
      (ye && !(y.significand >> 63)))
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
  unsigned swap = ex < ey || (ex == ey && x.significand < y.significand),
           sz = sx ^ sy ^ swap;
  uint64_t u = swap ? y.significand : x.significand,
           v = swap ? x.significand : y.significand;
  int32_t bias = 16383 + (swap ? ex - ey : ey - ex);
  GamePkgF128 f = {{0}}, g = {{0}};
  *adjust = 0;
  /* Normalize tiny ratios before division so binary128 gradual underflow
   * cannot reduce the internal 67-bit precision ahead of raw80 rounding. */
  if (!sx && !swap && ey - ex - (y.significand < x.significand) < -40) {
    *out = tiny_ratio(
        extended((GamePkgExt80){y.significand, (uint16_t)((sy << 15) | 16383)}),
        extended((GamePkgExt80){x.significand, 16383}));
    *adjust = ey - ex;
    return 0;
  }
  if (bias >= 1) {
    GamePkgF128 u128 = extended((GamePkgExt80){u, 16383});
    unsigned k;
    GamePkgExt80 r80 = reduce(v, u, bias, &k);
    GamePkgF128 r = extended(r80);
    if (r80.sign_exponent > 16382 ||
        (r80.sign_exponent == 16382 && r80.significand > u)) {
      r = calc(r, u128, 1, rc);
      ++k;
    }
    if (k > 32)
      abort();
    GamePkgF128 v128 = extended((GamePkgExt80){v, (uint16_t)bias});
    if (k > 1) {
      GamePkgF128 u1 = u128;
      u1.words[3] += 5u << 16;
      GamePkgF128 v1 = mul67(v128, gamepkg_x86_f128_from_u32(k)),
                  den = add67(u1, v1), z = div67(r, den), c = mul64(z, z),
                  h = mul67(c, c);
      if (sz)
        z = neg(z);
      GamePkgF128 t = mul67(z, c);
      GamePkgF128 p = add67(coefficient(0, 1), mul67(h, coefficient(0, 3)));
      GamePkgF128 q =
          mul67(c, add64(coefficient(0, 2), mul67(h, coefficient(0, 4))));
      f = bits(fpatan_atan_hi[k], fpatan_atan_lo[k]);
      if (sz)
        f = neg(f);
      g = add67(z, mul67(t, add64(p, q)));
    } else {
      GamePkgF128 z = div67(v128, u128);
      if (sz)
        z = neg(z);
      f = z;
      if (((z.words[3] >> 16) & 0x7fff) >= 16383 - 40) {
        GamePkgF128 c = mul64(z, z), h = mul67(c, c), t = mul67(z, c);
        GamePkgF128 p = add67(
            mul67(h, add64(mul67(h, coefficient(1, 5)), coefficient(1, 3))),
            coefficient(1, 1));
        GamePkgF128 q = mul67(add67(mul67(h, add64(mul67(h, coefficient(1, 6)),
                                                   coefficient(1, 4))),
                                    coefficient(1, 2)),
                              c);
        g = mul67(t, add64(p, q));
      }
    }
  }
  if (!sx && !swap)
    *out = calc(f, g, 0, rc);
  else {
    GamePkgF128 a = swap ? bits(FPATAN_PIBY2_HI, FPATAN_PIBY2_LO)
                         : bits(FPATAN_PI_HI, FPATAN_PI_LO);
    if (sy)
      a = neg(a);
    *out = calc(a, add67(f, g), 0, rc);
  }
  return 0;
}
