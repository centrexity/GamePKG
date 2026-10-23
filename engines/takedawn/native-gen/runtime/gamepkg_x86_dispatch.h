#ifndef GAMEPKG_X86_DISPATCH_H
#define GAMEPKG_X86_DISPATCH_H
#include "gamepkg_x86_far_memory.h"
#include "gamepkg_x86_fault.h"
#include <stddef.h>
#include <stdint.h>
/* Reviewed immutable host map, never guest-provided native pointers. Entries
 * name exact qualified instruction boundaries, not arbitrary address ranges.
 * Every block receives a checked entry PC and returns after transfer/stop. */
typedef struct GamePkgX86DispatchEntry {
  uint32_t address;
  void (*block)(uint32_t);
} GamePkgX86DispatchEntry;
typedef struct GamePkgX86DispatchExit {
  uint32_t pc;
  GamePkgX86FpFault fault;
} GamePkgX86DispatchExit;
/* Reviewed controlled-code profile. Callbacks are immutable trusted host code,
 * not guest function pointers. Descriptor is frozen before entry. Both preserve
 * live GPR/FP/RAM bindings and return normally (no unrelated unwinding/faults).
 * boundary runs before an instruction or at a completed REP interior prefix;
 * checkpoint_kind distinguishes those scopes. 0=continue,1=host
 * stop,other=error. retire runs AFTER a completed instruction or detected
 * exception(event=1); events2/3 report completed CLI/STI without generic
 * shadow expiration. 0=success,other=host error. It must not yield/deliver an
 * IRQ mid-commit. Opaque storage must outlive execution and not alias
 * map/output/registers.
 */
enum {
  GAMEPKG_X86_RETIRE_OTHER = 0,
  GAMEPKG_X86_RETIRE_FAULT = 1,
  GAMEPKG_X86_RETIRE_CLI = 2,
  GAMEPKG_X86_RETIRE_STI = 3
};
typedef struct GamePkgX86DispatchControl {
  void *opaque;
  int (*boundary)(void *, uint32_t pc, uint32_t ebp);
  int (*retire)(void *, uint32_t pc, uint32_t ebp, uint32_t event);
  /* Optional reviewed virtual-CPU instruction service. op0=CLI,1=STI.
   * 0=committed CPU image, writes flags;1=guest GP with unchanged image;
   * other=host error. Preserve all other guest banks; no yielding/faulting.
   * Missing service fails before a controlled CLI/STI executes. */
  int (*interrupt_step)(void *, uint32_t op, uint32_t *flags);
} GamePkgX86DispatchControl;
/* Scoped legacy protected32/pre-UMIP table provider, copied before entry.
 * read op0=GDTR/1=IDTR returns0 and copies logical base/limit, other=host
 * error. Pure host callback: preserve guest banks/ownership/bindings; never
 * yield or fault. Storage is live and disjoint from map/output/control/FP
 * bindings. It creates neither RAM capability nor descriptor/segment-cache
 * state. */
typedef struct GamePkgX86DispatchTables {
  void *opaque;
  int (*read)(void *, uint32_t op, uint32_t *base, uint32_t *limit);
} GamePkgX86DispatchTables;
/* Scoped reviewed code-cache capability, frozen separately from the original
 * four-field controller ABI. read is a pure nonyielding/nonfaulting snapshot;
 * code.offset/vector/error/mark_accessed are zero. cpl matches visible CS.
 * commit copies a fully resolved code proposal without RAM/PC/retirement or
 * yielding; rejection leaves caches unchanged. Both callbacks preserve all
 * live raw banks and bindings. Only reviewed host code supplies this object. */
typedef struct GamePkgX86DispatchCodeImage {
  GamePkgX86FarTables tables;
  GamePkgX86CodeTarget code;
  uint32_t cpl;
} GamePkgX86DispatchCodeImage;
typedef struct GamePkgX86DispatchCode {
  void *opaque;
  int (*read)(void *, GamePkgX86DispatchCodeImage *);
  int (*commit)(void *, const GamePkgX86CodeTarget *);
} GamePkgX86DispatchCode;
/* Requires all three capabilities. Flat-base/default32 generated code only;
 * a legitimate transfer to another code profile commits CS/PC and retires,
 * then stops before an unqualified target instruction executes. Nested runs
 * have only their own scoped capability; no host segment is loaded. */
int gamepkg_x86_dispatch_run_controlled_code(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    const GamePkgX86DispatchControl *control,
    const GamePkgX86DispatchTables *tables, const GamePkgX86DispatchCode *code);
/* Generated instruction boundary with qualified byte length. Old runs without
 * code capability preserve their old checkpoint behavior. With capability,
 * poll/yield first, then validate current profile and the complete cached-CS
 * span before executing the instruction. This is not RAM execute permission,
 * an instruction decoder, a byte-integrity check or a page-table walker. */
int gamepkg_x86_dispatch_instruction(uint32_t pc, uint32_t bytes, uint32_t ebp);
/* Exact qualified direct far-JMP service; generated blocks return immediately
 * after this call. Resolve selectors/descriptor/A under checked RAM, commit
 * CS then target PC/current x87-CS binding, retire once and transfer. Faults
 * before commit retain old CS/PC; later target-fetch faults retain new CS.
 * Completed cache/PC persist if the retirement callback rejects. */
void gamepkg_x86_dispatch_far_jump(uint32_t selector, uint32_t offset,
                                   uint32_t operand16, uint32_t ebp);
/* Requires both descriptors. Existing run/control ABI remains unchanged.
 * Missing service during a table instruction stops with reason3, before RAM.
 * The provider is scoped to this run; nested dispatch must bind its own. */
int gamepkg_x86_dispatch_run_controlled_tables(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    const GamePkgX86DispatchControl *control,
    const GamePkgX86DispatchTables *tables);
/* Exact controlled SGDT/SIDT service. segment0=flat data,1=stack,2=FS.
 * Validates the full segment span and commits six checked LE bytes, then
 * retires once as OTHER. Faults leave through the live guest boundary.
 * Returns1 on protocol stop,0 after retirement; provider never does RAM I/O. */
int gamepkg_x86_dispatch_store_table(uint32_t op, uint32_t operand16,
                                     uint32_t offset, uint32_t segment,
                                     uint32_t ebp);
/* Controlled generated maps only. Same returns as run, plus4=host boundary
 * event before instruction execution. No event/IRQ is acknowledged/injected.
 * The host owns resumption/handler policy and a shared CPU reservation. */
int gamepkg_x86_dispatch_run_controlled(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    const GamePkgX86DispatchControl *control);
enum { GAMEPKG_X86_CHECKPOINT_INSTRUCTION = 0, GAMEPKG_X86_CHECKPOINT_REP = 1 };
int gamepkg_x86_dispatch_boundary(uint32_t pc, uint32_t ebp);
/* Generated controlled strings only, after a completed element when another
 * remains and qualifies. It does not retire an instruction or accept an IRQ. */
int gamepkg_x86_dispatch_rep_checkpoint(uint32_t pc, uint32_t ebp);
/* Trusted boundary callback scope only; returns the kind above, -1 outside it.
 * Nested dispatch restores the outer scope. Retirement is not a checkpoint. */
int gamepkg_x86_dispatch_checkpoint_kind(void);
/* Complete nonfaulting raw snapshot from the frozen FP bindings supplied at
 * run entry. Requires those bindings and the active boundary callback scope.
 * Returns0=success,-1=invalid scope/bindings/output with unchanged output.
 * Output must be live, disjoint host storage, not
 * map/result/descriptor/register storage. No invented exception, RF change, IRQ
 * acknowledgement or frame push. The host separately retains kind and virtual
 * CPU/segment state for delivery. */
int gamepkg_x86_dispatch_capture_checkpoint(GamePkgX86FpFault *output);
int gamepkg_x86_dispatch_retire(uint32_t ebp);
/* Exact controlled CLI/STI only. CPU service commits IF/VIF/shadow and then
 * retirement event2/3 reports completion without expiring the new STI shadow.
 * Missing/invalid service returns a protocol stop; GP is raised at this site.
 * Returns0=completed,1=host protocol stop; genuine GP leaves via fault
 * boundary. */
int gamepkg_x86_dispatch_interrupt(uint32_t op, uint32_t ebp);
/* Returns0=explicit trusted host stop,1=real guest fault,2=unmapped target,
 * 3=host block protocol error,-1=invalid host arguments (output unchanged).
 * Unmapped targets are not fabricated CPU faults: the host must resolve a
 * kernel/import/entry or establish an actual fetch exception. Live registers
 * retain all completed work. Fault FP state is captured when source!=NULL.
 * Sorted unique map1..1048576, live immutable storage until exit; output must
 * not alias it or FP bindings. No allocation, native recursion, instruction
 * count limit or metering. Retain the process time/memory watchdog.
 * Resumption reenters a qualified PC with the current/restored register file;
 * caller owns RAM repair, guest SEH selection and native hook semantics. */
int gamepkg_x86_dispatch_run(const GamePkgX86DispatchEntry *map, size_t count,
                             uint32_t pc, GamePkgX86DispatchExit *result,
                             const GamePkgX86FpSource *source);
/* Emitted label site and trusted RAM/MMIO callback context. Use the explicit
 * site query rather than stale legacy global metadata. TLS PC is host state. */
extern _Thread_local uint32_t gamepkg_x86_dispatch_pc;
int gamepkg_x86_dispatch_site(uint32_t *pc, uint32_t *ebp);
void gamepkg_x86_dispatch_transfer(uint32_t target, uint32_t ebp);
void gamepkg_x86_dispatch_stop(uint32_t ebp);
void gamepkg_x86_dispatch_invalid(void);
/* Checked flat32 RAM stack/indirect source helpers. CALL/RET commit ESP only
 * after the complete store/load succeeds. Span crossing raises #SS(0).
 * Return target is guest data, not a native function pointer. */
uint32_t gamepkg_x86_dispatch_load(uint32_t offset, uint32_t segment,
                                   uint32_t eip, uint32_t ebp);
void gamepkg_x86_dispatch_call(uint32_t return_pc, uint32_t eip, uint32_t ebp);
/* Controlled near transfers: validate the prospective offset against the
 * current cached CS before retirement/LOOP-counter/stack commit. Only check
 * TAKEN branches. EIP==limit is a valid target, independently of a later
 * instruction's complete fetch span. Return1 is a host protocol stop; actual
 * target overflow raises GP0 at the SOURCE site with unchanged guest state.
 * Missing code capability retains the legacy flat-profile behavior. */
int gamepkg_x86_dispatch_check_target(uint32_t target, uint32_t eip,
                                      uint32_t ebp);
/* Resolve the CALL operand before entry; stack admission precedes the target
 * check; the check precedes the return-address push. Return1 stops without a push.
 * The old call helper is retained for existing host/ordinary ABI callers. */
int gamepkg_x86_dispatch_call_to(uint32_t target, uint32_t return_pc,
                                 uint32_t eip, uint32_t ebp);
/* RET reads the complete return offset, checks a bound active CS capability,
 * then commits ESP including cleanup. A read/target fault preserves ESP.
 * Out-of-dispatch host import adapters still need explicit CPU validation. */
uint32_t gamepkg_x86_dispatch_ret(uint32_t bytes, uint32_t extra, uint32_t eip,
                                  uint32_t ebp);
#endif
