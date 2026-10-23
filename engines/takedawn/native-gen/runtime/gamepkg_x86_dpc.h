#ifndef GAMEPKG_X86_DPC_H
#define GAMEPKG_X86_DPC_H
#include "gamepkg_x86_cpu.h"
/* Xbox 28-byte KDPC initialization only. Checked supervisor RAM, live fault
 * boundary, active flat/default32 CPL0 owner. Writes Type=19, Inserted=0,
 * DeferredRoutine and DeferredContext, preserving padding, links and argument
 * slots. Preflights ALL selected writes before changing any byte. Whole
 * logical record must not wrap. Wrong host profile/binding or wrapping span
 * returns-1 without RAM access. Guest PCs/contexts remain32-bit values, never
 * native pointers/callbacks. Caller must not reinitialize a live queued DPC.
 * Queue insertion/removal, priority delivery and callback execution are
 * separate services; initialization neither schedules nor invents success. */
int gamepkg_x86_dpc_initialize(GamePkgX86CpuThread *thread,
                               GamePkgX86Memory *memory, uint32_t address,
                               uint32_t routine, uint32_t context, uint32_t eip,
                               uint32_t ebp);
#endif
