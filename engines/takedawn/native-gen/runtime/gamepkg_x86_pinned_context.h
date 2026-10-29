/* Reviewed host adapter. Include after the pinned recomp_types.h; never
 * declare differently typed aliases for its TLS XMM objects. */
#ifndef GAMEPKG_X86_PINNED_CONTEXT_H
#define GAMEPKG_X86_PINNED_CONTEXT_H
#include "gamepkg_x86_fault.h"
#include <stddef.h>
#ifndef RECOMP_XMM_DEFINED
#error Include the reviewed runtime XMM declarations first
#endif
_Static_assert(sizeof(RecompXmm) == 16, "XMM register width");
_Static_assert(offsetof(RecompXmm, u) == 0, "XMM lane offset");
_Static_assert(sizeof(((RecompXmm *)0)->u) == 16, "XMM lane bank width");
_Static_assert(sizeof(((RecompXmm *)0)->u[0]) == 4, "XMM lane width");
static inline int gamepkg_x86_capture_pinned(uint32_t eip, uint32_t ebp,
                                             GamePkgX86FpFault *context) {
  const GamePkgX86FpSource source = {.x87 = gamepkg_x86_x87_state(),
                                     .xmm = {g_xmm0.u, g_xmm1.u, g_xmm2.u,
                                             g_xmm3.u, g_xmm4.u, g_xmm5.u,
                                             g_xmm6.u, g_xmm7.u}};
  return gamepkg_x86_capture_registers(eip, ebp, context, &source);
}
static inline int gamepkg_x86_run_pinned(void (*entry)(void),
                                         GamePkgX86FpFault *fault) {
  const GamePkgX86FpSource source = {.x87 = gamepkg_x86_x87_state(),
                                     .xmm = {g_xmm0.u, g_xmm1.u, g_xmm2.u,
                                             g_xmm3.u, g_xmm4.u, g_xmm5.u,
                                             g_xmm6.u, g_xmm7.u}};
  return gamepkg_x86_run_with_fp(entry, fault, &source);
}
static inline int gamepkg_x86_restore_pinned(const GamePkgX86FpFault *fault) {
  const GamePkgX86FpTarget target = {.x87 = gamepkg_x86_x87_state(),
                                     .xmm = {g_xmm0.u, g_xmm1.u, g_xmm2.u,
                                             g_xmm3.u, g_xmm4.u, g_xmm5.u,
                                             g_xmm6.u, g_xmm7.u}};
  return gamepkg_x86_restore_fault_registers(fault, &target);
}
#endif
