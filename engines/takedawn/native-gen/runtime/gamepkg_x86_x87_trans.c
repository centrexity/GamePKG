#include "gamepkg_x86_ext80.h"
#include "gamepkg_x86_x87.h"
#include "transcendentals/gamepkg_f2xm1.h"
#include "transcendentals/gamepkg_fpatan.h"
#include "transcendentals/gamepkg_fsincos.h"
#include "transcendentals/gamepkg_fyl2x.h"
#include <stdlib.h>

/* A wide multiply may round directly onto a raw80 value. Retain that rounding
 * direction when final narrowing has no fraction of its own; this also applies
 * to the separately wrapped exponent result. All values use logical words. */
static unsigned closing_exact(GamePkgF128 q) {
  return ((q.words[3] >> 16) & 0x7fff) && !q.words[0] &&
         !(q.words[1] & 0x1ffffu);
}
static void closing_hint(GamePkgF128 q, GamePkgF128 truncated,
                         GamePkgExt80Result *r) {
  unsigned greater = 0;
  for (unsigned i = 4; i-- > 0;) {
    uint32_t a = q.words[i], b = truncated.words[i];
    if (i == 3) {
      a &= 0x7fffffffu;
      b &= 0x7fffffffu;
    }
    if (a != b) {
      greater = a > b;
      break;
    }
  }
  if (greater) {
    if (!(r->exceptions & 32u))
      r->rounded_up = 1;
    if (r->range)
      r->wrapped_rounded_up = 1;
  }
}
static void calculate(GamePkgExt80 a, uint32_t control, GamePkgExt80Result *r) {
  unsigned exp = a.sign_exponent & 0x7fff, sign = a.sign_exponent >> 15;
  if ((exp && !(a.significand >> 63)) ||
      (exp == 0x7fff && (a.significand & UINT64_C(0x7fffffffffffffff)))) {
    if (gamepkg_x86_ext80_calculate(a, a, 0, control | 0x300, r))
      abort();
    return;
  }
  *r = (GamePkgExt80Result){0};
  if (!exp && !a.significand) {
    r->value = a;
    return;
  }
  if (exp == 0x7fff) {
    r->value = sign ? (GamePkgExt80){UINT64_C(0x8000000000000000), 0xbfff} : a;
    return;
  }
  r->exceptions = 32;
  r->unbounded_inexact = 1;
  if (exp >= 16383) {
    r->value = a;
    if (exp == 16383 && a.significand == UINT64_C(0x8000000000000000)) {
      unsigned rc = (control >> 10) & 3;
      if (sign)
        r->value = (GamePkgExt80){UINT64_C(0x8000000000000000), 0xbffe};
      else if (rc == 1 || rc == 3)
        r->value = (GamePkgExt80){UINT64_MAX, 0x3ffe};
      else {
        r->value = (GamePkgExt80){UINT64_C(0x8000000000000000), 0x3fff};
        r->rounded_up = 1;
      }
    }
    return;
  }
  GamePkgF128 wide;
  int32_t adjust;
  if (gamepkg_f2xm1_evaluate(a, (control >> 10) & 3, &wide, &adjust) ||
      gamepkg_x86_ext80_round_f128(wide.words, adjust, control, r))
    abort();
  if ((!exp || exp <= 16383 - 69) && ((control >> 10) & 3) != 3 &&
      closing_exact(wide)) {
    GamePkgF128 truncated;
    int32_t shift;
    if (gamepkg_f2xm1_evaluate(a, 3, &truncated, &shift) || shift != adjust)
      abort();
    closing_hint(wide, truncated, r);
  }
  r->exceptions |= 32 | (!exp ? 2u : 0u);
  r->unbounded_inexact = 1;
  if (r->range == 16)
    r->exceptions |= 16;
}

static void logarithm(GamePkgExt80 x, GamePkgExt80 y, uint32_t cw,
                      GamePkgExt80Result *r) {
  unsigned xe = x.sign_exponent & 0x7fff, ye = y.sign_exponent & 0x7fff,
           ys = y.sign_exponent & 0x8000;
  unsigned xn = xe == 0x7fff && (x.significand & UINT64_C(0x7fffffffffffffff)),
           yn = ye == 0x7fff && (y.significand & UINT64_C(0x7fffffffffffffff));
  *r = (GamePkgExt80Result){0};
  if ((xe && !(x.significand >> 63)) || (ye && !(y.significand >> 63)) || xn ||
      yn) {
    if (gamepkg_x86_ext80_calculate(x, y, 0, cw | 0x300, r))
      abort();
    return;
  }
  if ((x.sign_exponent & 0x8000) && x.significand)
    goto invalid;
  if (xe == 0x7fff) {
    if (!y.significand)
      goto invalid;
    r->exceptions = !ye ? 2u : 0u;
    r->value =
        (GamePkgExt80){UINT64_C(0x8000000000000000), (uint16_t)(ys | 0x7fff)};
    return;
  }
  if (ye == 0x7fff) {
    if (xe == 16383 && x.significand == UINT64_C(0x8000000000000000))
      goto invalid;
    r->exceptions = !xe && x.significand ? 2u : 0u;
    r->value = (GamePkgExt80){UINT64_C(0x8000000000000000),
                              (uint16_t)(ys ^ (xe < 16383 ? 0x8000 : 0))};
    r->value.sign_exponent |= 0x7fff;
    return;
  }
  if (!x.significand) {
    if (!y.significand)
      goto invalid;
    r->exceptions = 4;
    r->value = (GamePkgExt80){UINT64_C(0x8000000000000000),
                              (uint16_t)((ys ^ 0x8000) | 0x7fff)};
    return;
  }
  r->exceptions = (!xe || (!ye && y.significand)) ? 2u : 0u;
  if (!y.significand) {
    r->value = (GamePkgExt80){0, (uint16_t)(ys ^ (xe < 16383 ? 0x8000 : 0))};
    return;
  }
  if (xe == 16383 && x.significand == UINT64_C(0x8000000000000000)) {
    r->value = (GamePkgExt80){0, (uint16_t)ys};
    return;
  }
  {
    unsigned de = r->exceptions;
    GamePkgF128 wide;
    int32_t adjust;
    if (gamepkg_fyl2x_evaluate(x, y, (cw >> 10) & 3, &wide, &adjust) ||
        gamepkg_x86_ext80_round_f128(wide.words, adjust, cw, r))
      abort();
    if (((cw >> 10) & 3) != 3 && closing_exact(wide)) {
      GamePkgF128 truncated;
      int32_t shift;
      if (gamepkg_fyl2x_evaluate(x, y, 3, &truncated, &shift) ||
          shift != adjust)
        abort();
      closing_hint(wide, truncated, r);
    }
    r->exceptions |= de | 32;
    r->unbounded_inexact = 1;
    if (r->range == 16)
      r->exceptions |= 16;
    return;
  }
invalid:
  r->exceptions = 1;
  r->value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
}

static void logarithm_plus(GamePkgExt80 x, GamePkgExt80 y, uint32_t cw,
                           GamePkgExt80Result *r) {
  unsigned xe = x.sign_exponent & 0x7fff, ye = y.sign_exponent & 0x7fff,
           sign = (x.sign_exponent ^ y.sign_exponent) & 0x8000;
  *r = (GamePkgExt80Result){0};
  if ((xe && !(x.significand >> 63)) || (ye && !(y.significand >> 63)) ||
      (xe == 0x7fff && (x.significand & UINT64_C(0x7fffffffffffffff))) ||
      (ye == 0x7fff && (y.significand & UINT64_C(0x7fffffffffffffff)))) {
    if (gamepkg_x86_ext80_calculate(x, y, 0, cw | 0x300, r))
      abort();
    return;
  }
  if (xe == 0x7fff) {
    if ((x.sign_exponent & 0x8000) || !y.significand) {
      r->exceptions = 1;
      r->value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
      return;
    }
    r->exceptions = !ye ? 2u : 0u;
    r->value = (GamePkgExt80){UINT64_C(0x8000000000000000),
                              (uint16_t)((y.sign_exponent & 0x8000) | 0x7fff)};
    return;
  }
  if (ye == 0x7fff) {
    if (!x.significand) {
      r->exceptions = 1;
      r->value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
      return;
    }
    r->exceptions = !xe ? 2u : 0u;
    r->value =
        (GamePkgExt80){UINT64_C(0x8000000000000000), (uint16_t)(sign | 0x7fff)};
    return;
  }
  r->exceptions = (!xe && x.significand) || (!ye && y.significand) ? 2u : 0u;
  if (!x.significand || !y.significand) {
    r->value = (GamePkgExt80){0, (uint16_t)sign};
    return;
  }
  r->exceptions |= 32;
  r->unbounded_inexact = 1;
  /* Outside the architectural input interval, retain the fixed legacy profile.
   */
  if ((x.sign_exponent & 0x8000) && xe >= 16383) {
    r->value = x;
    return;
  }
  unsigned de = r->exceptions & 2u;
  GamePkgF128 wide;
  int32_t adjust;
  if (gamepkg_fyl2xp1_evaluate(x, y, (cw >> 10) & 3, &wide, &adjust) ||
      gamepkg_x86_ext80_round_f128(wide.words, adjust, cw, r))
    abort();
  if (((cw >> 10) & 3) != 3 && closing_exact(wide)) {
    GamePkgF128 truncated;
    int32_t shift;
    if (gamepkg_fyl2xp1_evaluate(x, y, 3, &truncated, &shift) ||
        shift != adjust)
      abort();
    closing_hint(wide, truncated, r);
  }
  r->exceptions |= de | 32;
  r->unbounded_inexact = 1;
  if (r->range == 16)
    r->exceptions |= 16;
}

/* Legacy finite-precision pi fractions, as pinned in fpu_constant.h and
 * fpatan.cc. which0=pi,1=pi/2,2=pi/4,3=3pi/4. */
static void angle_constant(unsigned which, unsigned sign, uint32_t cw,
                           GamePkgExt80Result *r) {
  uint64_t hi = which == 3 ? UINT64_C(0x96cbe3f9990e91a7)
                           : UINT64_C(0xc90fdaa22168c234),
           lo = which == 3 ? UINT64_C(0x9000000000000000)
                           : UINT64_C(0xc000000000000000);
  unsigned exp = which == 1 ? 0x3fff : which == 2 ? 0x3ffe : 0x4000;
  uint64_t h = ((uint64_t)(exp | (sign << 15)) << 48) |
               ((hi >> 15) & UINT64_C(0x0000ffffffffffff)),
           l = (hi << 49) | (lo >> 15);
  uint32_t words[4] = {(uint32_t)l, (uint32_t)(l >> 32), (uint32_t)h,
                       (uint32_t)(h >> 32)};
  if (gamepkg_x86_ext80_round_f128(words, 0, cw, r))
    abort();
}
static void arctangent(GamePkgExt80 x, GamePkgExt80 y, uint32_t cw,
                       GamePkgExt80Result *r) {
  unsigned xe = x.sign_exponent & 0x7fff, ye = y.sign_exponent & 0x7fff,
           sx = x.sign_exponent >> 15, sy = y.sign_exponent >> 15;
  if ((xe && !(x.significand >> 63)) || (ye && !(y.significand >> 63)) ||
      (xe == 0x7fff && (x.significand & UINT64_C(0x7fffffffffffffff))) ||
      (ye == 0x7fff && (y.significand & UINT64_C(0x7fffffffffffffff)))) {
    if (gamepkg_x86_ext80_calculate(x, y, 0, cw | 0x300, r))
      abort();
    return;
  }
  unsigned de = (!xe && x.significand) || (!ye && y.significand) ? 2u : 0u;
  *r = (GamePkgExt80Result){0};
  if (ye == 0x7fff)
    angle_constant(xe == 0x7fff ? (sx ? 3 : 2) : 1, sy, cw, r);
  else if (xe == 0x7fff || !y.significand) {
    if (sx)
      angle_constant(0, sy, cw, r);
    else
      r->value = (GamePkgExt80){0, (uint16_t)(sy << 15)};
  } else if (!x.significand)
    angle_constant(1, sy, cw, r);
  else {
    GamePkgF128 wide;
    int32_t adjust;
    if (gamepkg_fpatan_evaluate(x, y, (cw >> 10) & 3, &wide, &adjust) ||
        gamepkg_x86_ext80_round_f128(wide.words, adjust, cw, r))
      abort();
    r->exceptions |= 32;
    r->unbounded_inexact = 1;
    if (r->range == 16)
      r->exceptions |= 16;
  }
  r->exceptions |= de;
}

static void trigonometric(GamePkgExt80 x, unsigned which, uint32_t cw,
                          GamePkgExt80Result *r) {
  unsigned exp = x.sign_exponent & 0x7fff;
  if ((exp && !(x.significand >> 63)) ||
      (exp == 0x7fff && (x.significand & UINT64_C(0x7fffffffffffffff)))) {
    if (gamepkg_x86_ext80_calculate(x, x, 0, cw | 0x300, r))
      abort();
    return;
  }
  *r = (GamePkgExt80Result){0};
  if (exp == 0x7fff) {
    r->exceptions = 1;
    r->value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
    return;
  }
  if (!exp && !x.significand) {
    r->value = which ? (GamePkgExt80){UINT64_C(0x8000000000000000), 0x3fff} : x;
    return;
  }
  if (gamepkg_fsincos_evaluate(x, which, cw, r))
    abort();
  r->exceptions |= 32 | (!exp ? 2u : 0u);
  r->unbounded_inexact = 1;
  if (r->range == 16)
    r->exceptions |= 16;
}
static unsigned classify(GamePkgExt80 v) {
  unsigned e = v.sign_exponent & 0x7fff;
  return !e && !v.significand                        ? 1
         : e && e != 0x7fff && (v.significand >> 63) ? 0
                                                     : 2;
}
static void flags(GamePkgX87State *s, unsigned raised, unsigned up) {
  s->status = (uint16_t)((s->status & ~0x200u) | raised | (up ? 0x200u : 0));
  if (raised & ~s->control & 63u)
    s->status |= 0x8080;
}

static void trig_push(GamePkgX87State *s, unsigned op) {
  unsigned top = (s->status >> 11) & 7, next = (top + 7) & 7;
  unsigned empty = ((s->tags >> (2 * top)) & 3) == 3,
           full = ((s->tags >> (2 * next)) & 3) != 3;
  GamePkgExt80 a, b;
  if (empty || full) {
    flags(s, 0x41, !empty);
    if (!(s->control & 1))
      return;
    a = b = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
  } else {
    GamePkgExt80 x = {s->significand[top], s->sign_exponent[top]};
    unsigned exp = x.sign_exponent & 0x7fff;
    if (exp >= 16383 + 63 && exp < 0x7fff && (x.significand >> 63)) {
      s->status = (uint16_t)((s->status & ~0x200u) | 0x400u);
      return;
    }
    GamePkgExt80Result first = {0}, second = {0};
    unsigned nan =
        (exp && !(x.significand >> 63)) ||
        (exp == 0x7fff && (x.significand & UINT64_C(0x7fffffffffffffff)));
    if (nan) {
      if (gamepkg_x86_ext80_calculate(x, x, 0, s->control | 0x300, &first))
        abort();
      second = first;
    } else if (exp == 0x7fff) {
      first.exceptions = 1;
      first.value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
      second = first;
    } else {
      second.value = (GamePkgExt80){UINT64_C(0x8000000000000000), 0x3fff};
      if (!exp && !x.significand)
        first.value = x;
      else {
        if (op == 5 ? gamepkg_fsincos_pair(x, s->control, &first, &second)
                    : gamepkg_fptan_evaluate(x, s->control, &first))
          abort();
        first.exceptions |= 32 | (!exp ? 2u : 0u);
        first.unbounded_inexact = 1;
        if (first.range == 16)
          first.exceptions |= 16;
        if (second.range == 16)
          second.exceptions |= 16;
      }
    }
    unsigned raised = first.exceptions | second.exceptions;
    unsigned pre = raised & ~s->control & 7u;
    if (pre) {
      unsigned bit = pre & (~pre + 1u);
      flags(s, raised & ((bit << 1) - 1), 0);
      return;
    }
    a = first.value;
    b = second.value;
    /* C1 describes the sine/tangent lane, including its wrapped result. */
    unsigned up = first.rounded_up;
    if (first.range && !(s->control & first.range)) {
      a = first.wrapped;
      raised =
          (raised & ~32u) | first.range | (first.unbounded_inexact ? 32u : 0u);
      up = first.wrapped_rounded_up;
    }
    if (second.range && !(s->control & second.range)) {
      b = second.wrapped;
      raised = (raised & ~32u) | second.range |
               (second.unbounded_inexact ? 32u : 0u);
      up = second.wrapped_rounded_up;
    }
    flags(s, raised, up);
  }
  s->significand[top] = a.significand;
  s->sign_exponent[top] = a.sign_exponent;
  s->significand[next] = b.significand;
  s->sign_exponent[next] = b.sign_exponent;
  s->tags =
      (uint16_t)((s->tags & ~((3u << (2 * top)) | (3u << (2 * next)))) |
                 (classify(a) << (2 * top)) | (classify(b) << (2 * next)));
  s->status = (uint16_t)((s->status & ~0x3800u) | (next << 11));
}
void gamepkg_x86_x87_trans(uint32_t op, uint32_t ip, uint32_t opcode,
                           uint32_t ebp) {
  if (op > 7 || opcode > 0x7ff)
    abort();
  gamepkg_x86_x87_wait(ip, ebp);
  GamePkgX87State *s = gamepkg_x86_x87_state();
  unsigned binary = op == 1 || op == 2 || op == 7;
  unsigned trig = op >= 3 && op <= 6;
  unsigned top = (s->status >> 11) & 7, dst = (top + binary) & 7;
  gamepkg_x86_x87_record(s, ip, opcode);
  if (trig)
    s->status &= (uint16_t)~0x400u;
  if (op == 5 || op == 6) {
    trig_push(s, op);
    return;
  }
  GamePkgExt80 value;
  if (((s->tags >> (2 * top)) & 3) == 3 ||
      (binary && ((s->tags >> (2 * dst)) & 3) == 3)) {
    flags(s, 0x41, 0);
    if (!(s->control & 1))
      return;
    value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
  } else {
    GamePkgExt80Result r;
    GamePkgExt80 x = {s->significand[top], s->sign_exponent[top]};
    if (trig && (x.sign_exponent & 0x7fff) >= 16383 + 63 &&
        (x.sign_exponent & 0x7fff) < 0x7fff && (x.significand >> 63)) {
      s->status = (uint16_t)((s->status & ~0x200u) | 0x400u);
      return;
    }
    if (op == 7)
      logarithm_plus(x,
                     (GamePkgExt80){s->significand[dst], s->sign_exponent[dst]},
                     s->control, &r);
    else if (trig)
      trigonometric(x, op - 3, s->control, &r);
    else if (op == 2)
      arctangent(x, (GamePkgExt80){s->significand[dst], s->sign_exponent[dst]},
                 s->control, &r);
    else if (op)
      logarithm(x, (GamePkgExt80){s->significand[dst], s->sign_exponent[dst]},
                s->control, &r);
    else
      calculate(x, s->control, &r);
    unsigned raised = r.exceptions, up = r.rounded_up;
    value = r.value;
    unsigned pre = raised & ~s->control & 7u;
    if (pre) {
      unsigned first = pre & (~pre + 1u);
      flags(s, raised & ((first << 1) - 1), 0);
      return;
    }
    if (r.range && !(s->control & r.range)) {
      value = r.wrapped;
      up = r.wrapped_rounded_up;
      raised = (raised & ~(r.range | 32u)) | r.range |
               (r.unbounded_inexact ? 32u : 0u);
    }
    flags(s, raised, up);
  }
  s->significand[dst] = value.significand;
  s->sign_exponent[dst] = value.sign_exponent;
  s->tags = (uint16_t)((s->tags & ~(3u << (2 * dst))) |
                       (classify(value) << (2 * dst)));
  if (binary) {
    s->tags |= (uint16_t)(3u << (2 * top));
    s->status = (uint16_t)((s->status & ~0x3800u) | (((top + 1) & 7) << 11));
  }
}
