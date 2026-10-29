#ifndef GAMEPKG_X86_DPC_QUEUE_H
#define GAMEPKG_X86_DPC_QUEUE_H
#include "gamepkg_x86_cpu.h"
#include "gamepkg_x86_memory.h"
typedef struct GamePkgX86DpcQueue GamePkgX86DpcQueue;
/* Reviewed host service. CPU/RAM borrowed and live through quiescent release.
 * head is a12-byte guest PRCB image: LIST_ENTRY plus32-bit DpcRoutineActive.
 * Every call requires exact active CPU owner, flat/default32 CPL0, configured
 * IRQL in the same bound supervisor RAM and a live fault boundary. No native
 * lock crosses RAM/code/faults. Metadata is serialized by CPU reservation.
 * Guests must not forge opaque record/link/Inserted/active fields. Padding is
 * untouched; no guest pointer is chased to find host metadata/native code.
 * Return-1=host misuse/allocation,-2=corrupt/overlapping records; RAM faults
 * propagate before mutation. Successful insert/remove return1, duplicate or
 * absent return0. Duplicate insertion preserves existing arguments/order.
 * Full selected-write admission precedes allocation/commit. No quotas.
 */
GamePkgX86DpcQueue *gamepkg_x86_dpc_queue_create(GamePkgX86Cpu *,
                                                 GamePkgX86Memory *,
                                                 uint32_t head);
int gamepkg_x86_dpc_queue_init(GamePkgX86DpcQueue *, GamePkgX86CpuThread *,
                               uint32_t eip, uint32_t ebp);
/* Nonmutating admission for compound timer transitions. With exclusive owner
 * and immutable bindings, subsequent insert has no new RAM failure. Native
 * allocation can still fail before any queue mutation. */
int gamepkg_x86_dpc_queue_check_insert(GamePkgX86DpcQueue *,
                                       GamePkgX86CpuThread *, uint32_t dpc,
                                       uint32_t eip, uint32_t ebp);
int gamepkg_x86_dpc_queue_insert(GamePkgX86DpcQueue *, GamePkgX86CpuThread *,
                                 uint32_t dpc, uint32_t arg1, uint32_t arg2,
                                 uint32_t eip, uint32_t ebp);
int gamepkg_x86_dpc_queue_remove(GamePkgX86DpcQueue *, GamePkgX86CpuThread *,
                                 uint32_t dpc, uint32_t eip, uint32_t ebp);
/* Nonfaulting host metadata diagnostics/compound-object overlap guard. */
int gamepkg_x86_dpc_queue_in_domain(GamePkgX86DpcQueue *, GamePkgX86Cpu *,
                                    GamePkgX86Memory *);
int gamepkg_x86_dpc_queue_disjoint(GamePkgX86DpcQueue *, uint32_t address,
                                   uint32_t bytes);
typedef struct GamePkgX86DpcCall {
  int status; /*1=reserved,0=no eligible work,negative=host/protocol error*/
  uint32_t dpc, pc, context, arg1, arg2, previous_irql, sequence_low,
      sequence_high;
} GamePkgX86DpcCall;
typedef struct GamePkgX86DpcFrame {
  int status;
  uint32_t pc, esp;
} GamePkgX86DpcFrame;
/* IF/STI eligibility plus IRQL<2 required. Dequeues, clears Inserted, sets PRCB
 * active and raises trusted/mirrored IRQL2 atomically before callback entry.
 * A fault/unknown target/unfinished callback retains its ticket and
 * reservation. Fresh routine/context are captured at claim. The active DPC may
 * enqueue itself again with new arguments without changing the current by-value
 * ticket. */
GamePkgX86DpcCall gamepkg_x86_dpc_queue_claim(GamePkgX86DpcQueue *,
                                              GamePkgX86CpuThread *,
                                              uint32_t eip, uint32_t ebp);
/* Checked20-byte stdcall4 frame: trap,dpc,context,arg1,arg2. Stack admission
 * precedes target GP; neither frame nor prepared flag commits on a fault.
 * No raw GPR/FP/context publication or native callback invocation. */
GamePkgX86DpcFrame
gamepkg_x86_dpc_queue_prepare(GamePkgX86DpcQueue *, GamePkgX86CpuThread *,
                              GamePkgX86DpcCall, uint32_t esp,
                              uint32_t return_trap, uint32_t eip, uint32_t ebp);
/* Trusted host must supply its actually observed return PC/ESP at IRQL2.
 * These are host observations, not guest-supplied completion claims.
 * Clears active and restores the
 * previous priority; leaves callback RAM/FP/cache/register effects intact.
 * Caller separately saves/restores the interrupted context. This is not a
 * complete hardware-IRQ/IDT/SEH/thread-switch implementation. */
int gamepkg_x86_dpc_queue_complete(GamePkgX86DpcQueue *, GamePkgX86CpuThread *,
                                   GamePkgX86DpcCall, uint32_t pc, uint32_t esp,
                                   uint32_t eip, uint32_t ebp);
/* Normal release requires empty/inactive. abandon1 is terminal VM teardown,
 * with no guest resume or invented callback completion/priority lowering. */
int gamepkg_x86_dpc_queue_release(GamePkgX86DpcQueue *, GamePkgX86CpuThread *,
                                  uint32_t abandon);
#endif
