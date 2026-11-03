#ifndef GAMEPKG_X86_DISPATCHER_OBJECT_H
#define GAMEPKG_X86_DISPATCHER_OBJECT_H
#include "gamepkg_x86_cpu.h"
/* Trusted Xbox dispatcher-object initialization, logical LE guest layouts.
 * Active explicit flat/default32 CPL0 owner, exact bound supervisor RAM and a
 * live fault boundary are required. type0=notification,1=synchronization;
 * event state is a normalized host Boolean0/1. Invalid host profile/type/state
 * or wrapping whole-record span returns-1 before RAM access. All selected
 * stores are admitted before the first mutation. No native pointers, handles,
 * locks across faults, GPR/FP/IRQL/IF changes or instruction retirement.
 *
 * Event16: Type,Size,SignalState,self-linked wait head; preserves unused
 * Absolute/Inserted bytes. Timer40: initialized16-byte header (Absolute0,
 * Inserted0,Size10), zero DueTime/list links/Period; preserves Dpc at32..35,
 * which is supplied by a subsequent set operation. The timer starts inactive.
 * CPU/RAM are borrowed and live throughout the call; exclusive CPU ownership
 * serializes participating guest code. This is not atomic to unrelated host
 * writers of the borrowed arena. Reinitializing an object with active waiters
 * or a queued timer is invalid. Queue/wait/set/cancel/expiry/DPC delivery are
 * separate services; initialization neither arms a timer nor wakes a waiter.
 */
int gamepkg_x86_event_initialize(GamePkgX86CpuThread *thread,
                                 GamePkgX86Memory *memory, uint32_t address,
                                 uint32_t type, uint32_t state, uint32_t eip,
                                 uint32_t ebp);
int gamepkg_x86_timer_initialize(GamePkgX86CpuThread *thread,
                                 GamePkgX86Memory *memory, uint32_t address,
                                 uint32_t type, uint32_t eip, uint32_t ebp);
#endif
