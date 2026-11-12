/* Controlled generated SGDT/SIDT fixture; native table instructions are never
 * executed here. The separate ring0 corpus qualifies the legacy byte format. */
#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big-endian compiler required
#endif
_Static_assert(sizeof(void *) == 4, "actual big-endian32 target required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
/* PROGRAM */
static unsigned char ram[1024];
static GamePkgX86DispatchEntry map[128];
static size_t count;
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const struct {
  uint32_t pc, size, op, small, mode;
} rows[] = {/* ROWS */};
static void stop(uint32_t pc) {
  (void)pc;
  gamepkg_x86_dispatch_stop(g_ebp);
}
static void put32(unsigned offset, uint32_t value) {
  for (unsigned i = 0; i < 4; ++i)
    ram[offset + i] = (unsigned char)(value >> (8 * i));
}
static uint32_t reset(unsigned mode, unsigned permission, unsigned hole) {
  g_eax = 0x11223344;
  g_ecx = 0x55667788;
  g_edx = 0x99aabbcc;
  g_ebx = 17;
  g_esp = 0x1300;
  g_ebp = 0x1003;
  g_esi = 0x1003;
  g_edi = 0;
  g_eflags = 0x203;
  g_df = 0;
  g_fs_base = 0;
  if (mode == 1) {
    g_fs_base = 0xfffffff0;
    g_esi += 16;
  }
  if (mode == 2 || mode == 7) {
    g_ebx = 0xffff;
    g_esi = 0x1004;
    if (mode == 7) {
      g_fs_base = 0xfffffff0;
      g_esi += 16;
    }
  }
  if (mode == 3) {
    g_edi = 0x20000001;
    g_esi -= 40;
  }
  uint32_t address = mode == 5 ? g_esp + 6 : 0x1003;
  memset(ram, 0x5a, sizeof(ram));
  put32(0x300, 0xf0000000);
  GamePkgX86Region regions[3] = {{0x1000, sizeof(ram), permission, ram}};
  size_t n = 1;
  if (hole) {
    unsigned relative = address - 0x1000 + 5;
    regions[0] = (GamePkgX86Region){0x1000, relative, 3, ram};
    regions[1] = (GamePkgX86Region){address + 6, sizeof(ram) - relative - 1, 3,
                                    ram + relative + 1};
    n = 2;
  }
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(regions, n, 0);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  gamepkg_x86_memory_release(memory);
  return address;
}
static void expected(unsigned op, unsigned small, unsigned char bytes[6]) {
  static const unsigned char values[2][6] = {
      {0x34, 0x12, 0xef, 0xcd, 0xab, 0x89},
      {0x78, 0x56, 0x98, 0xba, 0xdc, 0xfe}};
  memcpy(bytes, values[op], 6);
  if (small)
    bytes[5] = 0;
}
typedef struct Trace {
  unsigned boundaries, retired, events, reads, mode;
  uint32_t stop_pc;
  GamePkgX86DispatchTables *mutate;
  GamePkgX86FpFault *reject_capture;
} Trace;
static int read_table(void *opaque, uint32_t op, uint32_t *base,
                      uint32_t *limit) {
  Trace *t = opaque;
  ++t->reads;
  assert(op < 2);
  if (t->mode == 1)
    return -1;
  if (t->mode == 2) {
    *limit = 65536;
    return 0;
  }
  if (t->mode == 3) {
    assert(gamepkg_x86_dispatch_store_table(op, 0, 0x1003, 0, g_ebp) == 1);
    return 0;
  }
  if (t->mode == 4)
    gamepkg_x86_general_protection(0xdead, g_ebp);
  *base = op ? 0xfedcba98 : 0x89abcdef;
  *limit = op ? 0x5678 : 0x1234;
  return 0;
}
static int boundary(void *opaque, uint32_t pc, uint32_t bp) {
  Trace *t = opaque;
  (void)bp;
  ++t->boundaries;
  if (t->reject_capture)
    assert(gamepkg_x86_dispatch_capture_checkpoint(t->reject_capture) == -1);
  if (t->mode == 6)
    assert(gamepkg_x86_dispatch_store_table(0, 0, 0x1003, 0, bp) == 1);
  if (t->mutate) {
    t->mutate->read = NULL;
    t->mutate->opaque = NULL;
    t->mutate = NULL;
  }
  return pc == t->stop_pc;
}
static int retire(void *opaque, uint32_t pc, uint32_t bp, uint32_t event) {
  Trace *t = opaque;
  (void)pc;
  (void)bp;
  if (event == GAMEPKG_X86_RETIRE_FAULT)
    ++t->events;
  else
    ++t->retired;
  if (t->mode == 7)
    assert(gamepkg_x86_dispatch_store_table(0, 0, 0x1013, 0, bp) == 1);
  return t->mode == 5 ? -1 : 0;
}
static GamePkgX86FpSource fp_source(void) {
  GamePkgX86FpSource s = {
      .x87 = &fp,
      .xmm = {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
  return s;
}
static int cpu_pending(void *opaque, const GamePkgX86InterruptState *state,
                       uint32_t kind, uint32_t pc, uint32_t bp) {
  uint32_t *expected_pc = opaque;
  (void)bp;
  assert(kind == GAMEPKG_X86_CHECKPOINT_INSTRUCTION && pc == *expected_pc &&
         state->sti_shadow == 0 && state->flags == 0x203);
  return 1;
}
static void nested_block(uint32_t pc) {
  (void)pc;
  if (gamepkg_x86_dispatch_store_table(1, 0, 0x1013, 0, g_ebp))
    return;
  gamepkg_x86_dispatch_stop(g_ebp);
}
static int nested_read(void *opaque, uint32_t op, uint32_t *base,
                       uint32_t *limit) {
  Trace *t = opaque;
  GamePkgX86DispatchEntry inner[] = {{0x20000, nested_block}};
  GamePkgX86DispatchExit result;
  assert(gamepkg_x86_dispatch_run(inner, 1, 0x20000, &result, NULL) == 3);
  Trace nested = {0};
  GamePkgX86DispatchControl control = {&nested, boundary, retire, NULL};
  GamePkgX86DispatchTables tables = {&nested, read_table};
  assert(!gamepkg_x86_dispatch_run_controlled_tables(inner, 1, 0x20000, &result,
                                                     NULL, &control, &tables) &&
         nested.reads == 1 && nested.retired == 1);
  return read_table(t, op, base, limit);
}
int main(void) {
  count = gamepkg_x86_cpu_generated_count;
  assert(count + 1 < 128);
  memcpy(map, gamepkg_x86_cpu_generated_entries, count * sizeof(*map));
  map[count++] = (GamePkgX86DispatchEntry){0xf0000000, stop};
  memset(&fp, 0x5a, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  fp.tags = 0xffff;
  memset(xmm, 0xa5, sizeof(xmm));
  GamePkgX86FpSource source = fp_source();
  unsigned checks = 0;
  for (unsigned i = 0; i < sizeof(rows) / sizeof(*rows); ++i) {
    unsigned char wanted[6];
    expected(rows[i].op, rows[i].small, wanted);
    for (unsigned scenario = 0; scenario < 4; ++scenario) {
      uint32_t address =
          reset(rows[i].mode, scenario == 1 ? 1 : 3, scenario == 2);
      unsigned char before[1024];
      memcpy(before, ram, sizeof(ram));
      Trace trace = {0};
      trace.stop_pc = rows[i].pc + rows[i].size;
      GamePkgX86DispatchControl control = {&trace, boundary, retire, NULL};
      GamePkgX86DispatchTables tables = {&trace, read_table};
      if (scenario == 3)
        trace.mutate = &tables;
      GamePkgX86DispatchExit result;
      GamePkgX87State old_fp = fp;
      uint32_t old_xmm[8][4];
      memcpy(old_xmm, xmm, sizeof(xmm));
      uint32_t old_bx = g_ebx;
      int reason = gamepkg_x86_dispatch_run_controlled_tables(
          map, count, rows[i].pc, &result, &source, &control, &tables);
      if (scenario == 1 || scenario == 2) {
        assert(reason == 1 && result.pc == rows[i].pc &&
               result.fault.core.vector == 14 &&
               result.fault.core.fault_address ==
                   (scenario == 1 ? address : address + 5) &&
               !trace.retired && trace.events == 1);
        assert(!memcmp(before, ram, sizeof(ram)));
      } else {
        assert(reason == 4 && result.pc == rows[i].pc + rows[i].size &&
               trace.retired == 1 && !trace.events && g_ebx == old_bx &&
               g_eflags == 0x203);
        assert(!memcmp(ram + address - 0x1000, wanted, 6));
        memcpy(before + address - 0x1000, wanted, 6);
        assert(!memcmp(before, ram, sizeof(ram)));
      }
      assert(trace.reads == 1 && !memcmp(&old_fp, &fp, sizeof(fp)) &&
             !memcmp(old_xmm, xmm, sizeof(xmm)));
      ++checks;
    }
    /* Real CPU provider: SGDT/SIDT retires an old STI shadow exactly before
     * the following instruction can observe a pending host request. */
    for (unsigned configured = 0; configured < 2; ++configured) {
      uint32_t address = reset(rows[i].mode, 3, 0);
      GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
      assert(cpu);
      const GamePkgX86Tables image = {{0x89abcdef, 0x1234},
                                      {0xfedcba98, 0x5678}};
      if (configured)
        assert(!gamepkg_x86_cpu_init_tables(cpu, &image));
      GamePkgX86InterruptState state = {0x203, 1, i % 4, 0, 0, 1};
      GamePkgX86CpuThread *thread = gamepkg_x86_cpu_thread_create(cpu, &state);
      assert(thread && !gamepkg_x86_cpu_enter(thread));
      GamePkgX86DispatchExit result;
      uint32_t next_pc = rows[i].pc + rows[i].size;
      GamePkgX86CpuEventSource event = {&next_pc, cpu_pending};
      int reason = gamepkg_x86_dispatch_run_cpu_events(
          map, count, rows[i].pc, &result, &source, thread, &event);
      assert(!gamepkg_x86_cpu_snapshot(thread, &state));
      if (configured) {
        assert(reason == 4 && !state.sti_shadow &&
               !memcmp(ram + address - 0x1000, wanted, 6));
        assert(!gamepkg_x86_dispatch_run_cpu(map, count, result.pc, &result,
                                             &source, thread));
      } else {
        assert(reason == 3 && state.sti_shadow == 1);
        for (unsigned b = 0; b < 6; ++b)
          assert(ram[address - 0x1000 + b] == 0x5a);
      }
      assert(!gamepkg_x86_cpu_snapshot(thread, &state));
      state.sti_shadow = 0;
      assert(!gamepkg_x86_cpu_restore(thread, &state) &&
             !gamepkg_x86_cpu_block(thread) &&
             !gamepkg_x86_cpu_thread_destroy(thread));
      gamepkg_x86_cpu_release(cpu);
      ++checks;
    }
  }
  /* Missing, erroneous and reentrant services never store or retire. */
  for (unsigned mode = 0; mode < 6; ++mode) {
    reset(0, 3, 0);
    Trace trace = {0};
    trace.mode = mode < 2 ? 0 : mode - 1;
    GamePkgX86DispatchControl control = {&trace, boundary, retire, NULL};
    GamePkgX86DispatchTables tables = {&trace, read_table};
    GamePkgX86DispatchExit result;
    int reason =
        mode == 0
            ? gamepkg_x86_dispatch_run(map, count, 0x10000, &result, &source)
        : mode == 1
            ? gamepkg_x86_dispatch_run_controlled(map, count, 0x10000, &result,
                                                  &source, &control)
            : gamepkg_x86_dispatch_run_controlled_tables(
                  map, count, 0x10000, &result, &source, &control, &tables);
    assert(reason == 3 && !trace.retired && !trace.events);
    for (unsigned b = 3; b < 9; ++b)
      assert(ram[b] == 0x5a);
  }
  /* Segment overflow is distinguished from paging and precedes any store. */
  for (unsigned mode = 0; mode < 2; ++mode) {
    reset(mode ? 4 : 0, 3, 0);
    Trace trace = {0};
    GamePkgX86DispatchControl control = {&trace, boundary, retire, NULL};
    GamePkgX86DispatchTables tables = {&trace, read_table};
    GamePkgX86DispatchExit result;
    g_esi = 0xfffffffb;
    g_ebp = 0xfffffffb;
    uint32_t pc = 0x10000 + (mode ? 4 : 0) * 0x20;
    assert(gamepkg_x86_dispatch_run_controlled_tables(
               map, count, pc, &result, &source, &control, &tables) == 1 &&
           result.fault.core.vector == (mode ? 12u : 13u) && !trace.retired &&
           trace.events == 1);
  }
  reset(0, 3, 0);
  Trace trace = {0};
  trace.stop_pc = 0x10003;
  GamePkgX86DispatchControl control = {&trace, boundary, retire, NULL};
  GamePkgX86DispatchTables tables = {&trace, nested_read};
  GamePkgX86DispatchExit result;
  assert(gamepkg_x86_dispatch_run_controlled_tables(
             map, count, 0x10000, &result, &source, &control, &tables) == 4 &&
         trace.reads == 1);
  unsigned char bytes[6];
  expected(0, 0, bytes);
  assert(!memcmp(ram + 3, bytes, 6));
  expected(1, 0, bytes);
  assert(!memcmp(ram + 0x13, bytes, 6));
  GamePkgX86DispatchExit sentinel;
  memset(&sentinel, 0xa5, sizeof(sentinel));
  result = sentinel;
  assert(gamepkg_x86_dispatch_run_controlled_tables(
             map, count, 0x10000, &result, &source, &control, NULL) == -1 &&
         !memcmp(&result, &sentinel, sizeof(result)));
  assert(gamepkg_x86_dispatch_run_controlled_tables(map, count, 0x10000,
                                                    &result, &source, &control,
                                                    (void *)&control) == -1 &&
         !memcmp(&result, &sentinel, sizeof(result)));
  assert(gamepkg_x86_dispatch_run_controlled_tables(map, count, 0x10000,
                                                    &result, &source, &control,
                                                    (void *)map) == -1 &&
         !memcmp(&result, &sentinel, sizeof(result)));
  /* A provider descriptor cannot occupy map/output/control or an FP bank. */
  union {
    GamePkgX86DispatchExit output;
    GamePkgX86DispatchTables tables;
  } alias;
  memset(&alias, 0xa5, sizeof(alias));
  alias.tables = tables;
  unsigned char old_alias[sizeof(alias)];
  memcpy(old_alias, &alias, sizeof(alias));
  assert(gamepkg_x86_dispatch_run_controlled_tables(
             map, count, 0x10000, &alias.output, &source, &control,
             &alias.tables) == -1 &&
         !memcmp(old_alias, &alias, sizeof(alias)));
  _Alignas(
      GamePkgX86DispatchTables) unsigned char unaligned[sizeof(tables) + 1];
  result = sentinel;
  assert(gamepkg_x86_dispatch_run_controlled_tables(
             map, count, 0x10000, &result, &source, &control,
             (void *)(unaligned + 1)) == -1 &&
         !memcmp(&result, &sentinel, sizeof(result)));
  union {
    GamePkgX86DispatchTables tables;
    uint32_t bank[4];
  } bank_alias;
  bank_alias.tables = tables;
  GamePkgX86FpSource overlapping = source;
  overlapping.xmm[0] = bank_alias.bank;
  assert(gamepkg_x86_dispatch_run_controlled_tables(
             map, count, 0x10000, &result, &overlapping, &control,
             &bank_alias.tables) == -1 &&
         !memcmp(&result, &sentinel, sizeof(result)));
  /* Checkpoint capture must not overwrite the original scoped descriptor. */
  union {
    GamePkgX86DispatchTables tables;
    GamePkgX86FpFault image;
  } capture_alias;
  memset(&capture_alias, 0xa5, sizeof(capture_alias));
  reset(0, 3, 0);
  trace = (Trace){0};
  trace.stop_pc = 0x10003;
  trace.reject_capture = &capture_alias.image;
  capture_alias.tables = (GamePkgX86DispatchTables){&trace, read_table};
  unsigned char old_capture[sizeof(capture_alias)];
  memcpy(old_capture, &capture_alias, sizeof(capture_alias));
  assert(gamepkg_x86_dispatch_run_controlled_tables(
             map, count, 0x10000, &result, &source, &control,
             &capture_alias.tables) == 4 &&
         !memcmp(old_capture, &capture_alias, sizeof(capture_alias)));
  for (unsigned mode = 5; mode < 8; ++mode) {
    reset(0, 3, 0);
    trace = (Trace){0};
    trace.mode = mode;
    tables = (GamePkgX86DispatchTables){&trace, read_table};
    assert(gamepkg_x86_dispatch_run_controlled_tables(
               map, count, 0x10000, &result, &source, &control, &tables) == 3);
    if (mode == 6) {
      assert(trace.reads == 0 && trace.retired == 0);
      for (unsigned b = 3; b < 9; ++b)
        assert(ram[b] == 0x5a);
    } else {
      expected(0, 0, bytes);
      assert(trace.reads == 1 && trace.retired == 1 &&
             !memcmp(ram + 3, bytes, 6));
    }
    for (unsigned b = 0x13; b < 0x19; ++b)
      assert(ram[b] == 0x5a);
  }
  for (unsigned vm = 0; vm < 2; ++vm) {
    reset(0, 3, 0);
    GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
    const GamePkgX86Tables image = {{1, 2}, {3, 4}};
    assert(cpu && !gamepkg_x86_cpu_init_tables(cpu, &image));
    GamePkgX86InterruptState state = {0x203, vm, vm ? 3u : 0u, 0, 0, 0};
    if (vm)
      state.flags |= 0x20000;
    g_eflags = state.flags;
    GamePkgX86CpuThread *thread = gamepkg_x86_cpu_thread_create(cpu, &state);
    assert(thread && !gamepkg_x86_cpu_enter(thread));
    assert(gamepkg_x86_dispatch_run_cpu(map, count, 0x10000, &result, &source,
                                        thread) == 3 &&
           result.fault.core.vector == 0);
    for (unsigned b = 3; b < 9; ++b)
      assert(ram[b] == 0x5a);
    assert(!gamepkg_x86_cpu_block(thread) &&
           !gamepkg_x86_cpu_thread_destroy(thread));
    gamepkg_x86_cpu_release(cpu);
  }
  gamepkg_x86_memory_bind(NULL);
  printf("Controlled table stores: %u generated/provider cases, byte "
         "transactions, faults, frozen/nested capability and STI retirement "
         "passed\n",
         checks);
  return 0;
}
