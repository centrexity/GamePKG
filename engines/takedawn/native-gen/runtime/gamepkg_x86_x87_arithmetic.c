#include "gamepkg_x86_ext80.h"
#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_x87_decode.h"
#include <stdlib.h>
static unsigned tag(GamePkgExt80 v) {
  unsigned exponent = v.sign_exponent & 0x7fff;
  if (!exponent && !v.significand)
    return 1;
  return exponent && exponent != 0x7fff && (v.significand >> 63) ? 0 : 2;
}
static void flags(GamePkgX87State *s, uint32_t raised, uint32_t c1) {
  s->status = (uint16_t)((s->status & ~0x200u) | (c1 ? 0x200u : 0u) | raised);
  if (raised & ~s->control & 63u)
    s->status |= 0x8080;
}
static void arithmetic(uint32_t op, uint32_t destination, uint32_t source,
                       uint32_t reverse, uint32_t pop, uint32_t ip,
                       uint32_t opcode, const GamePkgExt80 *memory,
                       unsigned memory_denormal) {
  GamePkgX87State *s = gamepkg_x86_x87_state();
  unsigned top = (s->status >> 11) & 7, dst = (top + destination) & 7,
           src = (top + source) & 7;
  gamepkg_x86_x87_record(s, ip, opcode);
  if (op >= 6)
    s->status &= (uint16_t)~0x400u;
  GamePkgExt80 value;
  unsigned raised, c1;
  if (((s->tags >> (2 * dst)) & 3) == 3 ||
      (!memory && ((s->tags >> (2 * src)) & 3) == 3)) {
    flags(s, 0x41, 0);
    if (!(s->control & 1))
      return;
    value = (GamePkgExt80){UINT64_C(0xc000000000000000), 0xffff};
  } else {
    GamePkgExt80 a = {s->significand[dst], s->sign_exponent[dst]},
                 b = memory ? *memory
                            : (GamePkgExt80){s->significand[src],
                                             s->sign_exponent[src]};
    if (reverse) {
      GamePkgExt80 tmp = a;
      a = b;
      b = tmp;
    }
    GamePkgExt80Result result;
    uint32_t condition = 0;
    if (op >= 6
            ? gamepkg_x86_ext80_remainder(a, b, op == 7, &result, &condition)
        : op == 5 ? gamepkg_x86_ext80_scale(a, b, s->control, &result)
                  : gamepkg_x86_ext80_calculate(a, b, op, s->control, &result))
      abort();
    raised = result.exceptions;
    c1 = result.rounded_up;
    value = result.value;
    /* A normalized IEEE memory operand still raises source DE, except
     * when invalid, zero divide or a NaN takes precedence. */
    unsigned a_nan = (a.sign_exponent & 0x7fff) == 0x7fff &&
                     (a.significand & UINT64_C(0x7fffffffffffffff));
    unsigned b_nan = (b.sign_exponent & 0x7fff) == 0x7fff &&
                     (b.significand & UINT64_C(0x7fffffffffffffff));
    if (memory_denormal && !(raised & 5u) && !a_nan && !b_nan)
      raised |= 2u;
    /* Pre-computation exceptions suppress the numerical result and pop.
     * In particular an unmasked denormal must not expose later O/U/P. */
    unsigned pre = raised & ~s->control & 7u;
    if (pre) {
      unsigned first = pre & (~pre + 1u);
      flags(s, raised & ((first << 1u) - 1u), 0);
      return;
    }
    if (result.range && !(s->control & result.range)) {
      value = result.wrapped;
      c1 = result.wrapped_rounded_up;
      raised = (raised & ~(result.range | 32u)) | result.range |
               (result.unbounded_inexact ? 32u : 0u);
    }
    flags(s, raised, c1);
    if (op >= 6 && !(raised & 1u) &&
        !((result.value.sign_exponent & 0x7fff) == 0x7fff &&
          (result.value.significand & UINT64_C(0x7fffffffffffffff))))
      s->status = (uint16_t)((s->status & ~0x4700u) | condition);
  }
  s->significand[dst] = value.significand;
  s->sign_exponent[dst] = value.sign_exponent;
  s->tags =
      (uint16_t)((s->tags & ~(3u << (2 * dst))) | (tag(value) << (2 * dst)));
  if (pop) {
    s->tags |= (uint16_t)(3u << (2 * top));
    s->status = (uint16_t)((s->status & ~0x3800u) | (((top + 1u) & 7u) << 11));
  }
}
void gamepkg_x86_x87_arithmetic(uint32_t op, uint32_t destination,
                                uint32_t source, uint32_t reverse, uint32_t pop,
                                uint32_t ip, uint32_t opcode, uint32_t ebp) {
  if (op > 7 || (op >= 5 && (destination || source != 1 || reverse || pop)) ||
      destination > 7 || source > 7 || reverse > 1 || pop > 1 || opcode > 0x7ff)
    abort();
  gamepkg_x86_x87_wait(ip, ebp);
  arithmetic(op, destination, source, reverse, pop, ip, opcode, 0, 0);
}
void gamepkg_x86_x87_memory_arithmetic(uint32_t op, uint32_t reverse,
                                       uint32_t kind, uint32_t low,
                                       uint32_t high, uint32_t ip,
                                       uint32_t opcode, uint32_t address,
                                       uint32_t ebp) {
  if (op > 3 || reverse > 1 || kind > 3 || opcode > 0x7ff)
    abort();
  gamepkg_x86_x87_wait(ip, ebp);
  unsigned raised = 0;
  GamePkgExt80 value =
      kind == 0 ? gamepkg_x87_decode_float(low, 23, 8, 127, 0, &raised)
      : kind == 1
          ? gamepkg_x87_decode_float((uint64_t)low | ((uint64_t)high << 32), 52,
                                     11, 1023, 0, &raised)
          : gamepkg_x87_decode_integer(low, kind == 2 ? 16 : 32);
  gamepkg_x86_x87_record_data(gamepkg_x86_x87_state(), address);
  arithmetic(op, 0, 0, reverse, 0, ip, opcode, &value, raised & 2u);
}
