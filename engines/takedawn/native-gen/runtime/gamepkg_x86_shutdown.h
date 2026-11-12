#ifndef GAMEPKG_X86_SHUTDOWN_H
#define GAMEPKG_X86_SHUTDOWN_H
#include "gamepkg_x86_cpu.h"
#include "gamepkg_x86_memory.h"
typedef struct GamePkgX86Shutdown GamePkgX86Shutdown;
/* Host-only value ticket, never guest RAM or a native guest function pointer.
 * status1=reserved callback,0=drained,-1=host misuse,-2=corrupt guest links. */
typedef struct GamePkgX86ShutdownCall {
  int status;
  uint32_t registration, pc, sequence_low, sequence_high;
} GamePkgX86ShutdownCall;
typedef struct GamePkgX86ShutdownFrame {
  int status;
  uint32_t pc, esp;
} GamePkgX86ShutdownFrame;
/* Creation requires the supervisor memory object already bound on this worker.
 * CPU and memory are borrowed and MUST outlive this service. Mutations require
 * the active owner in that CPU, exact bound supervisor RAM and explicit flat
 * default32 CPL0 code/tables. All handles are trusted host objects. Exclusive
 * CPU ownership serializes ALL kernel-list access; out-of-band list writers
 * violate this contract. Memory maps/backings remain immutable/live under a
 * binding. No host mutex, native segment load or execution quota is added. */
GamePkgX86Shutdown *gamepkg_x86_shutdown_create(GamePkgX86Cpu *cpu,
                                                GamePkgX86Memory *memory,
                                                uint32_t guest_list_head);
int gamepkg_x86_shutdown_init(GamePkgX86Shutdown *, GamePkgX86CpuThread *,
                              uint32_t eip, uint32_t ebp);
/* ABI is16 LE bytes: routine PC, signed32 priority, LIST_ENTRY at8.
 * Higher signed priorities first; equal priorities preserve insertion order.
 * Routine is reread at dispatch, priority orders at registration. Duplicate
 * queued registrations, overlapping objects or corrupt host-checked guest
 * links return-2 unchanged; absent unregister is idempotent. All faulting
 * reads/admission precede writes and host-node allocation, preventing partial
 * transitions/leaks. Commits have no fallible work under the binding contract.
 * Only addressed link words change; removed nodes retain their old links.
 * Returns0/-1/-2; real RAM faults leave the mandatory live fault boundary.
 * Caller owns Boolean ABI normalization and host-call retirement. */
int gamepkg_x86_shutdown_register(GamePkgX86Shutdown *, GamePkgX86CpuThread *,
                                  uint32_t registration, uint32_t add,
                                  uint32_t eip, uint32_t ebp);
int gamepkg_x86_shutdown_begin(GamePkgX86Shutdown *, GamePkgX86CpuThread *);
/* Pop one callback; allow registration/unregistration during draining. Only
 * one ticket is active. No count/depth limit; process time/memory limits apply.
 * A faulted/unsupported callback stays active until repaired/resumed or an
 * explicit process-abandon teardown. Empty next seals the drained service. */
GamePkgX86ShutdownCall gamepkg_x86_shutdown_next(GamePkgX86Shutdown *,
                                                 GamePkgX86CpuThread *,
                                                 uint32_t eip, uint32_t ebp);
/* Checked host-to-guest stdcall1 transition, not an emulated PUSH/CALL
 * sequence. Admit the complete8-byte frame before target-GP and any write.
 * Invalid host return trap/profile returns-1 before RAM. Commit frame, return
 * new ESP/PC; caller publishes them and uses the strict guest dispatcher. No
 * retirement, GPR/FP/cache commit or native callback call occurs here. On
 * fault, ticket remains reserved and can retry. After success resume the same
 * frame. */
GamePkgX86ShutdownFrame
gamepkg_x86_shutdown_prepare(GamePkgX86Shutdown *, GamePkgX86CpuThread *,
                             GamePkgX86ShutdownCall, uint32_t esp,
                             uint32_t return_trap, uint32_t eip, uint32_t ebp);
/* Complete only after actual dispatcher return to the prepared trap with the
 * original ESP (RET4). Callback changes to FP/cache/RAM are retained. Faults
 * and unresolved imports MUST NOT be reported as completion. */
int gamepkg_x86_shutdown_complete(GamePkgX86Shutdown *, GamePkgX86CpuThread *,
                                  GamePkgX86ShutdownCall, uint32_t pc,
                                  uint32_t esp);
/* Normal release requires sealed/drained state. abandon1 explicitly discards
 * unfinished state for terminal process/image teardown ONLY: all dispatches
 * stopped, no resume or guest access afterward. Caller must quiesce all users.
 * No guest cleanup is falsely claimed for this terminal error path. */
int gamepkg_x86_shutdown_release(GamePkgX86Shutdown *, GamePkgX86CpuThread *,
                                 uint32_t abandon);
#endif
