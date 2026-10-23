#ifndef GAMEPKG_X86_INTERRUPT_H
#define GAMEPKG_X86_INTERRUPT_H
#include <stdint.h>
/* Host-owned 32-bit x86 instruction state, not a guest save-area layout.
 * PE/CPL and CR4 PVI/VME are supplied by the reviewed virtual CPU. This image
 * is separate from the old core fault ABI and MUST accompany future thread/
 * fault contexts. The controlled CPU adapter binds retirement/checkpoint state.
 * No guest instruction may be admitted on the strength of flag changes alone.
 */
typedef struct GamePkgX86InterruptState {
  uint32_t flags, protected_mode, cpl, pvi, vme, sti_shadow;
} GamePkgX86InterruptState;
/* op0=CLI,1=STI. Return0=retired,1=#GP(0) required,-1=invalid host state/op.
 * Rejection/GP leave the entire image unchanged. Success changes only IF or
 * VIF and the STI shadow. A real IF0->1 STI inhibits maskable IRQ delivery at
 * the next boundary; STI when IF1 does not extend an existing shadow. Virtual
 * VIF updates do not inhibit physical IF. LOCK/prefix/opcode qualification is
 * outside this component. Host state must be aligned/live; no native flags,
 * register banks, device callbacks, host locks or allocations are touched.
 */
int gamepkg_x86_interrupt_step(GamePkgX86InterruptState *state, uint32_t op);
/* Retirement of a completed OTHER instruction ends a prior STI shadow.
 * Do not call after interrupt_step (it already performed that retirement).
 * REP interior checkpoints do not retire and must retain a prior STI shadow.
 * MOV/POP SS shadow and debugger semantics remain separate integration work. A
 * delivered exception/event also ends STI inhibition; event delivery uses this
 * same operation before handler entry.
 */
int gamepkg_x86_interrupt_retire_other(GamePkgX86InterruptState *state);
/* Nonmutating boundary query:1=maskable IRQ eligible,0=inhibited,-1=invalid.
 * It neither accepts/acknowledges a device IRQ nor constructs an IDT frame.
 * VIF/VIP/PVI do not mask physical IRQs; NMI/SMI are outside this query.
 */
int gamepkg_x86_interrupt_eligible(const GamePkgX86InterruptState *state);
/* Pure protected flat32 CPL0 hardware-IRQ flag plan. rep_in_progress/trap_gate
 * are booleans supplied by the reviewed boundary/IDT code. This does not accept
 * an IRQ, validate a descriptor, push a frame, change privilege or run a
 * handler. Return1=eligible plan,0=IF/STI inhibition,-1=invalid/unsupported
 * host state. Noneligible/rejected output and input remain unchanged. Caller
 * retains CPU ownership and revalidates the gate/stack before actual event
 * delivery. Other modes/privilege transitions/NMI/SMI remain unsupported here.
 */
typedef struct GamePkgX86IrqFlags {
  uint32_t saved_flags, handler_flags;
} GamePkgX86IrqFlags;
int gamepkg_x86_interrupt_plan_irq(const GamePkgX86InterruptState *state,
                                   uint32_t rep_in_progress, uint32_t trap_gate,
                                   GamePkgX86IrqFlags *output);
#endif
