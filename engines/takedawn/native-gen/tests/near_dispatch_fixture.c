/* Synthetic near-transfer replay. Native #UD rows indicate the branch completed
 * and stopped at a subsequent sentinel; no #UD emulation is claimed here. */
#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big-endian compiler required
#endif
_Static_assert(sizeof(void *) == 4, "big-endian32 required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
/* PROGRAM */
static void (*const functions[])(uint32_t) = {/* FUNCTIONS */};
static const struct {
  uint32_t size;
  unsigned char bytes[8];
} encodings[] = {/* ENCODINGS */};
typedef struct Row {
  uint32_t kind, size, target, mode, sp, cx, flags, window, vector, error, ip,
      after_sp, after_cx, eax, after_flags, address;
  unsigned char bytes[8], stack[32];
} Row;
static unsigned char stack[4096], operand[4];
static GamePkgX86CpuThread *thread;
static GamePkgX86FpSource fp_source;
static uint32_t xmm[8][4];
static unsigned others, faults, reads, fail_read, executions;
static void put(unsigned char *p, uint32_t n) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(n >> (8 * i));
}
static void initialize_stack(const Row *r) {
  memset(stack, 0xa5, sizeof(stack));
  put(operand, r->target);
  if (r->sp >= 0x11001000 && r->sp <= 0x11001ffc)
    put(stack + r->sp - 0x11001000, r->target);
  if (r->sp >= 0x11001004 && r->sp <= 0x11002000)
    put(stack + r->sp - 4 - 0x11001000, r->target);
}
static int boundary(void *p, uint32_t pc, uint32_t bp) {
  (void)p;
  (void)pc;
  (void)bp;
  return gamepkg_x86_cpu_checkpoint(thread, g_eflags);
}
static int retire(void *p, uint32_t pc, uint32_t bp, uint32_t event) {
  (void)p;
  assert((pc == 0 || (pc == 127 && event == GAMEPKG_X86_RETIRE_FAULT)) &&
         bp == 0x5678);
  if (event == GAMEPKG_X86_RETIRE_OTHER)
    ++others;
  else {
    assert(event == GAMEPKG_X86_RETIRE_FAULT);
    ++faults;
  }
  return gamepkg_x86_cpu_retire_other(thread, g_eflags);
}
static int tables(void *p, uint32_t op, uint32_t *base, uint32_t *limit) {
  (void)p;
  assert(op < 2);
  *base = *limit = 0;
  return 0;
}
static int read_code(void *p, GamePkgX86DispatchCodeImage *out) {
  (void)p;
  ++reads;
  if (fail_read && reads == fail_read)
    return -1;
  GamePkgX86CpuExecution state;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &state));
  *out = (GamePkgX86DispatchCodeImage){
      .tables = {0, 0, 0, 0, 0},
      .code = {.selector = state.code.cs_selector,
               .base = state.code.cs_base,
               .limit = state.code.cs_limit,
               .attributes = state.code.cs_attributes},
      .cpl = state.interrupt.cpl};
  return 0;
}
static int no_commit(void *p, const GamePkgX86CodeTarget *code) {
  (void)p;
  (void)code;
  assert(!"near transfer must not commit CS");
  return -1;
}
static void crossing_fetch(uint32_t pc) {
  assert(pc == 127);
  if (gamepkg_x86_dispatch_instruction(pc, 2, g_ebp))
    return;
  assert(!"boundary sentinel must fault before execution");
}
static void execute(const Row *r, unsigned actual) {
  assert(r->kind < 23 &&
         (r->target == 64 || r->target == 128 || r->target == 127) &&
         r->mode < 6 && r->window <= sizeof(stack) - 32);
  unsigned n = r->kind * 3 + (r->target == 64 ? 0 : r->target == 128 ? 1 : 2);
  assert(r->size == encodings[n].size &&
         !memcmp(r->bytes, encodings[n].bytes, 8));
  initialize_stack(r);
  const GamePkgX86Region regions[] = {
      {0x11001000, sizeof(stack), r->mode == 1 ? 1 : 3, stack},
      {0x12000000, sizeof(operand), 1, operand}};
  GamePkgX86Memory *memory =
      gamepkg_x86_memory_create(regions, r->mode == 4 ? 1 : 2, 1);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  gamepkg_x86_memory_release(memory);
  g_eax = 0x11223344;
  g_ecx = r->cx;
  g_edx = 0x44556677;
  g_ebx = 0x55667788;
  g_esp = r->sp;
  g_ebp = 0x5678;
  g_esi = 0x12000000;
  g_edi = r->target;
  g_eflags = r->flags;
  g_fs_base = 0;
  g_df = 0;
  GamePkgX87State *fp = gamepkg_x86_x87_state();
  memset(fp, 0xa5, sizeof(*fp));
  fp->control = 0x37e;
  fp->status = 0x8081;
  memset(xmm, 0x5a, sizeof(xmm));
  fp_source.x87 = fp;
  for (unsigned i = 0; i < 8; ++i)
    fp_source.xmm[i] = xmm[i];
  gamepkg_x86_x87_bind_segments(7, 0x2b, 0x2b, 0);
  GamePkgX86FpFault before, after;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &fp_source));
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  const GamePkgX86Tables image = {{0, 0}, {0, 0}};
  const GamePkgX86InterruptState state = {r->flags, 1, 3, 0, 0, 1};
  const GamePkgX86CpuCodeState code = {7, 0, 127, 0x4fb, 0, 0, 0, 0, 0};
  assert(!gamepkg_x86_cpu_init_tables(cpu, &image));
  thread = gamepkg_x86_cpu_thread_create(cpu, &state);
  assert(thread && !gamepkg_x86_cpu_enter(thread) &&
         !gamepkg_x86_cpu_init_code(thread, &code));
  const GamePkgX86DispatchControl control = {NULL, boundary, retire, NULL};
  const GamePkgX86DispatchTables table_source = {NULL, tables};
  const GamePkgX86DispatchCode code_source = {NULL, read_code, no_commit};
  const GamePkgX86DispatchEntry entry[] = {{0, functions[n]},
                                           {127, crossing_fetch}};
  GamePkgX86DispatchExit out;
  others = faults = reads = 0;
  int reason = actual ? gamepkg_x86_dispatch_run_cpu_code_events(
                            entry, 2, 0, &out, &fp_source, thread, NULL)
                      : gamepkg_x86_dispatch_run_controlled_code(
                            entry, 2, 0, &out, &fp_source, &control,
                            &table_source, &code_source);
  ++executions;
  if (r->vector == UINT32_MAX) {
    assert(reason == 3 && !out.fault.core.vector && out.pc == 0 && !others &&
           !faults);
  } else if (r->vector == 6) {
    assert(reason == 2 && out.pc == r->ip && !out.fault.core.vector);
    if (!actual)
      assert(others == 1 && !faults);
  } else {
    if (reason != 1 || out.fault.core.vector != r->vector ||
        out.fault.core.error_code != r->error || out.pc != r->ip)
      fprintf(
          stderr,
          "near kind=%u mode=%u target=%u cx=%08x flags=%x expected vector=%u "
          "err=%u ip=%x; got reason=%d vector=%u err=%u ip=%x ESP=%x ECX=%x\n",
          r->kind, r->mode, r->target, r->cx, r->flags, r->vector, r->error,
          r->ip, reason, out.fault.core.vector, out.fault.core.error_code,
          out.pc, g_esp, g_ecx);
    assert(reason == 1 && out.pc == r->ip &&
           out.fault.core.vector == r->vector &&
           out.fault.core.error_code == r->error &&
           out.fault.core.fault_address == r->address &&
           !memcmp(&out.fault.fp, &before.fp, sizeof(before.fp)));
    if (!actual)
      assert(others == (r->ip == 127 ? 1u : 0u) && faults == 1);
  }
  assert(g_esp == r->after_sp && g_ecx == r->after_cx && g_eax == r->eax &&
         g_eflags == (r->after_flags & ~0x10000u) && g_ebx == 0x55667788 &&
         g_edx == 0x44556677 && g_esi == 0x12000000 && g_edi == r->target &&
         !g_df);
  assert(!memcmp(stack + r->window, r->stack, 32));
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &fp_source) &&
         !memcmp(&after.fp, &before.fp, sizeof(before.fp)));
  GamePkgX86CpuExecution cpu_after;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &cpu_after));
  assert(!memcmp(&cpu_after.code, &code, sizeof(code)) &&
         cpu_after.interrupt.flags == r->flags &&
         cpu_after.interrupt.sti_shadow == (r->vector == UINT32_MAX ? 1u : 0u));
  assert(!gamepkg_x86_cpu_retire_other(thread, g_eflags) &&
         !gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
}
static Row fixed(unsigned kind, unsigned target, unsigned mode, uint32_t count,
                 uint32_t flags, unsigned vector, unsigned error, uint32_t ip,
                 uint32_t after_sp, uint32_t after_cx, uint32_t address) {
  Row r = {.kind = kind,
           .target = target,
           .mode = mode,
           .sp = mode == 3   ? 1
                 : mode == 5 ? 0x11000ffc
                             : 0x11001040,
           .cx = count,
           .flags = flags,
           .window = (mode == 3 || mode == 5) ? 0 : 56,
           .vector = vector,
           .error = error,
           .ip = ip,
           .after_sp = after_sp,
           .after_cx = after_cx,
           .eax = 0x11223344,
           .after_flags = flags | 0x10000,
           .address = address};
  unsigned n = kind * 3 + (target == 64 ? 0 : target == 128 ? 1 : 2);
  r.size = encodings[n].size;
  memcpy(r.bytes, encodings[n].bytes, 8);
  initialize_stack(&r);
  if ((vector == 6 || ip == 127) && kind < 6)
    put(stack + 60, r.size);
  memcpy(r.stack, stack + r.window, 32);
  return r;
}
static void units(void) {
  Row rows[] = {
      fixed(1, 128, 0, 2, 0x202, 13, 0, 0, 0x11001040, 2, 0),
      fixed(1, 128, 1, 2, 0x202, 14, 7, 0, 0x11001040, 2, 0x1100103c),
      fixed(1, 64, 1, 2, 0x202, 14, 7, 0, 0x11001040, 2, 0x1100103c),
      fixed(1, 64, 0, 2, 0x202, 6, 0, 64, 0x1100103c, 2, 0),
      fixed(2, 128, 4, 2, 0x202, 14, 4, 0, 0x11001040, 2, 0x12000000),
      fixed(8, 128, 4, 2, 0x202, 14, 4, 0, 0x11001040, 2, 0x12000000),
      fixed(1, 64, 3, 2, 0x202, 12, 0, 0, 1, 2, 0),
      fixed(1, 128, 3, 2, 0x202, 12, 0, 0, 1, 2, 0),
      fixed(15, 128, 0, 2, 0x202, 13, 0, 0, 0x11001040, 2, 0),
      fixed(15, 128, 0, 1, 0x202, 6, 0, 2, 0x11001040, 0, 0),
      fixed(16, 128, 0, 2, 0x202, 6, 0, 2, 0x11001040, 1, 0),
      fixed(17, 128, 0, 2, 0x242, 6, 0, 2, 0x11001040, 1, 0),
      fixed(18, 128, 0, 0x10001, 0x202, 6, 0, 3, 0x11001040, 0x10000, 0),
      fixed(18, 128, 0, 0, 0x202, 13, 0, 0, 0x11001040, 0, 0),
      fixed(10, 128, 0, 2, 0x202, 6, 0, 2, 0x11001040, 2, 0),
      fixed(10, 128, 0, 2, 0x242, 13, 0, 0, 0x11001040, 2, 0),
      fixed(14, 128, 0, 0x10000, 0x202, 13, 0, 0, 0x11001040, 0x10000, 0),
      fixed(19, 128, 0, 2, 0x202, 13, 0, 0, 0x11001040, 2, 0),
      fixed(20, 128, 0, 2, 0x202, 13, 0, 0, 0x11001040, 2, 0),
      fixed(21, 128, 0, 2, 0x202, 13, 0, 0, 0x11001040, 2, 0),
      fixed(22, 128, 0, 2, 0x202, 13, 0, 0, 0x11001040, 2, 0),
      fixed(22, 64, 0, 2, 0x202, 6, 0, 64, 0x1100104a, 2, 0),
      fixed(19, 128, 5, 2, 0x202, 14, 4, 0, 0x11000ffc, 2, 0x11000ffc),
      fixed(1, 127, 0, 2, 0x202, 13, 0, 127, 0x1100103c, 2, 0),
      fixed(15, 127, 0, 2, 0x202, 13, 0, 127, 0x11001040, 1, 0),
      fixed(22, 127, 0, 2, 0x202, 13, 0, 127, 0x1100104a, 2, 0)};
  for (unsigned i = 0; i < sizeof(rows) / sizeof(*rows); ++i)
    for (unsigned actual = 0; actual < 2; ++actual)
      execute(rows + i, actual);
  Row protocol = fixed(19, 64, 0, 2, 0x202, UINT32_MAX, 0, 0, 0x11001040, 2, 0);
  fail_read = 2;
  execute(&protocol, 0);
  fail_read = 0;
}
static uint32_t word(FILE *f) {
  uint32_t n = 0;
  for (unsigned i = 0; i < 4; ++i) {
    int c = fgetc(f);
    assert(c != EOF);
    n |= (uint32_t)c << (i * 8);
  }
  return n;
}
int main(int argc, char **argv) {
  assert(argc == 1 || argc == 2);
  units();
  if (argc == 2) {
    FILE *f = fopen(argv[1], "rb");
    assert(f);
    unsigned char magic[8];
    assert(fread(magic, 1, 8, f) == 8 && !memcmp(magic, "GPKNEAR1", 8));
    uint32_t count = word(f);
    assert(count == 5796);
    for (uint32_t i = 0; i < count; ++i) {
      Row r;
      uint32_t *fields[] = {&r.kind,     &r.size,  &r.target,      &r.mode,
                            &r.sp,       &r.cx,    &r.flags,       &r.window,
                            &r.vector,   &r.error, &r.ip,          &r.after_sp,
                            &r.after_cx, &r.eax,   &r.after_flags, &r.address};
      for (unsigned j = 0; j < 16; ++j)
        *fields[j] = word(f);
      assert(fread(r.bytes, 1, 8, f) == 8 && fread(r.stack, 1, 32, f) == 32 &&
             (r.vector == 6 || r.vector == 12 || r.vector == 13 ||
              r.vector == 14));
      for (unsigned actual = 0; actual < 2; ++actual)
        execute(&r, actual);
    }
    assert(fgetc(f) == EOF && !fclose(f));
  }
  printf("Near transfers: %u generated owner/control/CPU executions, native "
         "fault priority/ESP-ECX rollback/stack bytes and pending x87-MMX-XMM "
         "preservation passed; pointer_bits=%u\n",
         executions, (unsigned)(8 * sizeof(void *)));
  return 0;
}
