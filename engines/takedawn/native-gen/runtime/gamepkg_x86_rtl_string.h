#ifndef GAMEPKG_X86_RTL_STRING_H
#define GAMEPKG_X86_RTL_STRING_H
#include "gamepkg_x86_cpu.h"
/* Checked legacy ANSI_STRING8 initialization: full NUL scan,16-bit length/
 * maximum wrap, borrowed guest32 Buffer. NULL source clears all fields.
 * This explicit legacy profile matches Wine9 native RtlInitAnsiString; modern
 * documented saturation/retail-Xbox long-string parity are not claimed.
 * Exact active CPU, supervisor-bound RAM, flat32 CPL0 and configured/matching
 * priority mirror required. Resident RAM permits any configured priority.
 * No allocation, lock over a fault, native dereference of guest pointers,
 * GPR/FP/IRQL/retirement change, or artificial scan/instruction quota.
 * Source scan precedes whole8-byte write admission/commit, including aliases.
 * A fault never partially writes the destination. Native fault precedence and
 * partial-write parity are separate qualification. Returns0/-1 host profile.
 */
int gamepkg_x86_rtl_init_ansi(GamePkgX86Cpu *, GamePkgX86CpuThread *,
                              GamePkgX86Memory *, uint32_t destination,
                              uint32_t source, uint32_t eip, uint32_t ebp);
/* Counted byte equality, MaximumLength ignored. Different lengths/empty
 * equal lengths need no buffer fields/data; comparisons stop at mismatch and
 * include embedded NUL. Case mode uses low8 BOOLEAN bits, ASCII-only folding,
 * identity for every other byte, matching the measured Wine9 byte profile.
 * Descriptor fields are read only when required, not as native host structs.
 * Same owner/fault/priority/no-side-effect rules; no guest result write.
 * Full retail locale/NLS, fault precedence and volatile-register parity open.
 */
typedef struct GamePkgX86RtlEqualResult {
  int status;
  uint32_t equal;
} GamePkgX86RtlEqualResult;
GamePkgX86RtlEqualResult
gamepkg_x86_rtl_equal_ansi(GamePkgX86Cpu *, GamePkgX86CpuThread *,
                           GamePkgX86Memory *, uint32_t first, uint32_t second,
                           uint32_t insensitive, uint32_t eip, uint32_t ebp);
#endif
