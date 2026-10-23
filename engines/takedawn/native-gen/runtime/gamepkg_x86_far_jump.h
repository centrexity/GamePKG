#ifndef GAMEPKG_X86_FAR_JUMP_H
#define GAMEPKG_X86_FAR_JUMP_H
#include <stdint.h>
/* Pure legacy protected-mode direct-code far-JMP plans, not CPU mutation.
 * These host images are not packed guest memory and contain no host pointers.
 * Callers must check selector lookup BEFORE fetching eight descriptor bytes,
 * then decode those LE bytes into logical lo/hi words. Memory/accessed-bit
 * transactions, ownership, CS-cache commit, retirement, target fetch, gates/
 * tasks, long/v8086 modes and actual instruction admission remain caller work.
 */
typedef struct GamePkgX86DescriptorLookup {
  uint32_t vector, error, table, offset;
} GamePkgX86DescriptorLookup;
typedef struct GamePkgX86CodeTarget {
  uint32_t vector, error, selector, offset, base, limit, attributes,
      mark_accessed;
} GamePkgX86CodeTarget;
/* Return0=lookup/transfer plan,1=guest fault plan,-1=invalid host arguments or
 * unsupported system descriptor (plan_code). Rejected host outputs unchanged.
 * Guest-fault plans contain only vector/error; no proposed CS update. */
int gamepkg_x86_far_lookup(uint32_t selector, uint32_t gdt_limit,
                           uint32_t ldt_limit, uint32_t ldt_usable,
                           GamePkgX86DescriptorLookup *output);
/* Direct code descriptors: normal DPL=CPL/RPL<=CPL; conforming DPL<=CPL.
 * Presence follows privilege/type checks, then offset vs expanded limit.
 * operand16 truncates the offset. Visible selector RPL becomes CPL.
 * attributes contains access byte (A set) and legacy flags nibble at bits8..11.
 * mark_accessed requests a checked LOCK-semantics rights-byte update before
 * any CPU commit. This component performs no such update itself.
 * No input CPU/flags/GDT bytes are modified, including on guest faults. */
int gamepkg_x86_far_plan_code(uint32_t selector, uint32_t offset, uint32_t cpl,
                              uint32_t operand16, uint32_t lo, uint32_t hi,
                              GamePkgX86CodeTarget *output);
/* Check all1..15 instruction bytes against a proposed/loaded code segment.
 * Return0=span allowed,1=#GP(0),-1=invalid host image/size. This checks bounds,
 * not RAM execute permission, actual bytes, mode-aware decode or event
 * delivery. It never preempts the far-transfer plan: EIP==limit can load CS
 * successfully and only fault on the subsequent multi-byte instruction fetch.
 */
int gamepkg_x86_code_fetch_span(GamePkgX86CodeTarget code, uint32_t offset,
                                uint32_t bytes);
#endif
