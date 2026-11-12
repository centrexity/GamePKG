#include "gamepkg_x86_far_memory.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
int gamepkg_x86_far_resolve_code(GamePkgX86FarTables tables, uint32_t selector,
                                 uint32_t offset, uint32_t cpl,
                                 uint32_t operand16, uint32_t eip, uint32_t ebp,
                                 GamePkgX86CodeTarget *out) {
  if (!out || (uintptr_t)out % _Alignof(GamePkgX86CodeTarget) ||
      (uintptr_t)out > UINTPTR_MAX - (sizeof(*out) - 1) || cpl > 3 ||
      operand16 > 1)
    return -1;
  GamePkgX86DescriptorLookup lookup;
  int status = gamepkg_x86_far_lookup(
      selector, tables.gdt_limit, tables.ldt_limit, tables.ldt_usable, &lookup);
  if (status < 0)
    return -1;
  if (status)
    gamepkg_x86_segment_fault(lookup.vector, lookup.error, eip, ebp);
  uint32_t address =
      (lookup.table ? tables.ldt_base : tables.gdt_base) + lookup.offset;
  uint32_t words[2];
  gamepkg_x86_memory_descriptor_read(address, words, eip, ebp);
  GamePkgX86CodeTarget next;
  status = gamepkg_x86_far_plan_code(selector, offset, cpl, operand16, words[0],
                                     words[1], &next);
  if (status < 0)
    return -1;
  if (status)
    gamepkg_x86_segment_fault(next.vector, next.error, eip, ebp);
  if (next.mark_accessed)
    gamepkg_x86_memory_descriptor_accessed(address + 5u, eip, ebp);
  next.mark_accessed = 0;
  *out = next;
  return 0;
}
