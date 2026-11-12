#include "gamepkg_x86_far_jump.h"
#include <stddef.h>
static int output(const void *p, size_t bytes) {
  return p && (uintptr_t)p % _Alignof(uint32_t) == 0 &&
         (uintptr_t)p <= UINTPTR_MAX - (bytes - 1);
}
static int code_fault(GamePkgX86CodeTarget *out, uint32_t vector,
                      uint32_t error) {
  *out = (GamePkgX86CodeTarget){.vector = vector, .error = error};
  return 1;
}
int gamepkg_x86_far_lookup(uint32_t selector, uint32_t gdt_limit,
                           uint32_t ldt_limit, uint32_t ldt_usable,
                           GamePkgX86DescriptorLookup *out) {
  if (!output(out, sizeof(*out)) || selector > 65535 || gdt_limit > 65535 ||
      ldt_usable > 1)
    return -1;
  GamePkgX86DescriptorLookup next = {0};
  uint32_t table = (selector >> 2) & 1, index = selector & 0xfff8u;
  if (!(selector & 0xfffcu)) {
    next.vector = 13;
  } else if ((table && !ldt_usable) ||
             index + 7 > (table ? ldt_limit : gdt_limit)) {
    next.vector = 13;
    next.error = selector & 0xfffcu;
  } else {
    next.table = table;
    next.offset = index;
  }
  *out = next;
  return next.vector ? 1 : 0;
}
int gamepkg_x86_far_plan_code(uint32_t selector, uint32_t offset, uint32_t cpl,
                              uint32_t operand16, uint32_t lo, uint32_t hi,
                              GamePkgX86CodeTarget *out) {
  if (!output(out, sizeof(*out)) || selector > 65535 || cpl > 3 ||
      operand16 > 1)
    return -1;
  if (!(selector & 0xfffcu))
    return code_fault(out, 13, 0);
  uint32_t access = (hi >> 8) & 255, dpl = (access >> 5) & 3,
           error = selector & 0xfffcu;
  /* A system descriptor can denote a gate or task; do not turn a legitimate
   * unimplemented transition into a fictional direct-code #GP. */
  if (!(access & 16))
    return -1;
  if (!(access & 8) ||
      ((access & 4) ? dpl > cpl : (dpl != cpl || (selector & 3) > cpl)))
    return code_fault(out, 13, error);
  if (!(access & 128))
    return code_fault(out, 11, error);
  uint32_t limit = (lo & 65535) | (hi & 0xf0000), flags = (hi >> 20) & 15;
  if (flags & 8)
    limit = (limit << 12) | 4095;
  if (operand16)
    offset &= 65535;
  if (offset > limit)
    return code_fault(out, 13, 0);
  *out = (GamePkgX86CodeTarget){.selector = (selector & 0xfffcu) | cpl,
                                .offset = offset,
                                .base = (lo >> 16) | ((hi & 255) << 16) |
                                        (hi & 0xff000000u),
                                .limit = limit,
                                .attributes = (access | 1) | (flags << 8),
                                .mark_accessed = !(access & 1)};
  return 0;
}
int gamepkg_x86_code_fetch_span(GamePkgX86CodeTarget code, uint32_t offset,
                                uint32_t bytes) {
  if (!bytes || bytes > 15 || code.vector || code.error ||
      code.selector > 65535 || !(code.selector & 0xfffcu) ||
      (code.attributes & ~0xfffu) || (code.attributes & 0x98u) != 0x98u ||
      code.mark_accessed > 1)
    return -1;
  if ((code.attributes & 0x800u) ? ((code.limit & 4095) != 4095)
                                 : (code.limit > 0xfffff))
    return -1;
  uint32_t dpl = (code.attributes >> 5) & 3, cpl = code.selector & 3;
  if ((code.attributes & 4) ? dpl > cpl : dpl != cpl)
    return -1;
  return offset > code.limit || (uint64_t)offset + bytes - 1 > code.limit;
}
