#ifndef GAMEPKG_X86_CPU_DISPATCH_H
#define GAMEPKG_X86_CPU_DISPATCH_H
#include "gamepkg_x86_cpu.h"
#include "gamepkg_x86_dispatch.h"
/* Controlled generated maps only. The caller already owns this shared CPU,
 * has bound its RAM/GPR/FP state on the worker and synchronized raw flags.
 * Before every instruction, eligible queued guest workers may take the CPU.
 * Normal retirement and detected exceptions update the explicit CPU image.
 * The CPU remains reserved on return for host fault/import/stop handling.
 * No IRQ source is posted/accepted and no handler frame is constructed here.
 * Exact controlled CLI/STI use the active CPU provider and special retirement
 * without prematurely clearing a new STI shadow. A pending host request stops
 * at an eligible boundary; actual device/handler delivery remains host work. */
int gamepkg_x86_dispatch_run_cpu(const GamePkgX86DispatchEntry *map,
                                 size_t count, uint32_t pc,
                                 GamePkgX86DispatchExit *result,
                                 const GamePkgX86FpSource *source,
                                 GamePkgX86CpuThread *thread);
/* Optional reviewed host event source. It is polled only at IF-eligible
 * instruction/REP checkpoints, before queued-worker handoff. The state pointer
 * is a borrowed immutable CPU image, valid only for this callback. poll returns
 * 0=no pending stop,1=pending stop,other=host error. The descriptor is frozen.
 * Callbacks preserve live registers, bindings and CPU ownership/state; return
 * normally. They may capture the raw checkpoint and plan an IRQ flag image.
 * A pending stop returns dispatcher reason4 with no invented fault, while the
 * CPU remains reserved. It DOES NOT acknowledge/consume a device request or
 * construct/deliver an IRQ frame. The host must retain the request and complete
 * verified gate/stack/handler integration separately before guest IRQ
 * admission. Source/opaque storage are live host objects, disjoint from
 * output/map/FP banks.
 */
typedef struct GamePkgX86CpuEventSource {
  void *opaque;
  int (*poll)(void *, const GamePkgX86InterruptState *, uint32_t kind,
              uint32_t pc, uint32_t ebp);
} GamePkgX86CpuEventSource;
int gamepkg_x86_dispatch_run_cpu_events(const GamePkgX86DispatchEntry *map,
                                        size_t count, uint32_t pc,
                                        GamePkgX86DispatchExit *result,
                                        const GamePkgX86FpSource *source,
                                        GamePkgX86CpuThread *thread,
                                        const GamePkgX86CpuEventSource *events);
/* Opt-in fully configured CS/LDTR cache service for generated instruction
 * fetch-span checks and far JMP. Requires explicit init_code/table setup and
 * active ownership; no default bootstrap metadata is invented. Legacy run
 * entries retain their existing behavior and cannot execute a far transfer.
 * Current native generator profile is base-zero/default32. A valid transfer
 * to a different profile commits state but stops before target execution.
 * Current x87 CS is bound for future metadata, preserving past FP state and
 * independent DS/SS/FS selection. Nested dispatch restores the outer binding.
 */
int gamepkg_x86_dispatch_run_cpu_code_events(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    GamePkgX86CpuThread *thread, const GamePkgX86CpuEventSource *events);
/* Strict kernel profile: explicit CS/tables plus required, bound IRQL state.
 * Checked supervisor PCR byte is synchronized at entry and after scheduling,
 * checked against the trusted level at each subsequent boundary. A direct
 * guest edit is a protocol stop. Initialization and host services must use
 * the same borrowed memory and exclusive CPU reservation. Old entries reject
 * required-IRQL domains. RAM checks run inside the dispatch fault boundary.
 * Host event polling retains IF/STI semantics; these are stop requests, not
 * delivery of a device IRQ. Higher-priority IRQ delivery is separate work.
 * This does not install NT_TIB/current-thread/TLS during thread switching. */
int gamepkg_x86_dispatch_run_cpu_kernel_events(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    GamePkgX86CpuThread *thread, const GamePkgX86CpuEventSource *events);
#endif
