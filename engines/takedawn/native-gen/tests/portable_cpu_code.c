/* Synthetic owner/cache/fault/restore checks, without game media. */
#include "gamepkg_x86_cpu_code.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big-endian compiler required
#endif
_Static_assert(sizeof(void *) == 4, "actual big-endian32 required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static const GamePkgX86Tables tables = {{0x1000, 31}, {0x3000, 255}};
static const GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 1};
static const GamePkgX86CpuCodeState flat = {8, 0, UINT32_MAX, 0xc9b, 0,
                                            0, 0, 0,          0};
static GamePkgX86CpuThread *current;
static uint32_t selector, target, operand16, bytes, action;
static int returned;
static unsigned cases;
static GamePkgX87State raw_x87;
static uint32_t raw_xmm[8][4];
static const GamePkgX86FpSource fp_source = {
    &raw_x87,
    {raw_xmm[0], raw_xmm[1], raw_xmm[2], raw_xmm[3], raw_xmm[4], raw_xmm[5],
     raw_xmm[6], raw_xmm[7]}};
static void entry(void) {
  returned =
      action ? gamepkg_x86_cpu_check_code_span(current, target, bytes, 0x789abc)
             : gamepkg_x86_cpu_far_transfer(current, selector, target,
                                            operand16, 0x123456, 0x789abc);
}
static void invoke(unsigned vector, unsigned error, unsigned fault_address) {
  GamePkgX86FpFault image, before_raw, after_raw;
  assert(!gamepkg_x86_capture_registers(0, 0, &before_raw, &fp_source));
  returned = 123;
  assert(gamepkg_x86_run_with_fp(entry, &image, &fp_source) ==
         (vector ? 1 : 0));
  const GamePkgX86Fault fault = image.core;
  ++cases;
  if (vector) {
    assert(!memcmp(&image.fp, &before_raw.fp, sizeof(image.fp)));
    assert(returned == 123 && fault.vector == vector &&
           fault.error_code == error && fault.fault_address == fault_address &&
           fault.eip == (action ? target : 0x123456) && fault.ebp == 0x789abc &&
           fault.eax == 0xaabbccdd && fault.eflags == g_eflags);
  }
  assert(!gamepkg_x86_capture_registers(0, 0, &after_raw, &fp_source));
  assert(!memcmp(&before_raw, &after_raw, sizeof(before_raw)));
  assert(g_eax == 0xaabbccdd);
}
static void put(unsigned char *p, uint32_t lo, uint32_t hi) {
  for (unsigned i = 0; i < 4; ++i) {
    p[i] = (unsigned char)(lo >> (8 * i));
    p[i + 4] = (unsigned char)(hi >> (8 * i));
  }
}
static void bind_regions(const GamePkgX86Region *regions, size_t count) {
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(regions, count, 1);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  gamepkg_x86_memory_release(memory);
}
static void same_execution(const GamePkgX86CpuExecution *expected) {
  GamePkgX86CpuExecution got;
  assert(!gamepkg_x86_cpu_snapshot_execution(current, &got));
  assert(!memcmp(&got, expected, sizeof(got)));
}
static GamePkgX86Cpu *make_cpu(unsigned configured) {
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  if (configured)
    assert(!gamepkg_x86_cpu_init_tables(cpu, &tables));
  current = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(current);
  return cpu;
}
static void finish_cpu(GamePkgX86Cpu *cpu) {
  assert(!gamepkg_x86_cpu_retire_other(current, g_eflags));
  assert(!gamepkg_x86_cpu_block(current));
  assert(!gamepkg_x86_cpu_thread_destroy(current));
  gamepkg_x86_cpu_release(cpu);
}
typedef struct ForeignJob {
  GamePkgX86CpuThread *thread;
  GamePkgX86CpuExecution expected;
} ForeignJob;
#ifdef _WIN32
static DWORD WINAPI foreign_worker(void *raw) {
#else
static void *foreign_worker(void *raw) {
#endif
  const ForeignJob *job = raw;
  GamePkgX86CpuExecution got, untouched;
  memset(&got, 0x5a, sizeof(got));
  untouched = got;
  const GamePkgX86CodeTarget proposal = {
      .selector = 16, .limit = UINT32_MAX, .attributes = 0xc9b};
  assert(gamepkg_x86_cpu_init_code(job->thread, &flat) == -1);
  assert(gamepkg_x86_cpu_snapshot_execution(job->thread, &got) == -1 &&
         !memcmp(&got, &untouched, sizeof(got)));
  assert(gamepkg_x86_cpu_restore_execution(job->thread, &job->expected) == -1);
  assert(gamepkg_x86_cpu_commit_code(job->thread, &proposal) == -1);
  assert(gamepkg_x86_cpu_far_transfer(job->thread, 0, 0, 0, 0, 0) == -1);
  assert(gamepkg_x86_cpu_check_code_span(job->thread, 0, 1, 0) == -1);
  return 0;
}
static void foreign_owner(const GamePkgX86CpuExecution *saved) {
  ForeignJob job = {current, *saved};
#ifdef _WIN32
  HANDLE thread = CreateThread(NULL, 0, foreign_worker, &job, 0, NULL);
  assert(thread && WaitForSingleObject(thread, INFINITE) == WAIT_OBJECT_0);
  assert(CloseHandle(thread));
#else
  pthread_t thread;
  assert(!pthread_create(&thread, NULL, foreign_worker, &job));
  assert(!pthread_join(thread, NULL));
#endif
  same_execution(saved);
}
static void setup_and_invalid(void) {
  GamePkgX86CpuExecution out, saved;
  memset(&out, 0xa5, sizeof(out));
  saved = out;
  GamePkgX86Cpu *cpu = make_cpu(0);
  assert(!gamepkg_x86_cpu_enter(current));
  assert(gamepkg_x86_cpu_init_code(current, &flat) == -1);
  assert(gamepkg_x86_cpu_snapshot_execution(current, &out) == -1 &&
         !memcmp(&out, &saved, sizeof(out)));
  selector = 0;
  action = 0;
  invoke(0, 0, 0); /* Missing configuration wins before even selector GP. */
  assert(returned == -1);
  finish_cpu(cpu);
  cpu = make_cpu(1);
  assert(gamepkg_x86_cpu_init_code(current, &flat) == -1);
  assert(!gamepkg_x86_cpu_enter(current));
  assert(gamepkg_x86_cpu_snapshot_execution(current, &out) == -1 &&
         !memcmp(&out, &saved, sizeof(out)));
  for (unsigned invalid = 0; invalid < 18; ++invalid) {
    GamePkgX86CpuCodeState bad = flat;
    if (invalid == 0)
      bad.cs_selector = 0;
    if (invalid == 1)
      bad.cs_selector = 65536;
    if (invalid == 2)
      bad.cs_selector = 11;
    if (invalid == 3)
      bad.cs_attributes &= ~1u;
    if (invalid == 4)
      bad.cs_attributes &= ~0x80u;
    if (invalid == 5)
      bad.cs_attributes &= ~0x10u;
    if (invalid == 6)
      bad.cs_attributes &= ~8u;
    if (invalid == 7)
      bad.cs_attributes |= 0x1000;
    if (invalid == 8)
      bad.cs_limit--;
    if (invalid == 9)
      bad.cs_attributes &= ~0x800u;
    if (invalid == 10)
      bad.ldtr_usable = 2;
    if (invalid == 11)
      bad.ldtr_selector = 8;
    if (invalid == 12)
      bad.ldtr_attributes = 0x1000;
    if (invalid == 13) {
      bad.ldtr_usable = 1;
      bad.ldtr_attributes = 0x82;
    }
    if (invalid == 14) {
      bad.ldtr_usable = 1;
      bad.ldtr_selector = 12;
      bad.ldtr_attributes = 0x82;
    }
    if (invalid == 15) {
      bad.ldtr_usable = 1;
      bad.ldtr_selector = 24;
      bad.ldtr_attributes = 0x92;
    }
    if (invalid == 16) {
      bad.ldtr_usable = 1;
      bad.ldtr_selector = 24;
      bad.ldtr_attributes = 0x82;
      bad.ldtr_limit = 0x100000;
    }
    if (invalid == 17) {
      bad.ldtr_usable = 1;
      bad.ldtr_selector = 24;
      bad.ldtr_attributes = 0x882;
      bad.ldtr_limit = 0;
    }
    assert(gamepkg_x86_cpu_init_code(current, &bad) == -1);
  }
  for (unsigned vm = 0; vm < 2; ++vm) {
    GamePkgX86InterruptState bad_mode = initial;
    if (vm) {
      bad_mode.cpl = 3;
      bad_mode.flags |= 0x20000;
    } else {
      bad_mode.protected_mode = 0;
    }
    assert(!gamepkg_x86_cpu_restore(current, &bad_mode));
    assert(gamepkg_x86_cpu_init_code(current, &flat) == -1);
  }
  assert(!gamepkg_x86_cpu_restore(current, &initial));
  assert(!gamepkg_x86_cpu_init_code(current, &flat));
  assert(gamepkg_x86_cpu_init_code(current, &flat) == -1);
  assert(!gamepkg_x86_cpu_snapshot_execution(current, &saved));
  for (unsigned invalid = 0; invalid < 7; ++invalid) {
    GamePkgX86CpuExecution bad = saved;
    if (invalid == 0)
      bad.tables.gdtr.base++;
    if (invalid == 1)
      bad.tables.gdtr.limit++;
    if (invalid == 2)
      bad.tables.idtr.base++;
    if (invalid == 3)
      bad.tables.idtr.limit++;
    if (invalid == 4)
      bad.interrupt.cpl = 3;
    if (invalid == 5)
      bad.interrupt.protected_mode = 0;
    if (invalid == 6) {
      bad.interrupt.cpl = 3;
      bad.interrupt.flags |= 0x20000;
    }
    assert(gamepkg_x86_cpu_restore_execution(current, &bad) == -1);
    assert(gamepkg_x86_cpu_restore(current, &bad.interrupt) ==
           (invalid < 4 ? 0 : -1));
    same_execution(&saved);
  }
  assert(gamepkg_x86_cpu_checkpoint(current, 0x20202) == -1);
  same_execution(&saved);
  /* Misalignment/overflow/alias rejection precedes dereference or mutation. */
  const void *bad_spans[] = {NULL,
                             (unsigned char *)&out + 1,
                             (void *)(UINTPTR_MAX & ~(uintptr_t)3),
                             current,
                             cpu,
                             (unsigned char *)current + 4};
  for (unsigned i = 0; i < sizeof(bad_spans) / sizeof(*bad_spans); ++i) {
    assert(gamepkg_x86_cpu_init_code(current, bad_spans[i]) == -1);
    assert(gamepkg_x86_cpu_snapshot_execution(current, (void *)bad_spans[i]) ==
           -1);
    assert(gamepkg_x86_cpu_restore_execution(current, bad_spans[i]) == -1);
    assert(gamepkg_x86_cpu_commit_code(current, bad_spans[i]) == -1);
    same_execution(&saved);
  }
  GamePkgX86CodeTarget proposal = {.selector = 16,
                                   .base = 0x89abcdef,
                                   .limit = 0x12345fff,
                                   .offset = 0x1234,
                                   .attributes = 0xc9b};
  for (unsigned invalid = 0; invalid < 8; ++invalid) {
    GamePkgX86CodeTarget bad = proposal;
    if (invalid == 0)
      bad.vector = 13;
    if (invalid == 1)
      bad.error = 8;
    if (invalid == 2)
      bad.mark_accessed = 1;
    if (invalid == 3)
      bad.offset = bad.limit + 1;
    if (invalid == 4)
      bad.selector = 19;
    if (invalid == 5)
      bad.attributes &= ~1u;
    if (invalid == 6)
      bad.attributes &= ~0x80u;
    if (invalid == 7)
      bad.limit--;
    assert(gamepkg_x86_cpu_commit_code(current, &bad) == -1);
    same_execution(&saved);
  }
  foreign_owner(&saved);
  assert(!gamepkg_x86_cpu_suspend(current));
  assert(gamepkg_x86_cpu_snapshot_execution(current, &out) == -1);
  assert(gamepkg_x86_cpu_commit_code(current, &proposal) == -1);
  assert(gamepkg_x86_cpu_restore_execution(current, &saved) == -1);
  assert(!gamepkg_x86_cpu_enter(current));
  same_execution(&saved);
  finish_cpu(cpu);
}
static void transfers_and_fetch(void) {
  GamePkgX86Cpu *cpu = make_cpu(1);
  assert(!gamepkg_x86_cpu_enter(current));
  assert(!gamepkg_x86_cpu_init_code(current, &flat));
  GamePkgX86CpuExecution original, before, after;
  assert(!gamepkg_x86_cpu_snapshot_execution(current, &original));
  unsigned char gdt[32], ldt[16];
  memset(gdt, 0xa5, sizeof(gdt));
  memset(ldt, 0x5a, sizeof(ldt));
  GamePkgX86Region regions[2] = {{0x1000, 32, 3, gdt}, {0x2000, 16, 3, ldt}};
  for (unsigned cpl = 0; cpl < 4; ++cpl)
    for (unsigned rpl = 0; rpl < 4; ++rpl)
      for (unsigned local = 0; local < 2; ++local)
        for (unsigned small = 0; small < 2; ++small)
          for (unsigned accessed = 0; accessed < 2; ++accessed) {
            before = original;
            before.interrupt.cpl = cpl;
            before.code.cs_selector |= cpl;
            before.code.cs_attributes |= cpl << 5;
            before.code.ldtr_selector = 24;
            before.code.ldtr_base = 0x2000;
            before.code.ldtr_limit = 15;
            before.code.ldtr_attributes = 0x82;
            before.code.ldtr_usable = 1;
            assert(!gamepkg_x86_cpu_restore_execution(current, &before));
            unsigned char *descriptor = local ? ldt + 8 : gdt + 16;
            put(descriptor, 0xcdef2345,
                0x89c19aab | (cpl << 13) | (accessed << 8));
            bind_regions(regions, 2);
            selector = (local ? 12 : 16) | rpl;
            target = small ? 0xa5a51234 : 0x1234;
            operand16 = small;
            action = 0;
            if (rpl > cpl) {
              invoke(13, selector & 0xfffcu, 0);
              same_execution(&before);
              assert((descriptor[5] & 1) == accessed);
            } else {
              invoke(0, 0, 0);
              assert(returned == 0 && (descriptor[5] & 1));
              after = before;
              after.code.cs_selector = (selector & 0xfffcu) | cpl;
              after.code.cs_base = 0x89abcdef;
              after.code.cs_limit = 0x12345fff;
              after.code.cs_attributes = 0xc9b | (cpl << 5);
              same_execution(&after); /* Includes unchanged STI shadow1. */
            }
          }
  assert(!gamepkg_x86_cpu_restore_execution(current, &original));
  before = original;
  /* Conforming DPL0 code remains valid at CPL3 even with selector RPL3. */
  before = original;
  before.interrupt.cpl = 3;
  before.code.cs_selector = 11;
  before.code.cs_attributes = 0xcfb;
  assert(!gamepkg_x86_cpu_restore_execution(current, &before));
  put(gdt + 16, 0xcdef2345, 0x89c19eab);
  bind_regions(regions, 2);
  selector = 19;
  target = 0x1234;
  operand16 = action = 0;
  invoke(0, 0, 0);
  assert(returned == 0);
  after = before;
  after.code.cs_selector = 19;
  after.code.cs_base = 0x89abcdef;
  after.code.cs_limit = 0x12345fff;
  after.code.cs_attributes = 0xc9f;
  same_execution(&after);
  /* Cache the descriptor's actual16-bit default size, independently of the
   * far pointer operand size. An untruncated offset fails before an A write. */
  assert(!gamepkg_x86_cpu_restore_execution(current, &original));
  before = original;
  put(gdt + 16, 0xffff, 0x9a00);
  selector = 16;
  target = 0xa5a51234;
  invoke(13, 0, 0);
  same_execution(&before);
  assert(gdt[21] == 0x9a);
  operand16 = 1;
  invoke(0, 0, 0);
  assert(returned == 0 && gdt[21] == 0x9b);
  after = before;
  after.code.cs_selector = 16;
  after.code.cs_limit = 65535;
  after.code.cs_attributes = 0x9b;
  same_execution(&after);
  assert(!gamepkg_x86_cpu_restore_execution(current, &original));
  before = original;
  selector = 16;
  target = 0;
  operand16 = action = 0;
  put(gdt + 16, 0, 0x409a00); /* Valid entry offset0, only one code byte. */
  regions[0].permissions = 1;
  bind_regions(regions, 2);
  invoke(14, 3, 0x1015);
  same_execution(&before);
  regions[0].permissions = 3;
  regions[0].length = 23;
  bind_regions(regions, 2);
  invoke(14, 0, 0x1017);
  same_execution(&before);
  regions[0].length = 32;
  bind_regions(regions, 2);
  target = 1;
  invoke(13, 0, 0); /* Transfer-limit fault leaves CS8 and A0. */
  same_execution(&before);
  assert(gdt[21] == 0x9a);
  target = 0;
  invoke(0, 0, 0);
  assert(returned == 0 && gdt[21] == 0x9b);
  after = before;
  after.code.cs_selector = 16;
  after.code.cs_base = 0;
  after.code.cs_limit = 0;
  after.code.cs_attributes = 0x49b;
  same_execution(&after);
  action = 1;
  bytes = 5;
  invoke(13, 0, 0); /* Later crossing fetch retains the new CS16. */
  same_execution(&after);
  bytes = 1;
  invoke(0, 0, 0);
  assert(returned == 0);
  target = 1;
  invoke(13, 0, 0);
  same_execution(&after);
  for (bytes = 0; bytes <= 16; bytes += 16) {
    invoke(0, 0, 0);
    assert(returned == -1);
    same_execution(&after);
  }
  assert(!gamepkg_x86_cpu_retire_other(current, g_eflags));
  after.interrupt.sti_shadow = 0;
  same_execution(&after);
  assert(!gamepkg_x86_cpu_restore_execution(current, &original));
  same_execution(&original); /* Includes restoring the earlier STI shadow. */
  finish_cpu(cpu);
  gamepkg_x86_memory_bind(NULL);
}
typedef struct Job {
  GamePkgX86CpuThread *thread;
  unsigned id;
} Job;
#ifdef _WIN32
static DWORD WINAPI worker(void *raw) {
#else
static void *worker(void *raw) {
#endif
  Job *job = raw;
  GamePkgX86CpuExecution expected = {{0x202, 1, 0, 0, 0, 0}, tables, flat};
  assert(!gamepkg_x86_cpu_enter(job->thread));
  assert(!gamepkg_x86_cpu_init_code(job->thread, &flat));
  assert(!gamepkg_x86_cpu_block(job->thread));
  for (unsigned round = 0; round < 128; ++round) {
    GamePkgX86CpuExecution got;
    memset(&got, 0xa5, sizeof(got));
    assert(gamepkg_x86_cpu_snapshot_execution(job->thread, &got) == -1);
    assert(!gamepkg_x86_cpu_enter(job->thread));
    assert(!gamepkg_x86_cpu_snapshot_execution(job->thread, &got));
    assert(!memcmp(&got, &expected, sizeof(got)));
    expected.code.cs_base = (job->id << 24) | round;
    expected.code.ldtr_base = ~expected.code.cs_base;
    assert(!gamepkg_x86_cpu_restore_execution(job->thread, &expected));
    assert(!gamepkg_x86_cpu_check_code_span(job->thread, UINT32_MAX, 1, 0));
    assert(!gamepkg_x86_cpu_suspend(job->thread));
    assert(!gamepkg_x86_cpu_enter(job->thread));
    assert(!gamepkg_x86_cpu_snapshot_execution(job->thread, &got));
    assert(!memcmp(&got, &expected, sizeof(got)));
    assert(!gamepkg_x86_cpu_block(job->thread));
  }
  return 0;
}
static void workers(void) {
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu && !gamepkg_x86_cpu_init_tables(cpu, &tables));
  GamePkgX86InterruptState state = initial;
  state.sti_shadow = 0;
  Job jobs[8];
#ifdef _WIN32
  HANDLE threads[8];
#else
  pthread_t threads[8];
#endif
  for (unsigned i = 0; i < 8; ++i) {
    jobs[i] = (Job){gamepkg_x86_cpu_thread_create(cpu, &state), i + 1};
    assert(jobs[i].thread);
#ifdef _WIN32
    threads[i] = CreateThread(NULL, 0, worker, jobs + i, 0, NULL);
    assert(threads[i]);
#else
    assert(!pthread_create(threads + i, NULL, worker, jobs + i));
#endif
  }
  for (unsigned i = 0; i < 8; ++i) {
#ifdef _WIN32
    assert(WaitForSingleObject(threads[i], INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(threads[i]));
#else
    assert(!pthread_join(threads[i], NULL));
#endif
    assert(!gamepkg_x86_cpu_thread_destroy(jobs[i].thread));
  }
  gamepkg_x86_cpu_release(cpu);
}
int main(void) {
  memset(&raw_x87, 0xa5, sizeof(raw_x87));
  raw_x87.control = 0x37e;
  raw_x87.status = 0x8081;
  memset(raw_xmm, 0x5a, sizeof(raw_xmm));
  g_eax = 0xaabbccdd;
  g_eflags = initial.flags;
  setup_and_invalid();
  transfers_and_fetch();
  workers();
  printf(
      "CPU code caches: %u transfer/fetch fault cases, explicit init/context/"
      "alias/mode/commit rejection and1024 eight-worker restore/resume leases "
      "passed; pointer_bits=%u\n",
      cases, (unsigned)(8 * sizeof(void *)));
  return 0;
}
