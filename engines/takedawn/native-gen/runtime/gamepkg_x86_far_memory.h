#ifndef GAMEPKG_X86_FAR_MEMORY_H
#define GAMEPKG_X86_FAR_MEMORY_H
#include "gamepkg_x86_far_jump.h"
/* Explicit trusted legacy protected-mode table/cache image. The caller must
 * qualify protected32/non-v8086 mode and current CPL/LDTR before entry. This
 * is neither a guest layout nor a RAM permission/ownership capability. */
typedef struct GamePkgX86FarTables {
  uint32_t gdt_base, gdt_limit, ldt_base, ldt_limit, ldt_usable;
} GamePkgX86FarTables;
/* Complete selector precheck, checked supervisor descriptor read, direct-code
 * validation and conditional locked accessed-bit update, in that order.
 * Return0 publishes a proposed cache with mark_accessed=0 (write serviced).
 * Return-1 rejects invalid host arguments/unsupported gate/task descriptors;
 * all output bytes remain unchanged. Real selector/page faults propagate
 * through the mandatory live boundary, also with unchanged output. Output
 * must be private aligned host storage, disjoint from borrowed backing RAM.
 * Tables and offsets use architectural linear32 wrap, descriptor bytes LE.
 * A write failure leaves RAM and CPU unchanged; no cache/PC commit, fetch,
 * retirement, CPU ownership, gate/task or handler-frame behavior is provided.
 */
int gamepkg_x86_far_resolve_code(GamePkgX86FarTables tables, uint32_t selector,
                                 uint32_t offset, uint32_t cpl,
                                 uint32_t operand16, uint32_t eip, uint32_t ebp,
                                 GamePkgX86CodeTarget *output);
#endif
