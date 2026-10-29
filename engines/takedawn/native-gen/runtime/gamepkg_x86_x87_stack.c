#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_x87_decode.h"
#include <stdlib.h>
static GamePkgExt80 get(const GamePkgX87State *s, unsigned p) {
  return (GamePkgExt80){s->significand[p], s->sign_exponent[p]};
}
static unsigned tag(GamePkgExt80 v) {
  unsigned e = v.sign_exponent & 0x7fff;
  return !e && !v.significand                        ? 1
         : e && e != 0x7fff && (v.significand >> 63) ? 0
                                                     : 2;
}
static void put(GamePkgX87State *s, unsigned p, GamePkgExt80 v) {
  s->significand[p] = v.significand;
  s->sign_exponent[p] = v.sign_exponent;
  s->tags = (uint16_t)((s->tags & ~(3u << (2 * p))) | (tag(v) << (2 * p)));
}
static unsigned empty(const GamePkgX87State *s, unsigned p) {
  return ((s->tags >> (2 * p)) & 3) == 3;
}
static GamePkgExt80 indefinite(void) {
  return (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
}
static void flags(GamePkgX87State *s, unsigned raised, unsigned up) {
  s->status = (uint16_t)((s->status & ~0x200u) | raised | (up ? 0x200u : 0));
  if (raised & ~s->control & 63u)
    s->status |= 0x8080;
}
static unsigned condition(unsigned op, unsigned flags) {
  unsigned cf = flags & 1, zf = (flags >> 6) & 1, pf = (flags >> 2) & 1;
  switch (op) {
  case 10:
    return cf;
  case 11:
    return zf;
  case 12:
    return cf | zf;
  case 13:
    return pf;
  case 14:
    return !cf;
  case 15:
    return !zf;
  case 16:
    return !(cf | zf);
  default:
    return !pf;
  }
}
static GamePkgExt80 integral(GamePkgExt80 value, unsigned rc, unsigned *inexact,
                             unsigned *up) {
  unsigned sign = value.sign_exponent >> 15, exp = value.sign_exponent & 0x7fff;
  int e = (int)(exp ? exp : 1) - 16383;
  *inexact = *up = 0;
  if (!value.significand || e >= 63)
    return value;
  unsigned shift = (unsigned)(63 - e);
  uint64_t sig = value.significand, q = shift < 64 ? sig >> shift : 0,
           rem = shift < 64 ? sig & ((UINT64_C(1) << shift) - 1) : sig;
  *inexact = rem != 0;
  if (rem) {
    unsigned greater = shift < 64
                           ? rem > (UINT64_C(1) << (shift - 1))
                           : shift == 64 && rem > UINT64_C(0x8000000000000000),
             equal = shift < 64
                         ? rem == (UINT64_C(1) << (shift - 1))
                         : shift == 64 && rem == UINT64_C(0x8000000000000000);
    *up = rc == 0   ? (greater || (equal && (q & 1)))
          : rc == 1 ? sign
          : rc == 2 ? !sign
                    : 0;
    q += *up;
  }
  if (!q)
    return (GamePkgExt80){0, (uint16_t)(sign << 15)};
  unsigned bit = 0;
  uint64_t scan = q;
  while (scan >> 1) {
    scan >>= 1;
    ++bit;
  }
  return (GamePkgExt80){q << (63 - bit),
                        (uint16_t)((sign << 15) | (16383 + bit))};
}
void gamepkg_x86_x87_stack(uint32_t op, uint32_t index, uint32_t eflags,
                           uint32_t ip, uint32_t opcode, uint32_t ebp) {
  if (op > 17 || index > 7 || opcode > 0x7ff)
    abort();
  gamepkg_x86_x87_wait(ip, ebp);
  GamePkgX87State *s = gamepkg_x86_x87_state();
  unsigned top = (s->status >> 11) & 7, src = (top + index) & 7;
  gamepkg_x86_x87_record(s, ip, opcode);
  if (op == 7)
    return;
  if (op == 3) {
    s->tags |= (uint16_t)(3u << (2 * src));
    return;
  }
  if (op == 4 || op == 5) {
    top = (top + (op == 4 ? 1 : 7)) & 7;
    s->status = (uint16_t)((s->status & ~0x3a00u) | (top << 11));
    return;
  }
  GamePkgExt80 a = get(s, top), b = get(s, src);
  unsigned raised = 0, up = 0;
  if (op == 6) {
    unsigned exp = a.sign_exponent & 0x7fff, bits;
    if (empty(s, top))
      bits = 0x4100;
    else if (!exp)
      bits = a.significand ? 0x4400 : 0x4000;
    else if (!(a.significand >> 63))
      bits = 0;
    else if (exp == 0x7fff)
      bits = (a.significand & UINT64_C(0x7fffffffffffffff)) ? 0x100 : 0x500;
    else
      bits = 0x400;
    s->status = (uint16_t)((s->status & ~0x4700u) | bits |
                           ((a.sign_exponent & 0x8000) ? 0x200 : 0));
    return;
  }
  if (op == 9) {
    unsigned dst = (top + 7) & 7;
    GamePkgExt80 exponent = indefinite(), significand = indefinite();
    if (empty(s, top))
      raised = 0x41;
    else if (!empty(s, dst)) {
      raised = 0x41;
      up = 1;
    } else {
      unsigned exp = a.sign_exponent & 0x7fff;
      uint64_t sig = a.significand;
      if (exp && !(sig >> 63))
        raised = 1;
      else if (exp == 0x7fff) {
        if (sig & UINT64_C(0x7fffffffffffffff)) {
          exponent = significand = a;
          exponent.significand |= UINT64_C(0x4000000000000000);
          significand = exponent;
          if (!(sig & UINT64_C(0x4000000000000000)))
            raised = 1;
        } else {
          significand = a;
          exponent = (GamePkgExt80){UINT64_C(0x8000000000000000), 0x7fff};
        }
      } else if (!sig) {
        raised = 4;
        significand = a;
        exponent = (GamePkgExt80){UINT64_C(0x8000000000000000), 0xffff};
      } else {
        int e = (int)(exp ? exp : 1) - 16383;
        if (!exp)
          raised = 2;
        while (!(sig >> 63)) {
          sig <<= 1;
          --e;
        }
        significand = (GamePkgExt80){
            sig, (uint16_t)((a.sign_exponent & 0x8000) | 0x3fff)};
        exponent = gamepkg_x87_decode_integer((uint64_t)(int64_t)e, 64);
      }
    }
    flags(s, raised, up);
    if (raised & ~s->control & 7u)
      return;
    put(s, top, exponent);
    put(s, dst, significand);
    s->status = (uint16_t)((s->status & ~0x3800u) | (dst << 11));
    return;
  }
  if (empty(s, top) || ((op == 2 || op >= 10) && empty(s, src)))
    raised = 0x41;
  if (raised) {
    flags(s, raised, 0);
    if (!(s->control & 1))
      return;
    if (op == 2) {
      if (empty(s, top))
        a = indefinite();
      if (empty(s, src))
        b = indefinite();
      put(s, top, b);
      put(s, src, a);
    } else
      put(s, top, indefinite());
    return;
  }
  if (op == 0)
    a.sign_exponent &= 0x7fff;
  else if (op == 1)
    a.sign_exponent ^= 0x8000;
  else if (op == 2) {
    flags(s, 0, 0);
    put(s, top, b);
    put(s, src, a);
    return;
  } else if (op >= 10) {
    /* Ordinary conditional moves retain C1. A new stack fault above clears it.
     */
    if (condition(op, eflags))
      put(s, top, b);
    return;
  } else if (op == 8) {
    unsigned exp = a.sign_exponent & 0x7fff;
    if (exp && !(a.significand >> 63)) {
      raised = 1;
      a = indefinite();
    } else if (exp == 0x7fff) {
      if ((a.significand & UINT64_C(0x7fffffffffffffff)) &&
          !(a.significand & UINT64_C(0x4000000000000000))) {
        raised = 1;
        a.significand |= UINT64_C(0x4000000000000000);
      }
    } else {
      if (!exp && a.significand)
        raised = 2;
      if (raised & ~s->control & 2u) {
        flags(s, raised, 0);
        return;
      }
      unsigned inexact;
      a = integral(a, (s->control >> 10) & 3, &inexact, &up);
      if (inexact)
        raised |= 32;
    }
  }
  flags(s, raised, up);
  if (raised & ~s->control & 3u)
    return;
  put(s, top, a);
}
