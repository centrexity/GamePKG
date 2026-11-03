#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_x87_decode.h"
#include <stdlib.h>
static unsigned unsupported(GamePkgExt80 a) {
  return (a.sign_exponent & 0x7fff) && !(a.significand >> 63);
}
static unsigned nan_value(GamePkgExt80 a) {
  return (a.sign_exponent & 0x7fff) == 0x7fff &&
         (a.significand & UINT64_C(0x7fffffffffffffff));
}
static unsigned denormal(GamePkgExt80 a) {
  return !(a.sign_exponent & 0x7fff) && a.significand;
}
/* Positive subnormals and pseudo-denormals have effective exponent one.
 * Signs of zero are ignored. Unsupported/NaN values never reach this path. */
static unsigned relation(GamePkgExt80 a, GamePkgExt80 b) {
  unsigned ae = a.sign_exponent & 0x7fff, be = b.sign_exponent & 0x7fff;
  unsigned az = !ae && !a.significand, bz = !be && !b.significand;
  if (az && bz)
    return 0x40;
  unsigned as = a.sign_exponent >> 15, bs = b.sign_exponent >> 15;
  if (as != bs)
    return as ? 1 : 0;
  if (!ae)
    ae = 1;
  if (!be)
    be = 1;
  if (ae == be && a.significand == b.significand)
    return 0x40;
  unsigned less = ae < be || (ae == be && a.significand < b.significand);
  return (less ^ as) ? 1 : 0;
}
uint32_t gamepkg_x86_x87_compare(uint32_t kind, uint32_t source, uint32_t pop,
                                 uint32_t quiet, uint32_t integer_flags,
                                 uint32_t flags, uint32_t low, uint32_t high,
                                 uint32_t ip, uint32_t opcode, uint32_t address,
                                 uint32_t ebp) {
  if (kind > 5 || source > 7 || pop > 2 || quiet > 1 || integer_flags > 1 ||
      opcode > 0x7ff)
    abort();
  gamepkg_x86_x87_wait(ip, ebp);
  GamePkgX87State *s = gamepkg_x86_x87_state();
  unsigned top = (s->status >> 11) & 7, src = (top + source) & 7;
  gamepkg_x86_x87_record(s, ip, opcode);
  if (kind >= 2)
    gamepkg_x86_x87_record_data(s, address);
  GamePkgExt80 a = {s->significand[top], s->sign_exponent[top]}, b = {0, 0};
  unsigned raised = 0, decoded = 0, result = 0x45;
  if (kind == 0)
    b = (GamePkgExt80){s->significand[src], s->sign_exponent[src]};
  else if (kind == 2)
    b = gamepkg_x87_decode_float(low, 23, 8, 127, 0, &decoded);
  else if (kind == 3)
    b = gamepkg_x87_decode_float((uint64_t)low | ((uint64_t)high << 32), 52, 11,
                                 1023, 0, &decoded);
  else if (kind >= 4)
    b = gamepkg_x87_decode_integer(low, kind == 4 ? 16 : 32);
  if (((s->tags >> (2 * top)) & 3) == 3 ||
      (kind == 0 && ((s->tags >> (2 * src)) & 3) == 3))
    raised = 0x41;
  else if (unsupported(a) || unsupported(b))
    raised = 1;
  else if (nan_value(a) || nan_value(b)) {
    if (!quiet ||
        (nan_value(a) && !(a.significand & UINT64_C(0x4000000000000000))) ||
        (nan_value(b) && !(b.significand & UINT64_C(0x4000000000000000))))
      raised = 1;
  } else {
    result = relation(a, b);
    if (denormal(a) || denormal(b) || (decoded & 2))
      raised = 2;
  }
  /* FCOMI-family preserves C1 on measured hardware, unlike the status-word
   * comparisons. This is independently reported on Intel and AMD in QEMU's
   * 2026-09-11 correction; the SDM's unconditional C1-clear text is inaccurate.
   */
  s->status =
      (uint16_t)((s->status &
                  ~((integer_flags && !(raised & 0x40u)) ? 0u : 0x200u)) |
                 raised);
  if (raised & ~s->control & 63u)
    s->status |= 0x8080;
  if (integer_flags)
    flags &= ~0x890u;
  if (integer_flags)
    flags = (flags & ~0x45u) | result;
  else
    s->status = (uint16_t)((s->status & ~0x4500u) | ((result & 1u) << 8) |
                           ((result & 4u) << 8) | ((result & 0x40u) << 8));
  /* The measured comparison relation is visible even with an unmasked
   * exception. It suppresses popping, not the comparison flags. */
  if (raised & ~s->control & 3u)
    return flags;
  for (unsigned i = 0; i < pop; ++i) {
    s->tags |= (uint16_t)(3u << (2 * top));
    top = (top + 1) & 7;
  }
  s->status = (uint16_t)((s->status & ~0x3800u) | (top << 11));
  return flags;
}
