#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_flags.h"
#include <setjmp.h>
#include <stdlib.h>
#include <string.h>

#if defined(_MSC_VER)
#define GAMEPKG_TLS __declspec(thread)
#elif defined(__GNUC__) || defined(__clang__)
#define GAMEPKG_TLS __thread
#else
#define GAMEPKG_TLS _Thread_local
#endif

/* Match the pinned runtime's TLS ABI. Guest EBP is local to a generated C
 * function and is supplied explicitly at the faulting instruction. */
extern GAMEPKG_TLS uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi,
    g_edi;
extern GAMEPKG_TLS uint32_t g_eflags, g_fs_base;
extern GAMEPKG_TLS int g_df;

static uint32_t flags_word(void) {
  return (g_eflags & 0x003f7bd5u) | 2u | (g_df ? 0x400u : 0u);
}
uint32_t gamepkg_x86_flags_image(void) { return flags_word() & ~0x30000u; }
void gamepkg_x86_flags_load(uint32_t value, uint32_t bytes) {
  if (bytes != 2 && bytes != 4)
    abort();
  uint32_t preserved = flags_word() & (bytes == 2 ? 0x003e0000u : 0x001a0000u);
  g_eflags = preserved | (value & (bytes == 2 ? 0x7fd5u : 0x00247fd5u)) | 2u;
  g_df = (int)((g_eflags >> 10) & 1);
}

typedef struct FaultFrame {
  jmp_buf jump;
  struct FaultFrame *previous;
  GamePkgX86Fault *result;
  GamePkgX86FpSnapshot *fp;
  GamePkgX86FpSource source;
} FaultFrame;
static GAMEPKG_TLS FaultFrame *active;
/* SSE1/Pentium III profile: DAZ (bit6) was introduced after this CPU. Keep
 * guest controls separate from the host CPU's floating-point environment. */
static GAMEPKG_TLS uint32_t mxcsr = 0x1f80u;

static int run(void (*entry)(void), GamePkgX86Fault *fault,
               GamePkgX86FpSnapshot *fp, const GamePkgX86FpSource *source) {
  FaultFrame frame;
  frame.previous = active;
  frame.result = fault;
  frame.fp = fp;
  frame.source = source ? *source : (GamePkgX86FpSource){0};
  *fault = (GamePkgX86Fault){0};
  if (fp)
    memset(fp, 0, sizeof(*fp));
  active = &frame;
  /* frame fields are initialized before setjmp and not modified afterward.
   * Only the externally owned result is changed before longjmp. */
  if (setjmp(frame.jump) == 0) {
    entry();
    active = frame.previous;
    return 0;
  }
  active = frame.previous;
  return 1;
}

int gamepkg_x86_run(void (*entry)(void), GamePkgX86Fault *fault) {
  if (!entry || !fault)
    return -1;
  return run(entry, fault, NULL, NULL);
}

static int span(const void *p, size_t n, size_t alignment) {
  return p && (uintptr_t)p % alignment == 0 &&
         (uintptr_t)p <= UINTPTR_MAX - (n - 1);
}
static int overlap(const void *a, size_t an, const void *b, size_t bn) {
  uintptr_t av = (uintptr_t)a, bv = (uintptr_t)b;
  return av <= bv ? bv - av < an : av - bv < bn;
}
int gamepkg_x86_fp_source_compatible(const GamePkgX86FpSource *source,
                                     const void *output, size_t bytes) {
  if (!span(source, sizeof(*source), _Alignof(GamePkgX86FpSource)) || !bytes ||
      !span(output, bytes, 1) ||
      overlap(output, bytes, source, sizeof(*source)))
    return 0;
  if (!span(source->x87, sizeof(*source->x87), _Alignof(GamePkgX87State)) ||
      overlap(output, bytes, source->x87, sizeof(*source->x87)))
    return 0;
  for (unsigned i = 0; i < 8; ++i) {
    if (!span(source->xmm[i], 16, _Alignof(uint32_t)) ||
        overlap(output, bytes, source->xmm[i], 16) ||
        overlap(source->x87, sizeof(*source->x87), source->xmm[i], 16))
      return 0;
    for (unsigned j = 0; j < i; ++j)
      if (overlap(source->xmm[i], 16, source->xmm[j], 16))
        return 0;
  }
  return 1;
}
int gamepkg_x86_run_with_fp(void (*entry)(void), GamePkgX86FpFault *fault,
                            const GamePkgX86FpSource *source) {
  if (!entry || !span(fault, sizeof(*fault), _Alignof(GamePkgX86FpFault)) ||
      !gamepkg_x86_fp_source_compatible(source, fault, sizeof(*fault)))
    return -1;
  return run(entry, &fault->core, &fault->fp, source);
}

static void copy_x87(GamePkgX87State *d, const GamePkgX87State *s) {
  /* Host padding is not architectural state. Source/destination are disjoint.
   */
  memset(d, 0, sizeof(*d));
  for (unsigned i = 0; i < 8; ++i) {
    d->significand[i] = s->significand[i];
    d->sign_exponent[i] = s->sign_exponent[i];
  }
  d->control = s->control;
  d->status = s->status;
  d->tags = s->tags;
  d->instruction_pointer = s->instruction_pointer;
  d->data_pointer = s->data_pointer;
  d->opcode = s->opcode;
  d->instruction_selector = s->instruction_selector;
  d->data_selector = s->data_selector;
}

static void capture(FaultFrame *frame) {
  if (!frame->fp)
    return;
  copy_x87(&frame->fp->x87, frame->source.x87);
  for (unsigned i = 0; i < 8; ++i)
    memcpy(frame->fp->xmm[i], frame->source.xmm[i], 16);
}

int gamepkg_x86_capture_registers(uint32_t eip, uint32_t ebp,
                                  GamePkgX86FpFault *context,
                                  const GamePkgX86FpSource *source) {
  if (!span(context, sizeof(*context), _Alignof(GamePkgX86FpFault)) ||
      !gamepkg_x86_fp_source_compatible(source, context, sizeof(*context)) ||
      (g_df != 0 && g_df != 1))
    return -1;
  const GamePkgX86FpSource bindings = *source;
  memset(context, 0, sizeof(*context));
  context->core = (GamePkgX86Fault){.eip = eip,
                                    .eax = g_eax,
                                    .ecx = g_ecx,
                                    .edx = g_edx,
                                    .ebx = g_ebx,
                                    .esp = g_esp,
                                    .ebp = ebp,
                                    .esi = g_esi,
                                    .edi = g_edi,
                                    .eflags = g_eflags,
                                    .fs_base = g_fs_base,
                                    .direction = (uint32_t)g_df,
                                    .mxcsr = mxcsr};
  copy_x87(&context->fp.x87, bindings.x87);
  for (unsigned i = 0; i < 8; ++i)
    memcpy(context->fp.xmm[i], bindings.xmm[i], 16);
  return 0;
}

int gamepkg_x86_restore_fault_registers(const GamePkgX86FpFault *fault,
                                        const GamePkgX86FpTarget *target) {
  if (!span(fault, sizeof(*fault), _Alignof(GamePkgX86FpFault)) ||
      !span(target, sizeof(*target), _Alignof(GamePkgX86FpTarget)) ||
      overlap(fault, sizeof(*fault), target, sizeof(*target)))
    return -1;
  const GamePkgX86FpTarget bindings = *target;
  if (fault->core.mxcsr & ~0xffbfu || fault->core.direction > 1 ||
      !span(bindings.x87, sizeof(*bindings.x87), _Alignof(GamePkgX87State)) ||
      overlap(fault, sizeof(*fault), bindings.x87, sizeof(*bindings.x87)) ||
      overlap(target, sizeof(*target), bindings.x87, sizeof(*bindings.x87)))
    return -1;
  for (unsigned i = 0; i < 8; ++i) {
    if (!span(bindings.xmm[i], 16, _Alignof(uint32_t)) ||
        overlap(fault, sizeof(*fault), bindings.xmm[i], 16) ||
        overlap(target, sizeof(*target), bindings.xmm[i], 16) ||
        overlap(bindings.x87, sizeof(*bindings.x87), bindings.xmm[i], 16))
      return -1;
    for (unsigned j = 0; j < i; ++j)
      if (overlap(bindings.xmm[i], 16, bindings.xmm[j], 16))
        return -1;
  }
  /* All checks precede mutation. No arithmetic, allocation or fault-capable
   * guest operations occur during commit. EIP remains an explicit dispatcher
   * input, never a host function pointer or an unchecked native jump. */
  copy_x87(bindings.x87, &fault->fp.x87);
  for (unsigned i = 0; i < 8; ++i)
    memcpy(bindings.xmm[i], fault->fp.xmm[i], 16);
  g_eax = fault->core.eax;
  g_ecx = fault->core.ecx;
  g_edx = fault->core.edx;
  g_ebx = fault->core.ebx;
  g_esp = fault->core.esp;
  g_ebp = fault->core.ebp;
  g_esi = fault->core.esi;
  g_edi = fault->core.edi;
  g_eflags = fault->core.eflags;
  g_fs_base = fault->core.fs_base;
  g_df = (int)fault->core.direction;
  mxcsr = fault->core.mxcsr;
  return 0;
}

static _Noreturn void deliver(uint32_t vector, uint32_t eip, uint32_t ebp,
                              uint32_t address, uint32_t error_code) {
  FaultFrame *frame = active;
  /* Missing mandatory host integration is a programming error. A fault must
   * never silently return or fabricate a successful quotient. */
  if (!frame)
    abort();
  capture(frame);
  *frame->result = (GamePkgX86Fault){.vector = vector,
                                     .eip = eip,
                                     .eax = g_eax,
                                     .ecx = g_ecx,
                                     .edx = g_edx,
                                     .ebx = g_ebx,
                                     .esp = g_esp,
                                     .ebp = ebp,
                                     .esi = g_esi,
                                     .edi = g_edi,
                                     .eflags = g_eflags,
                                     .fs_base = g_fs_base,
                                     .direction = (uint32_t)g_df,
                                     .mxcsr = mxcsr,
                                     .fault_address = address,
                                     .error_code = error_code};
  g_ebp = ebp;
  longjmp(frame->jump, 1);
}

_Noreturn void gamepkg_x86_divide_error(uint32_t eip, uint32_t ebp) {
  deliver(0, eip, ebp, 0, 0);
}
_Noreturn void gamepkg_x86_breakpoint_trap(uint32_t next_eip, uint32_t ebp) {
  deliver(3, next_eip, ebp, 0, 0);
}
_Noreturn void gamepkg_x86_stack_segment(uint32_t eip, uint32_t ebp) {
  deliver(12, eip, ebp, 0, 0);
}

_Noreturn void gamepkg_x86_general_protection(uint32_t eip, uint32_t ebp) {
  deliver(13, eip, ebp, 0, 0);
}
_Noreturn void gamepkg_x86_segment_fault(uint32_t vector, uint32_t error_code,
                                         uint32_t eip, uint32_t ebp) {
  if ((vector != 11 && vector != 12 && vector != 13) || error_code > 65535)
    abort();
  deliver(vector, eip, ebp, 0, error_code);
}
_Noreturn void gamepkg_x86_simd_error(uint32_t eip, uint32_t ebp) {
  deliver(19, eip, ebp, 0, 0);
}
uint32_t gamepkg_x86_get_mxcsr(void) { return mxcsr; }
void gamepkg_x86_set_mxcsr(uint32_t value, uint32_t eip, uint32_t ebp) {
  if (value & ~0xffbfu)
    gamepkg_x86_general_protection(eip, ebp);
  mxcsr = value;
}
void gamepkg_x86_sse_status(uint32_t exceptions, uint32_t eip, uint32_t ebp) {
  /* Only the operation's newly raised conditions trigger faults. Loading
   * old sticky flags with their masks clear does not itself raise #XM. */
  if (exceptions & ~0x3fu)
    abort();
  mxcsr |= exceptions;
  if (exceptions & ~(mxcsr >> 7u))
    gamepkg_x86_simd_error(eip, ebp);
}

_Noreturn void gamepkg_x86_x87_error(uint32_t eip, uint32_t ebp) {
  deliver(16, eip, ebp, 0, 0);
}

_Noreturn void gamepkg_x86_page_fault(uint32_t address, uint32_t error_code,
                                      uint32_t eip, uint32_t ebp) {
  if (error_code & ~31u)
    abort();
  deliver(14, eip, ebp, address, error_code);
}
