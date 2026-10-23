#ifndef GAMEPKG_X86_CPU_H
#define GAMEPKG_X86_CPU_H
#include "gamepkg_x86_far_jump.h"
#include "gamepkg_x86_interrupt.h"
typedef struct GamePkgX86Cpu GamePkgX86Cpu;
typedef struct GamePkgX86CpuThread GamePkgX86CpuThread;
typedef struct GamePkgX86Memory GamePkgX86Memory;
/* Opt-in kernel priority domain, configured before ANY thread is created.
 * Each suspended continuation retains its processor IRQL. The active owner
 * owns that value; it is separate from EFLAGS.IF and the partial execution
 * image below. Required but unbound threads cannot retire guest instructions.
 * The borrowed RAM object must outlive the thread. Snapshot is owner-only,
 * aligned host storage disjoint from CPU/thread. bind/commit are trusted
 * metadata commits AFTER checked supervisor mirror admission/write, with no
 * intervening guest execution. They perform no RAM access or retirement.
 * Binding requires explicit flat/default32 CPL0 code and starts at PASSIVE.
 * No legacy domain acquires an implicit mirror or IRQL state. */
typedef struct GamePkgX86CpuIrql {
  uint32_t required, configured, level, address;
  GamePkgX86Memory *memory;
} GamePkgX86CpuIrql;
int gamepkg_x86_cpu_require_irql(GamePkgX86Cpu *cpu);
int gamepkg_x86_cpu_snapshot_irql(GamePkgX86CpuThread *thread,
                                  GamePkgX86CpuIrql *output);
int gamepkg_x86_cpu_bind_irql(GamePkgX86CpuThread *thread,
                              GamePkgX86Memory *memory, uint32_t address);
int gamepkg_x86_cpu_commit_irql(GamePkgX86CpuThread *thread, uint32_t expected,
                                uint32_t next);
/* Host representations, never native pointers or packed guest layouts.
 * The pre-UMIP legacy protected32 profile uses 16-bit table limits. */
typedef struct GamePkgX86TableRegister {
  uint32_t base, limit;
} GamePkgX86TableRegister;
typedef struct GamePkgX86Tables {
  GamePkgX86TableRegister gdtr, idtr;
} GamePkgX86Tables;
/* Logical hidden CS/LDTR caches, never a packed descriptor or host selector.
 * attrs uses the access byte and flags nibble at bits8..11. An unusable LDTR
 * has a null visible selector; its ignored hidden fields remain explicit.
 * GPR/FP/DS/SS/ES/FS/GS/RAM/PC and handler frames are separate state. */
typedef struct GamePkgX86CpuCodeState {
  uint32_t cs_selector, cs_base, cs_limit, cs_attributes;
  uint32_t ldtr_selector, ldtr_base, ldtr_limit, ldtr_attributes, ldtr_usable;
} GamePkgX86CpuCodeState;
/* Coherent owner-only image. Tables belong to this shared CPU and are
 * immutable after setup; restore checks their identity, never rewrites them.
 * This is a partial CPU context and must accompany the separate raw banks. */
typedef struct GamePkgX86CpuExecution {
  GamePkgX86InterruptState interrupt;
  GamePkgX86Tables tables;
  GamePkgX86CpuCodeState code;
} GamePkgX86CpuExecution;
/* Explicit one-time code-cache setup by the active owner, after table setup.
 * Cached code must be present/accessed, match the current protected non-VM
 * CPL and have a representable expanded limit. No default flat cache exists.
 * Input/output host spans must be aligned/live/disjoint from CPU/thread.
 * All failures return-1 without mutation. No descriptor read is implicit. */
int gamepkg_x86_cpu_init_code(GamePkgX86CpuThread *thread,
                              const GamePkgX86CpuCodeState *code);
int gamepkg_x86_cpu_snapshot_execution(GamePkgX86CpuThread *thread,
                                       GamePkgX86CpuExecution *output);
int gamepkg_x86_cpu_restore_execution(GamePkgX86CpuThread *thread,
                                      const GamePkgX86CpuExecution *image);
/* Trusted commit AFTER checked descriptor/A-bit resolution. Requires a
 * nonfaulting valid proposal with mark_accessed=0 and unchanged current CPL.
 * Changes only the hidden/visible CS fields, preserving LDTR/flags/shadow.
 * No PC commit, instruction retirement, fetch or RAM access is performed. */
int gamepkg_x86_cpu_commit_code(GamePkgX86CpuThread *thread,
                                const GamePkgX86CodeTarget *target);
/* Configure the shared processor exactly once, before creating ANY thread.
 * An explicitly configured zero base/limit is valid; an unconfigured domain
 * is distinct. No RAM capability, native LGDT/LIDT, GDT/IDT entries, segment
 * cache, handler frame or instruction admission is created by this API.
 * Input is aligned/readable host storage, disjoint from the CPU object.
 * Invalid/repeated/late configuration returns-1 without mutation. */
int gamepkg_x86_cpu_init_tables(GamePkgX86Cpu *cpu,
                                const GamePkgX86Tables *tables);
/* Active owner only; output must be aligned/live/disjoint from CPU/thread.
 * Missing configuration/wrong ownership/rejected output returns-1 unchanged.
 * The shared table-register image must accompany future full CPU contexts;
 * it is not part of the old GPR/interrupt snapshot ABI. */
int gamepkg_x86_cpu_snapshot_tables(GamePkgX86CpuThread *thread,
                                    GamePkgX86Tables *output);
/* Reviewed host scheduler domain, never guest pointers/IDs. One domain must
 * be shared by ALL guest workers on the same emulated single processor.
 * Native mutexes protect metadata only; guest execution owns a reservation.
 * No native mutex remains locked across translated code/fault boundaries.
 * The controlled dispatcher connects instruction retirement and REP
 * checkpoints; IRQ posting/acceptance/handler frames remain separate work.
 */
GamePkgX86Cpu *gamepkg_x86_cpu_create(void);
void gamepkg_x86_cpu_release(GamePkgX86Cpu *cpu);
/* Nonmutating trusted-service guard: active current-worker ownership in this
 * exact borrowed CPU domain. Both host handles must remain live. */
int gamepkg_x86_cpu_owner_in_domain(GamePkgX86CpuThread *thread,
                                    GamePkgX86Cpu *cpu);
GamePkgX86CpuThread *
gamepkg_x86_cpu_thread_create(GamePkgX86Cpu *cpu,
                              const GamePkgX86InterruptState *initial);
/* Returns-1 if active, queued or retaining a suspended CPU reservation.
 * No implicit cancellation, force-unlock, thread termination or state loss.
 * External host references to thread handles must be quiescent before destroy.
 */
int gamepkg_x86_cpu_thread_destroy(GamePkgX86CpuThread *thread);
/* Enter waits in FIFO order; a suspended owner resumes its own reservation.
 * One native worker at a time may use a thread handle. Guest GPR/FP/RAM state
 * must be bound/restored separately on that worker before translated entry.
 * Do not cancel/unwind a native worker while it waits in these APIs.
 */
int gamepkg_x86_cpu_enter(GamePkgX86CpuThread *thread);
/* Suspend retains the reservation even with IF0/STI shadow. Other workers
 * cannot steal it while host code resolves a fault/import. Block instead
 * releases it for an actual host wait, only when IF1 and no STI shadow.
 * Both detach the current worker; enter is required before further execution.
 * Configured IRQL>=DISPATCH_LEVEL(2) also inhibits block/yield, independently
 * of IF. Suspend always retains the reservation, including at elevated IRQL.
 * Returns0=success,1=inhibition forbids block,-1=wrong owner/args.
 */
int gamepkg_x86_cpu_suspend(GamePkgX86CpuThread *thread);
int gamepkg_x86_cpu_block(GamePkgX86CpuThread *thread);
/* At an eligible instruction boundary, yield to queued workers then reacquire
 * in FIFO order. Returns1 when inhibited,0 otherwise,-1 for wrong ownership.
 * No instruction counters, time slices, device acknowledgements or CPU quota.
 */
int gamepkg_x86_cpu_yield(GamePkgX86CpuThread *thread);
/* Called by the current owner. Step uses current raw g_eflags supplied by the
 * adapter;0=retired,1=GP required with unchanged image,-1=invalid ownership/op.
 * retire_other synchronizes raw flags and ends the prior STI shadow; delivered
 * exceptions/events use it too. Full images accompany future fault contexts.
 */
int gamepkg_x86_cpu_step(GamePkgX86CpuThread *thread, uint32_t flags,
                         uint32_t op);
int gamepkg_x86_cpu_retire_other(GamePkgX86CpuThread *thread, uint32_t flags);
/* Publish a partially completed instruction's current raw flags, preserving
 * all inhibition/control state. This is NOT retirement or event delivery. */
int gamepkg_x86_cpu_checkpoint(GamePkgX86CpuThread *thread, uint32_t flags);
int gamepkg_x86_cpu_snapshot(GamePkgX86CpuThread *thread,
                             GamePkgX86InterruptState *output);
/* Trusted interrupt-image restore by the active owner. A configured code
 * cache must remain compatible with the new mode/CPL; use restore_execution
 * to restore those together. Does not restore native frames, GPR/FP banks,
 * RAM/segment bindings or dispatcher PC. */
int gamepkg_x86_cpu_restore(GamePkgX86CpuThread *thread,
                            const GamePkgX86InterruptState *image);
/* Host diagnostics only. Live domain handle required. */
uint32_t gamepkg_x86_cpu_waiters(GamePkgX86Cpu *cpu);
/* Nonmutating active-native-owner check:0=owns,-1=other/invalid. */
int gamepkg_x86_cpu_check_owner(GamePkgX86CpuThread *thread);
#endif
