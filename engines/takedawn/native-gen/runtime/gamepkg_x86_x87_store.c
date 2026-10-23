#include "gamepkg_x86_ext80.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_x87.h"
#include <stdlib.h>
typedef struct {
  uint64_t bits;
  unsigned exceptions, up;
} Conversion;
/* Round an unsigned magnitude without native128 or host FP. At/beyond64 bits
 * the quotient is zero; handle the halfway bit separately. shift0 has no
 * remainder and returns before any shift-1 expression can be evaluated. */
static uint64_t rounded(uint64_t sig, unsigned shift, unsigned sign,
                        unsigned rc, unsigned *inexact, unsigned *up) {
  uint64_t q = shift < 64 ? sig >> shift : 0,
           rem = shift < 64 ? sig & ((UINT64_C(1) << shift) - 1) : sig;
  *inexact = rem != 0;
  *up = 0;
  if (!rem)
    return q;
  unsigned greater = shift < 64
                         ? rem > (UINT64_C(1) << (shift - 1))
                         : shift == 64 && rem > UINT64_C(0x8000000000000000);
  unsigned equal = shift < 64
                       ? rem == (UINT64_C(1) << (shift - 1))
                       : shift == 64 && rem == UINT64_C(0x8000000000000000);
  *up = rc == 0   ? (greater || (equal && (q & 1)))
        : rc == 1 ? sign
        : rc == 2 ? !sign
                  : 0;
  return q + *up;
}
static Conversion convert(GamePkgExt80 v, unsigned kind, unsigned cw) {
  unsigned sign = v.sign_exponent >> 15, exp = v.sign_exponent & 0x7fff,
           rc = (cw >> 10) & 3;
  uint64_t sig = v.significand;
  unsigned unsupported = exp && !(sig >> 63),
           nan = exp == 0x7fff && (sig & UINT64_C(0x7fffffffffffffff));
  Conversion out = {0, 0, 0};
  if (kind >= 3) {
    unsigned width = kind == 3 ? 16 : kind == 4 ? 32 : 64;
    uint64_t limit = UINT64_C(1) << (width - 1);
    out.bits = limit;
    if (unsupported || exp == 0x7fff) {
      out.exceptions = 1;
      return out;
    }
    if (!sig) {
      out.bits = 0;
      return out;
    }
    int e = (int)(exp ? exp : 1) - 16383;
    if (e > 63) {
      out.exceptions = 1;
      return out;
    }
    unsigned inexact, up;
    uint64_t q = rounded(sig, (unsigned)(63 - e), sign, rc, &inexact, &up);
    if (q > limit || (!sign && q == limit)) {
      out.exceptions = 1;
      return out;
    }
    out.bits = sign ? 0u - q : q;
    if (width < 64)
      out.bits &= (UINT64_C(1) << width) - 1;
    out.exceptions = inexact ? 32 : 0;
    out.up = up;
    return out;
  }
  unsigned fraction = kind == 0 ? 23 : 52, bias = kind == 0 ? 127 : 1023,
           expmax = kind == 0 ? 255 : 2047;
  uint64_t signbit = (uint64_t)sign << (fraction + (kind == 0 ? 8 : 11));
  out.bits = signbit;
  if (unsupported) {
    out.bits = kind == 0 ? UINT64_C(0xffc00000) : UINT64_C(0xfff8000000000000);
    out.exceptions = 1;
    return out;
  }
  if (exp == 0x7fff) {
    out.bits |= (uint64_t)expmax << fraction;
    if (nan) {
      out.bits |= (sig >> (63 - fraction)) & ((UINT64_C(1) << fraction) - 1);
      out.bits |= UINT64_C(1) << (fraction - 1);
      if (!(sig & UINT64_C(0x4000000000000000)))
        out.exceptions = 1;
    }
    return out;
  }
  if (!sig)
    return out;
  int e = (int)(exp ? exp : 1) - 16383;
  while (!(sig >> 63)) {
    sig <<= 1;
    --e;
  }
  unsigned inexact, up;
  uint64_t q = rounded(sig, 63 - fraction, sign, rc, &inexact, &up);
  int unbounded = e;
  if (q == (UINT64_C(1) << (fraction + 1))) {
    q >>= 1;
    ++unbounded;
  }
  if (unbounded > (int)bias) {
    out.exceptions = 8;
    if (!(cw & 8))
      return out;
    unsigned infinity = rc == 0 || (rc == 1 && sign) || (rc == 2 && !sign);
    out.bits |= infinity ? (uint64_t)expmax << fraction
                         : ((uint64_t)expmax << fraction) - 1;
    out.exceptions |= 32;
    out.up = infinity;
    return out;
  }
  unsigned tiny = unbounded < 1 - (int)bias;
  if (tiny && !(cw & 16)) {
    out.exceptions = 16;
    return out;
  }
  if (e < 1 - (int)bias) {
    q = rounded(sig, (unsigned)(63 - fraction + 1 - (int)bias - e), sign, rc,
                &inexact, &up);
    out.bits |= q;
  } else
    out.bits |= ((uint64_t)(unbounded + (int)bias) << fraction) |
                (q & ((UINT64_C(1) << fraction) - 1));
  out.exceptions = (inexact ? 32u : 0u) | (tiny && inexact ? 16u : 0u);
  out.up = up;
  return out;
}
static unsigned tag(GamePkgExt80 v) {
  unsigned exp = v.sign_exponent & 0x7fff;
  return !exp && !v.significand                          ? 1
         : exp && exp != 0x7fff && (v.significand >> 63) ? 0
                                                         : 2;
}
void gamepkg_x86_x87_store(uint32_t kind, uint32_t destination, uint32_t pop,
                           uint32_t ip, uint32_t opcode, uint32_t address,
                           uint32_t ebp) {
  if (kind > 7 || destination > 7 || pop > 1 || opcode > 0x7ff)
    abort();
  gamepkg_x86_x87_wait(ip, ebp);
  static const unsigned widths[] = {4, 8, 10, 2, 4, 8, 0, 10};
  unsigned width = widths[kind];
  if (width)
    gamepkg_x86_memory_check(address, width, 1, ip, ebp);
  /* The bound view has immutable maps/permissions and holds a live reference.
   * No callback or binding change occurs before the re-admitted store below.
   * Keep all architectural changes private until that store succeeds. */
  GamePkgX87State *s = gamepkg_x86_x87_state(), staged = *s;
  unsigned top = (s->status >> 11) & 7, dst = (top + destination) & 7;
  GamePkgExt80 v = {s->significand[top], s->sign_exponent[top]};
  unsigned raised = 0, up = 0;
  if (((s->tags >> (2 * top)) & 3) == 3) {
    raised = 0x41;
    v = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
  }
  uint32_t words[4] = {0, 0, 0, 0};
  if (kind == 7) {
    Conversion out = convert(v, 5, s->control);
    unsigned sign = v.sign_exponent >> 15;
    uint64_t magnitude = sign ? 0u - out.bits : out.bits;
    if ((out.exceptions & 1) || magnitude >= UINT64_C(1000000000000000000)) {
      /* Intel packed-BCD indefinite: FFFF C000 0000 0000 0000. Invalid
       * conversion supersedes any precision result from integer rounding. */
      raised |= 1;
      words[1] = 0xc0000000u;
      words[2] = 0xffffu;
    } else {
      raised |= out.exceptions;
      up = out.up;
      for (unsigned i = 0; i < 18; ++i) {
        words[i / 8] |= (uint32_t)(magnitude % 10) << ((i % 8) * 4);
        magnitude /= 10;
      }
      words[2] |= sign << 15;
    }
  } else if (kind != 2 && kind != 6) {
    Conversion out = convert(v, kind, s->control);
    raised |= out.exceptions;
    up = out.up;
    words[0] = (uint32_t)out.bits;
    words[1] = (uint32_t)(out.bits >> 32);
  } else {
    words[0] = (uint32_t)v.significand;
    words[1] = (uint32_t)(v.significand >> 32);
    words[2] = v.sign_exponent;
  }
  gamepkg_x86_x87_record(&staged, ip, opcode);
  if (width)
    gamepkg_x86_x87_record_data(&staged, address);
  staged.status =
      (uint16_t)((staged.status & ~0x200u) | raised | (up ? 0x200u : 0u));
  if (raised & ~s->control & 63u)
    staged.status |= 0x8080;
  if (!(raised & ~s->control & 0x19u)) {
    if (width)
      gamepkg_x86_memory_write(address, words, width, (1u << width) - 1, 0, ip,
                               ebp);
    else {
      staged.significand[dst] = v.significand;
      staged.sign_exponent[dst] = v.sign_exponent;
      staged.tags = (uint16_t)((staged.tags & ~(3u << (2 * dst))) |
                               (tag(v) << (2 * dst)));
    }
    if (pop) {
      staged.tags |= (uint16_t)(3u << (2 * top));
      staged.status =
          (uint16_t)((staged.status & ~0x3800u) | (((top + 1) & 7) << 11));
    }
  }
  *s = staged;
}
