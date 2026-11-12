#ifndef GAMEPKG_X86_TIMER_H
#define GAMEPKG_X86_TIMER_H
#include "gamepkg_x86_dpc_queue.h"
typedef struct GamePkgX86Timers GamePkgX86Timers;
/* Host snapshots in100ns units; interrupt time is monotonic without64-bit
 * counter wrap during this VM lifetime. System time is absolute since1601 and
 * may move in either direction. No native clock or sleeping is implicit. */
typedef struct GamePkgX86Clock {
  uint64_t interrupt_time, system_time;
} GamePkgX86Clock;
/* CPU/RAM/DPC queue borrowed, outlive quiescent teardown. head is an8-byte
 * guest list. All methods require exact owner/domain/bound supervisor RAM,
 * flat/default32 CPL0 and configured IRQL, with a live fault boundary except
 * metadata-only clock update. Opaque timer/DPC objects must not overlap or be
 * forged/reinitialized while queued. Native mutexes never cross RAM/faults.
 * status-1=host misuse/allocation/clock backwards,-2=corrupt record/alias,
 * -3=waiters require the not-yet-integrated dispatcher wait service. No fake
 * waking of waiters, instruction/depth/count quota or guest native callback.
 * RAM faults precede mutation/allocation. Compound DPC enqueue and timer
 * commit preflight all relevant writes; unrelated host RAM writers violate
 * exclusive-reservation synchronization. Guest list is deterministic FIFO,
 * not a claimed reproduction of a retail kernel's internal timer wheel.
 */
GamePkgX86Timers *gamepkg_x86_timers_create(GamePkgX86Cpu *, GamePkgX86Memory *,
                                            GamePkgX86DpcQueue *, uint32_t head,
                                            GamePkgX86Clock initial);
int gamepkg_x86_timers_init(GamePkgX86Timers *, GamePkgX86CpuThread *,
                            uint32_t eip, uint32_t ebp);
int gamepkg_x86_timers_clock(GamePkgX86Timers *, GamePkgX86CpuThread *,
                             GamePkgX86Clock);
typedef struct GamePkgX86TimerResult {
  int status;
  uint32_t previous;
} GamePkgX86TimerResult;
/* due is raw signed64 LARGE_INTEGER bits: negative relative, nonnegative
 * absolute. period is nonnegative signed32 milliseconds. Set resets signal;
 * already-due timers signal/queue DPC immediately, periodic timers rearm.
 * BOOLEAN previous reports whether the timer was armed before this call.
 * Cancel does not clear signal or cancel a DPC already queued by expiration.
 * Deferred callback receives system-time low/high as its two argument values.
 */
GamePkgX86TimerResult gamepkg_x86_timer_set(GamePkgX86Timers *,
                                            GamePkgX86CpuThread *,
                                            uint32_t timer, uint64_t due,
                                            uint32_t period, uint32_t dpc,
                                            uint32_t eip, uint32_t ebp);
GamePkgX86TimerResult gamepkg_x86_timer_cancel(GamePkgX86Timers *,
                                               GamePkgX86CpuThread *,
                                               uint32_t timer, uint32_t eip,
                                               uint32_t ebp);
typedef struct GamePkgX86TimerPump {
  int status;
  uint32_t timer, dpc, queued;
} GamePkgX86TimerPump;
/* One atomic timer transition: status0=no work,1=expired,2=absolute deadline
 * adjusted after a clock change. Periodic missed intervals coalesce into one
 * expiration; the next phase deadline remains in the future. Clock update is
 * a separate successful metadata operation and is not rolled back by a later
 * pump fault. Pump until idle before resuming guest execution. Queuing a DPC
 * does not execute it; IF/STI/IRQL eligibility and checked frame/return are
 * handled by the DPC service. Signal state remains until reset/set or a future
 * synchronization-timer wait. Full waits/IRQ delivery remain separate work.
 */
GamePkgX86TimerPump gamepkg_x86_timers_pump(GamePkgX86Timers *,
                                            GamePkgX86CpuThread *, uint32_t eip,
                                            uint32_t ebp);
int gamepkg_x86_timers_release(GamePkgX86Timers *, GamePkgX86CpuThread *,
                               uint32_t abandon);
#endif
