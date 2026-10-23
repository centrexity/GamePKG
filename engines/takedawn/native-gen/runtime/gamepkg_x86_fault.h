/* Host-owned native guest fault boundary. Not a guest SDK or an SEH emulator.
 */
#ifndef GAMEPKG_X86_FAULT_H
#define GAMEPKG_X86_FAULT_H
#include "gamepkg_x86_x87.h"
#include <stddef.h>
#include <stdint.h>

typedef struct GamePkgX86Fault {
  uint32_t vector, eip;
  uint32_t eax, ecx, edx, ebx, esp, ebp, esi, edi;
  uint32_t eflags, fs_base, direction, mxcsr;
  uint32_t fault_address, error_code;
} GamePkgX86Fault;

/* Host format, not FXSAVE or a guest memory image. Physical x87 registers
 * include their MMX aliases; XMM lanes are raw logical uint32_t values. */
typedef struct GamePkgX86FpSnapshot {
  GamePkgX87State x87;
  uint32_t xmm[8][4];
} GamePkgX86FpSnapshot;
typedef struct GamePkgX86FpFault {
  GamePkgX86Fault core;
  GamePkgX86FpSnapshot fp;
} GamePkgX86FpFault;
typedef struct GamePkgX86FpSource {
  const GamePkgX87State *x87;
  const uint32_t *xmm[8];
} GamePkgX86FpSource;
typedef struct GamePkgX86FpTarget {
  GamePkgX87State *x87;
  uint32_t *xmm[8];
} GamePkgX86FpTarget;

/* Nonfaulting trusted-host capture at a dispatcher/import boundary. Uses the
 * same host image as fault capture/restore; vector/address/error are zero.
 * EIP and local EBP are supplied explicitly. Captures raw x87/MMX/XMM bits,
 * including pending exceptions, without executing guest arithmetic or WAIT.
 * Returns0 on success,-1 on invalid/overlapping bindings or nonboolean DF;
 * rejection changes neither output nor live registers. This is not a guest
 * context instruction, thread scheduler, segment/RAM binding or SEH image.
 * Source/output storage must be live on this worker and mutually disjoint.
 */
int gamepkg_x86_capture_registers(uint32_t eip, uint32_t ebp,
                                  GamePkgX86FpFault *context,
                                  const GamePkgX86FpSource *source);

/* Restore the captured register file on the current worker. This is a trusted
 * host operation, not guest SETCONTEXT/FXRSTOR. Return0 on success,-1 on
 * invalid pointers, overlapping spans, reserved MXCSR bits or nonboolean
 * direction; rejection changes no registers or output. All target storage is
 * borrowed, writable and distinct from the input/descriptor and other target
 * banks. Raw x87 values/metadata are restored without normalization or
 * pending-fault delivery. Restore does not recreate native frames, select an
 * EIP/SEH handler, restore RAM/segment bindings, or resume execution. The host
 * must separately validate fault->core.eip against its executable dispatch map.
 * Flags follow the new emitter's eager g_eflags ABI; legacy lazy flags need
 * their bridge. */
int gamepkg_x86_restore_fault_registers(const GamePkgX86FpFault *fault,
                                        const GamePkgX86FpTarget *target);

/* Optional complete floating-register capture at the same fault boundary.
 * Source pointers are trusted host storage on the current worker, borrowed
 * until entry exits. Each XMM pointer addresses four uint32_t lanes. Storage
 * must be aligned, readable and mutually disjoint and must not overlap the
 * output. The descriptor is copied before entry; register values remain live.
 * Invalid arguments leave output untouched and do not enter guest code.
 * Normal completion zeros output. Capture does not restore state or resume
 * guest execution; completed instructions are not rolled back. */
/* Nonmutating trusted-host binding validation for extended runtime outputs.
 * Output span must be nonempty/live. Checks bounds/alignment/overlap of source
 * banks/descriptor and output, not ownership. Caller validates output type. */
int gamepkg_x86_fp_source_compatible(const GamePkgX86FpSource *source,
                                     const void *output, size_t output_bytes);
int gamepkg_x86_run_with_fp(void (*entry)(void), GamePkgX86FpFault *fault,
                            const GamePkgX86FpSource *source);

/* A live boundary is mandatory before entering generated code that can fault.
 * Return 0 on normal completion, 1 on a guest fault, -1 on invalid arguments.
 * A fault preserves the faulting guest state and abandons native call frames;
 * the caller must dispatch/recover the guest exception, never continue as if
 * the entry returned normally. This does not walk the guest's SEH chain.
 * Boundaries are nested and thread-local, use no heap storage, and must not be
 * bypassed by an unrelated longjmp across the host boundary itself. They are
 * for translated C frames, not C++ scopes with live destructors. MXCSR is
 * captured; use run_with_fp for floating registers. Guest exception dispatch
 * and resumption remain unimplemented. */
int gamepkg_x86_run(void (*entry)(void), GamePkgX86Fault *fault);
_Noreturn void gamepkg_x86_divide_error(uint32_t eip, uint32_t ebp);
/* Unprefixed CC INT3, protected flat32 profile. Unlike faults, #BP is a trap:
 * saved EIP is the next instruction, vector3, no error code. IDT/privilege
 * transitions, SEH/debugger delivery and other INT encodings are host work. */
_Noreturn void gamepkg_x86_breakpoint_trap(uint32_t next_eip, uint32_t ebp);
_Noreturn void gamepkg_x86_general_protection(uint32_t eip, uint32_t ebp);
/* Protected-mode #NP/#SS/#GP with an explicit architectural16-bit error code.
 * Trusted delivery helper, not descriptor validation or a handler frame.
 * Raw register capture remains before exception-frame flag construction. */
_Noreturn void gamepkg_x86_segment_fault(uint32_t vector, uint32_t error_code,
                                         uint32_t eip, uint32_t ebp);
_Noreturn void gamepkg_x86_stack_segment(uint32_t eip, uint32_t ebp);
_Noreturn void gamepkg_x86_x87_error(uint32_t eip, uint32_t ebp);
_Noreturn void gamepkg_x86_simd_error(uint32_t eip, uint32_t ebp);
/* Concrete memory adapter supplies the resolved access/CPL error bits.
 * Full CR0/CR4/page-table walking and guest SEH remain a host integration gate.
 */
_Noreturn void gamepkg_x86_page_fault(uint32_t address, uint32_t error_code,
                                      uint32_t eip, uint32_t ebp);
uint32_t gamepkg_x86_get_mxcsr(void);
void gamepkg_x86_set_mxcsr(uint32_t value, uint32_t eip, uint32_t ebp);
/* Host arithmetic services OR newly raised status bits before fault delivery.
 * This is an internal host API, not a way for guest source to bypass LDMXCSR.
 */
void gamepkg_x86_sse_status(uint32_t exceptions, uint32_t eip, uint32_t ebp);
#endif
