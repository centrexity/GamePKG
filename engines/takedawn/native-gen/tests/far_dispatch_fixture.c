/* Synthetic generated far-JMP integration; no native far JMP or game media. */
#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big-endian compiler required
#endif
_Static_assert(sizeof(void *) == 4, "actual big-endian32 required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
/* PROGRAM */
static const struct Row {
  uint32_t pc, bytes, selector, target, small;
} rows[] = {/* ROWS */};
static GamePkgX86DispatchEntry map[128];
static size_t map_count;
static unsigned char gdt[32], ldt[24];
static uint32_t xmm[8][4];
static GamePkgX86FpSource source;
static GamePkgX86FpFault raw_before;
static GamePkgX86CpuThread *thread;
static unsigned event_count, guarded_execution, cpu_cases;
static const struct Row *row;
static unsigned char *selected;
static uint32_t descriptor_address;
static void finish(uint32_t pc) {
  (void)pc;
  gamepkg_x86_dispatch_stop(g_ebp);
}
static void guarded(uint32_t pc) {
  if (gamepkg_x86_dispatch_instruction(pc, 5, g_ebp))
    return;
  ++guarded_execution;
  gamepkg_x86_dispatch_stop(g_ebp);
}
static int order(const void *a, const void *b) {
  uint32_t av = ((const GamePkgX86DispatchEntry *)a)->address;
  uint32_t bv = ((const GamePkgX86DispatchEntry *)b)->address;
  return av < bv ? -1 : av != bv;
}
static void maps(void) {
  map_count = gamepkg_x86_cpu_generated_count;
  assert(map_count + 3 < sizeof(map) / sizeof(*map));
  memcpy(map, gamepkg_x86_cpu_generated_entries, map_count * sizeof(*map));
  map[map_count++] = (GamePkgX86DispatchEntry){0xf0000000, finish};
  map[map_count++] = (GamePkgX86DispatchEntry){0x12000, guarded};
  map[map_count++] = (GamePkgX86DispatchEntry){0x30000, guarded};
  qsort(map, map_count, sizeof(*map), order);
}
static void descriptor(unsigned char *p, uint32_t base, uint32_t limit,
                       unsigned default32, unsigned access) {
  uint32_t flags = default32 ? 4u : 0u;
  if (limit > 0xfffff) {
    assert((limit & 4095) == 4095);
    limit >>= 12;
    flags |= 8;
  }
  uint32_t lo = (limit & 65535) | (base << 16);
  uint32_t hi = ((base >> 16) & 255) | (access << 8) | (limit & 0xf0000) |
                (flags << 20) | (base & 0xff000000);
  for (unsigned i = 0; i < 4; ++i) {
    p[i] = (unsigned char)(lo >> (8 * i));
    p[i + 4] = (unsigned char)(hi >> (8 * i));
  }
}
static void raw_reset(void) {
  g_eax = 0xaabbccdd;
  g_ecx = 0xf0000000;
  g_edx = 0x1122;
  g_ebx = 17;
  g_esp = 0x9000;
  g_ebp = 0x8000;
  g_esi = 0x1234;
  g_edi = 0x5678;
  g_eflags = 0x202;
  g_fs_base = 0x80000000;
  g_df = 0;
  GamePkgX87State *fp = gamepkg_x86_x87_state();
  memset(fp, 0xa5, sizeof(*fp));
  fp->control = 0x37e;
  fp->status = 0x8081;
  memset(xmm, 0x5a, sizeof(xmm));
  source.x87 = fp;
  for (unsigned i = 0; i < 8; ++i)
    source.xmm[i] = xmm[i];
  gamepkg_x86_x87_bind_segments(8, 0x10, 0x18, 0x20);
  gamepkg_x86_x87_data_segment(2, 0x1000);
  assert(!gamepkg_x86_capture_registers(0, 0, &raw_before, &source));
  event_count = guarded_execution = 0;
}
static void raw_preserved(void) {
  GamePkgX86FpFault after;
  assert(!gamepkg_x86_capture_registers(0, 0, &after, &source));
  assert(!memcmp(&after.fp, &raw_before.fp, sizeof(after.fp)));
  assert(g_ecx == 0xf0000000 && g_edx == 0x1122 && g_ebx == 17 &&
         g_esp == 0x9000 && g_esi == 0x1234 && g_edi == 0x5678 &&
         g_fs_base == 0x80000000 && g_eflags == 0x202 && !g_df);
}
static int pending(void *opaque, const GamePkgX86InterruptState *state,
                   uint32_t kind, uint32_t pc, uint32_t ebp) {
  (void)opaque;
  assert(state->flags == 0x202 && !state->sti_shadow && !kind &&
         pc == row->target && ebp == 0x8000);
  GamePkgX86CpuExecution image;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &image));
  assert(image.code.cs_selector == row->selector);
  GamePkgX86FpFault saved;
  assert(!gamepkg_x86_dispatch_capture_checkpoint(&saved));
  assert(saved.core.eip == pc &&
         !memcmp(&saved.fp, &raw_before.fp, sizeof(saved.fp)));
  ++event_count;
  return 1;
}
enum {
  NORMAL,
  TRANSFER_LIMIT,
  FETCH_LIMIT,
  SECOND_LIMIT,
  READONLY_CLEAR,
  READONLY_SET,
  NOT_PRESENT,
  READ_HOLE,
  SOURCE_LIMIT,
  DEFAULT16,
  NONZERO_BASE,
  STI_PENDING,
  LEGACY
};
static void cpu_case(const struct Row *r, unsigned mode) {
  row = r;
  raw_reset();
  memset(gdt, 0xa5, sizeof(gdt));
  memset(ldt, 0x5a, sizeof(ldt));
  uint32_t base = mode == NONZERO_BASE ? 0x10000 : 0;
  uint32_t limit = mode == TRANSFER_LIMIT ? r->target - 1
                   : mode == FETCH_LIMIT  ? r->target
                   : mode == SECOND_LIMIT ? r->target + 4
                   : mode == DEFAULT16    ? 65535
                                          : UINT32_MAX;
  unsigned access = mode == NOT_PRESENT    ? 0x1a
                    : mode == READONLY_SET ? 0x9b
                                           : 0x9a;
  descriptor(gdt + 8, base, limit, mode != DEFAULT16, access);
  descriptor(gdt + 16, base, limit, mode != DEFAULT16, access);
  descriptor(ldt + 16, base, limit, mode != DEFAULT16, access);
  descriptor(gdt + 24, 0x4000, 23, 0, 0x82);
  selected = (r->selector & 4) ? ldt + 16 : gdt + (r->selector == 8 ? 8 : 16);
  descriptor_address =
      (r->selector & 4) ? 0x4010 : (r->selector == 8 ? 0x1008 : 0x1010);
  unsigned permission =
      (mode == READONLY_CLEAR || mode == READONLY_SET) ? 1 : 3;
  GamePkgX86Region regions[2] = {{0x1000, sizeof(gdt), permission, gdt},
                                 {0x4000, sizeof(ldt), permission, ldt}};
  unsigned valid = r->selector == 8 || r->selector == 16 || r->selector == 20;
  if (mode == READ_HOLE && valid) {
    unsigned index = (r->selector & 4) ? 1 : 0;
    regions[index].length = descriptor_address - regions[index].base + 7;
  }
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(regions, 2, 1);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  gamepkg_x86_memory_release(memory);
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  const GamePkgX86Tables tables = {{0x1000, 31}, {0x3000, 255}};
  const GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 1};
  assert(!gamepkg_x86_cpu_init_tables(cpu, &tables));
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread));
  GamePkgX86CpuCodeState code = {8,      0,  UINT32_MAX, 0xc9b, 24,
                                 0x4000, 23, 0x82,       1};
  if (mode == SOURCE_LIMIT) {
    code.cs_limit = r->pc + r->bytes - 2;
    code.cs_attributes = 0x49b;
  }
  assert(!gamepkg_x86_cpu_init_code(thread, &code));
  const GamePkgX86CpuEventSource events = {NULL, pending};
  unsigned char old_gdt[sizeof(gdt)], old_ldt[sizeof(ldt)];
  memcpy(old_gdt, gdt, sizeof(gdt));
  memcpy(old_ldt, ldt, sizeof(ldt));
  GamePkgX86DispatchExit out;
  int status = mode == LEGACY
                   ? gamepkg_x86_dispatch_run_cpu(map, map_count, r->pc, &out,
                                                  &source, thread)
                   : gamepkg_x86_dispatch_run_cpu_code_events(
                         map, map_count, r->pc, &out, &source, thread,
                         mode == STI_PENDING ? &events : NULL);
  ++cpu_cases;
  unsigned vector = 0, error = 0, address = 0;
  uint32_t fault_pc = r->pc;
  unsigned committed = 0, changed_eax = 0;
  if (mode == LEGACY) {
    assert(status == 3);
  } else if (mode == SOURCE_LIMIT) {
    vector = 13;
  } else if (!valid) {
    vector = 13;
    error = r->selector & 0xfffcu;
  } else if (mode == READ_HOLE) {
    vector = 14;
    address = descriptor_address + 7;
  } else if (mode == NOT_PRESENT) {
    vector = 11;
    error = r->selector & 0xfffcu;
  } else if (mode == TRANSFER_LIMIT || (mode == DEFAULT16 && !r->small)) {
    vector = 13;
  } else if (mode == READONLY_CLEAR) {
    vector = 14;
    error = 3;
    address = descriptor_address + 5;
  } else {
    committed = 1;
    if (mode == STI_PENDING) {
      assert(status == 4 && out.pc == r->target && event_count == 1);
    } else if (mode == DEFAULT16 || mode == NONZERO_BASE) {
      assert(status == 3 && out.pc == base + r->target && !guarded_execution);
    } else if (mode == FETCH_LIMIT) {
      vector = 13;
      fault_pc = r->target;
    } else if (mode == SECOND_LIMIT) {
      vector = 13;
      fault_pc = r->target + 5;
      changed_eax = 1;
    } else {
      assert(status == 0 && out.pc == 0xf0000000);
      changed_eax = 1;
    }
  }
  if (vector) {
    assert(status == 1 && out.pc == fault_pc &&
           out.fault.core.vector == vector && out.fault.core.eip == fault_pc &&
           out.fault.core.error_code == error &&
           out.fault.core.fault_address == address &&
           !memcmp(&out.fault.fp, &raw_before.fp, sizeof(out.fault.fp)));
  } else
    assert(!out.fault.core.vector);
  assert(g_eax == (changed_eax ? 0x11223344u : 0xaabbccddu));
  GamePkgX86CpuExecution after;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &after));
  assert(after.interrupt.flags == 0x202 &&
         after.interrupt.sti_shadow == (mode == LEGACY ? 1u : 0u));
  if (committed) {
    assert(after.code.cs_selector == r->selector &&
           after.code.cs_base == base && after.code.cs_limit == limit &&
           (selected[5] & 1));
    assert(gamepkg_x86_x87_current_code_segment() == r->selector);
    GamePkgX87State record = {0};
    gamepkg_x86_x87_record(&record, 0x1234, 0x100);
    gamepkg_x86_x87_record_data(&record, 0x1234);
    assert(record.instruction_selector == r->selector &&
           record.instruction_pointer == 0x1234 &&
           record.data_selector == 0x20 && record.data_pointer == 0x234);
  } else {
    assert(!memcmp(&after.code, &code, sizeof(code)));
    assert(selected[5] == access);
  }
  if (committed) {
    if (r->selector & 4)
      old_ldt[21] |= 1;
    else
      old_gdt[(r->selector == 8 ? 8 : 16) + 5] |= 1;
    GamePkgX86CpuCodeState expected = code;
    expected.cs_selector = r->selector;
    expected.cs_base = base;
    expected.cs_limit = limit;
    expected.cs_attributes = (access | 1u) | (mode == DEFAULT16 ? 0u : 0x400u) |
                             (limit > 0xfffff ? 0x800u : 0u);
    assert(!memcmp(&after.code, &expected, sizeof(expected)));
  }
  assert(!memcmp(gdt, old_gdt, sizeof(gdt)) &&
         !memcmp(ldt, old_ldt, sizeof(ldt)));
  raw_preserved();

  assert(!gamepkg_x86_cpu_retire_other(thread, g_eflags));
  assert(!gamepkg_x86_cpu_block(thread));
  assert(!gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
}
enum {
  F_NORMAL,
  F_READ_ERROR,
  F_READ_FAULT,
  F_READ_REENTER,
  F_BAD_TABLE,
  F_BAD_CODE,
  F_BAD_CPL,
  F_COMMIT_ERROR,
  F_COMMIT_FAULT,
  F_COMMIT_REENTER,
  F_RETIRE_ERROR,
  F_FREEZE,
  F_NEST,
  F_BOUNDARY_STOP
};
typedef struct Probe {
  GamePkgX86DispatchCodeImage image;
  unsigned mode, reads, commits, boundaries, retired, faults;
} Probe;
static _Alignas(GamePkgX86FpFault) GamePkgX86DispatchCode original_code;
static unsigned nested_seen;
static int probe_boundary(void *, uint32_t, uint32_t);
static int probe_retire(void *, uint32_t, uint32_t, uint32_t);
static int probe_read(void *, GamePkgX86DispatchCodeImage *);
static int probe_commit(void *, const GamePkgX86CodeTarget *);
static int probe_table(void *raw, uint32_t op, uint32_t *base,
                       uint32_t *limit) {
  Probe *p = raw;
  assert(op < 2);
  *base = op ? 0x3000 : p->image.tables.gdt_base;
  *limit = op ? 255 : p->image.tables.gdt_limit;
  return 0;
}
static void nested_far(uint32_t pc) {
  (void)pc;
  gamepkg_x86_dispatch_far_jump(16, 0x20000, 0, g_ebp);
}
static void nested_nop(uint32_t pc) {
  if (gamepkg_x86_dispatch_instruction(pc, 1, g_ebp))
    return;
  nested_seen = gamepkg_x86_x87_current_code_segment();
  if (gamepkg_x86_dispatch_retire(g_ebp))
    return;
  gamepkg_x86_dispatch_stop(g_ebp);
}
static int probe_boundary(void *raw, uint32_t pc, uint32_t ebp) {
  Probe *p = raw;
  (void)pc;
  (void)ebp;
  ++p->boundaries;
  if (p->mode == F_BOUNDARY_STOP)
    return 1;
  if (p->mode == F_FREEZE) {
    original_code.read = NULL;
    original_code.commit = NULL;
  }
  if (p->mode == F_NEST && p->boundaries == 1) {
    assert(gamepkg_x86_dispatch_capture_checkpoint((void *)&original_code) ==
           -1);
    const GamePkgX86DispatchEntry plain = {0xabc00, nested_far};
    GamePkgX86DispatchExit out;
    assert(gamepkg_x86_dispatch_run(&plain, 1, plain.address, &out, &source) ==
           3);
    Probe inner = *p;
    inner.mode = F_NORMAL;
    inner.image.code.selector = 32;
    inner.boundaries = inner.retired = 0;
    const GamePkgX86DispatchControl control = {&inner, probe_boundary,
                                               probe_retire, NULL};
    const GamePkgX86DispatchTables tables = {&inner, probe_table};
    const GamePkgX86DispatchCode code = {&inner, probe_read, probe_commit};
    const GamePkgX86DispatchEntry entry = {0xabc00, nested_nop};
    assert(gamepkg_x86_dispatch_run_controlled_code(&entry, 1, entry.address,
                                                    &out, &source, &control,
                                                    &tables, &code) == 0);
    assert(nested_seen == 32 && inner.retired == 1 &&
           gamepkg_x86_x87_current_code_segment() == 8);
  }
  return 0;
}
static int probe_retire(void *raw, uint32_t pc, uint32_t ebp, uint32_t event) {
  Probe *p = raw;
  (void)ebp;
  if (event == GAMEPKG_X86_RETIRE_FAULT) {
    ++p->faults;
    return 0;
  }
  assert(event == GAMEPKG_X86_RETIRE_OTHER);
  ++p->retired;
  if (pc == row->pc) {
    assert(p->image.code.selector == row->selector &&
           gamepkg_x86_dispatch_pc == row->target &&
           gamepkg_x86_x87_current_code_segment() == row->selector);
    if (p->mode == F_RETIRE_ERROR)
      return -1;
  }
  return 0;
}
static int probe_read(void *raw, GamePkgX86DispatchCodeImage *out) {
  Probe *p = raw;
  ++p->reads;
  if (p->mode == F_READ_ERROR)
    return -1;
  if (p->mode == F_READ_FAULT)
    gamepkg_x86_general_protection(0xdead, 0x8000);
  if (p->mode == F_READ_REENTER)
    gamepkg_x86_dispatch_far_jump(16, 0x20000, 0, 0x8000);
  *out = p->image;
  if (p->mode == F_BAD_TABLE)
    out->tables.gdt_limit = 65536;
  if (p->mode == F_BAD_CODE)
    out->code.attributes &= ~1u;
  if (p->mode == F_BAD_CPL)
    out->cpl = 3;
  return 0;
}
static int probe_commit(void *raw, const GamePkgX86CodeTarget *next) {
  Probe *p = raw;
  ++p->commits;
  assert(!next->vector && !next->error && !next->mark_accessed &&
         (selected[5] & 1));
  if (p->mode == F_COMMIT_ERROR)
    return -1;
  if (p->mode == F_COMMIT_FAULT)
    gamepkg_x86_general_protection(0xbad, 0x8000);
  if (p->mode == F_COMMIT_REENTER) {
    assert(gamepkg_x86_dispatch_instruction(row->pc, 1, 0x8000));
    return -1;
  }
  p->image.code = *next;
  p->image.code.offset = 0;
  return 0;
}
static Probe probe_reset(unsigned mode) {
  row = &rows[4];
  assert(row->selector == 16 && !row->small);
  raw_reset();
  memset(gdt, 0xa5, sizeof(gdt));
  memset(ldt, 0x5a, sizeof(ldt));
  descriptor(gdt + 16, 0, UINT32_MAX, 1, 0x9a);
  selected = gdt + 16;
  const GamePkgX86Region region = {0x1000, 32, 3, gdt};
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(&region, 1, 1);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  gamepkg_x86_memory_release(memory);
  Probe p = {0};
  p.mode = mode;
  p.image = (GamePkgX86DispatchCodeImage){
      .tables = {0x1000, 31, 0x4000, 23, 1},
      .code = {.selector = 8, .limit = UINT32_MAX, .attributes = 0xc9b},
      .cpl = 0};
  return p;
}
static void provider_cases(void) {
  for (unsigned mode = F_NORMAL; mode <= F_BOUNDARY_STOP; ++mode) {
    Probe p = probe_reset(mode);
    GamePkgX86DispatchControl control = {&p, probe_boundary, probe_retire,
                                         NULL};
    GamePkgX86DispatchTables tables = {&p, probe_table};
    original_code = (GamePkgX86DispatchCode){&p, probe_read, probe_commit};
    GamePkgX86DispatchExit out;
    int status = gamepkg_x86_dispatch_run_controlled_code(
        map, map_count, row->pc, &out, &source, &control, &tables,
        &original_code);
    if (mode == F_NORMAL || mode == F_FREEZE || mode == F_NEST) {
      assert(status == 0 && out.pc == 0xf0000000 && p.commits == 1 &&
             p.retired == 3 && !p.faults && p.image.code.selector == 16 &&
             g_eax == 0x11223344 && (selected[5] & 1));
    } else if (mode == F_BOUNDARY_STOP) {
      assert(status == 4 && !p.reads && !p.commits && !p.retired &&
             selected[5] == 0x9a);
    } else {
      assert(status == 3 && !p.faults && g_eax == 0xaabbccdd);
      if (mode == F_RETIRE_ERROR) {
        assert(out.pc == row->target && p.image.code.selector == 16 &&
               p.retired == 1);
      } else
        assert(p.image.code.selector == 8 && !p.retired);
      if (mode >= F_COMMIT_ERROR && mode <= F_COMMIT_REENTER) {
        assert(p.commits == 1 && (selected[5] & 1));
      }
      if (mode < F_COMMIT_ERROR) {
        assert(!p.commits && selected[5] == 0x9a);
      }
      assert(out.fault.core.vector ==
             ((mode == F_READ_FAULT || mode == F_COMMIT_FAULT) ? 13u : 0u));
    }
    raw_preserved();
  }
  Probe p = probe_reset(F_NORMAL);
  const GamePkgX86DispatchControl control = {&p, probe_boundary, probe_retire,
                                             NULL};
  const GamePkgX86DispatchTables tables = {&p, probe_table};
  original_code = (GamePkgX86DispatchCode){&p, probe_read, probe_commit};
  GamePkgX86DispatchExit out, saved;
  memset(&out, 0xa5, sizeof(out));
  saved = out;
  assert(gamepkg_x86_dispatch_run_controlled_code(map, map_count, row->pc, &out,
                                                  &source, &control, &tables,
                                                  NULL) == -1);
  assert(!memcmp(&out, &saved, sizeof(out)));
  const GamePkgX86DispatchCode *aliases[] = {
      NULL,
      (void *)map,
      (void *)&control,
      (void *)&tables,
      (void *)&out,
      (void *)source.x87,
      (void *)((unsigned char *)&original_code + 1)};
  for (unsigned i = 0; i < sizeof(aliases) / sizeof(*aliases); ++i) {
    assert(gamepkg_x86_dispatch_run_controlled_code(map, map_count, row->pc,
                                                    &out, &source, &control,
                                                    &tables, aliases[i]) == -1);
    assert(!memcmp(&out, &saved, sizeof(out)));
  }
  original_code.read = NULL;
  assert(gamepkg_x86_dispatch_run_controlled_code(map, map_count, row->pc, &out,
                                                  &source, &control, &tables,
                                                  &original_code) == -1);
  original_code.read = probe_read;
  original_code.commit = NULL;
  assert(gamepkg_x86_dispatch_run_controlled_code(map, map_count, row->pc, &out,
                                                  &source, &control, &tables,
                                                  &original_code) == -1);
  assert(!memcmp(&out, &saved, sizeof(out)) && !p.reads && selected[5] == 0x9a);
  gamepkg_x86_memory_bind(NULL);
}
static void missing_cpu_configuration(void) {
  Probe p = probe_reset(F_NORMAL);
  (void)p;
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  const GamePkgX86Tables tables = {{0x1000, 31}, {0x3000, 255}};
  const GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 0};
  assert(!gamepkg_x86_cpu_init_tables(cpu, &tables));
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread));
  GamePkgX86DispatchExit out, saved;
  memset(&out, 0xa5, sizeof(out));
  saved = out;
  assert(gamepkg_x86_dispatch_run_cpu_code_events(map, map_count, row->pc, &out,
                                                  &source, thread, NULL) == -1);
  assert(!memcmp(&out, &saved, sizeof(out)) && selected[5] == 0x9a);
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
}
int main(void) {
  maps();
  for (unsigned i = 0; i < sizeof(rows) / sizeof(*rows); ++i)
    for (unsigned mode = NORMAL; mode <= LEGACY; ++mode)
      cpu_case(rows + i, mode);
  provider_cases();
  missing_cpu_configuration();
  printf(
      "Far dispatch: %u generated/CPU cases, scoped/frozen/nested/error/alias "
      "providers, CS-PC-retirement/fetch/STI ordering and raw x87/MMX/XMM "
      "binding preservation passed; pointer_bits=%u\n",
      cpu_cases, (unsigned)(8 * sizeof(void *)));
  return 0;
}
