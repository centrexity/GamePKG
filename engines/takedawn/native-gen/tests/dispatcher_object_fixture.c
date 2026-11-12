#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_dispatcher_object.h"
#include "gamepkg_x86_irql.h"
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
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
static GamePkgX86Cpu *cpu;
static GamePkgX86CpuThread *thread;
static GamePkgX86Memory *memory, *argument_memory;
static unsigned char arena[40], mirror[1];
static uint32_t base, address, kind, type, signal_state, priority;
static int status;
static unsigned checks, executed;
static const GamePkgX86CpuCodeState code = {8, 0, UINT32_MAX, 0xc9b, 0,
                                            0, 0, 0,          0};
static const GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 0};
static void put(unsigned char *p, uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(v >> (8 * i));
}
static uint32_t get(const unsigned char *p) {
  return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) |
         ((uint32_t)p[3] << 24);
}
uint32_t xbox_MmioLoad(uint32_t a, int bytes) {
  uint32_t pc, bp, w[4];
  assert(!gamepkg_x86_dispatch_site(&pc, &bp));
  gamepkg_x86_memory_read(a, w, (uint32_t)bytes, pc, bp);
  return w[0];
}
void xbox_MmioStore(uint32_t a, uint32_t v, int bytes) {
  uint32_t pc, bp, w[4] = {v, 0, 0, 0};
  assert(!gamepkg_x86_dispatch_site(&pc, &bp));
  gamepkg_x86_memory_write(a, w, (uint32_t)bytes, (1u << bytes) - 1, 0, pc, bp);
}
/* PROGRAM */
static const GamePkgX86DispatchEntry entries[] = {
    {0x400000, gamepkg_cpu_block_00400000}};
static void initialize(void) {
  status =
      kind ? gamepkg_x86_timer_initialize(thread, argument_memory, address,
                                          type, 0x1234, g_ebp)
           : gamepkg_x86_event_initialize(thread, argument_memory, address,
                                          type, signal_state, 0x1234, g_ebp);
}
static void initialize_priority(void) {
  assert(!gamepkg_x86_irql_initialize(thread, memory, 0x4000, 0x1234, g_ebp));
}
static void change_priority(void) {
  GamePkgX86IrqlResult r =
      gamepkg_x86_irql(thread, priority ? 1 : 2, priority, 0x1234, g_ebp);
  assert(!r.status);
}
static void run(unsigned vector, unsigned error, uint32_t bad, int expected) {
  unsigned char before_ram[40];
  memcpy(before_ram, arena, 40);
  GamePkgX86FpFault before, after, fault;
  GamePkgX86CpuExecution old, now;
  GamePkgX86CpuIrql a, b;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source) &&
         !gamepkg_x86_cpu_snapshot_execution(thread, &old) &&
         !gamepkg_x86_cpu_snapshot_irql(thread, &a));
  int rc = gamepkg_x86_run_with_fp(initialize, &fault, &source);
  assert(rc == (vector ? 1 : 0));
  if (vector) {
    assert(fault.core.vector == vector && fault.core.error_code == error &&
           fault.core.fault_address == bad && fault.core.eip == 0x1234 &&
           !memcmp(&before.fp, &fault.fp, sizeof(before.fp)));
  } else
    assert(status == expected);
  if (vector || expected)
    assert(!memcmp(before_ram, arena, 40));
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before, &after, sizeof(before)) &&
         !gamepkg_x86_cpu_snapshot_execution(thread, &now) &&
         !memcmp(&old, &now, sizeof(old)) &&
         !gamepkg_x86_cpu_snapshot_irql(thread, &b) && a.level == b.level &&
         a.address == b.address && a.memory == b.memory &&
         a.configured == b.configured);
  ++checks;
}
static void setup(uint32_t a, unsigned denied, unsigned permissions,
                  unsigned user) {
  base = address = a;
  for (unsigned i = 0; i < 40; ++i)
    arena[i] = (unsigned char)(0x80 + i);
  mirror[0] = 0;
  GamePkgX86Region regions[41];
  size_t count = 0;
  for (unsigned i = 0; i < 40; ++i) {
    if (i == denied && permissions == 4)
      continue;
    regions[count++] =
        (GamePkgX86Region){a + i, 1, i == denied ? permissions : 3, arena + i};
  }
  regions[count++] = (GamePkgX86Region){0x4000, 1, 3, mirror};
  memory = gamepkg_x86_memory_create(regions, count, user);
  cpu = gamepkg_x86_cpu_create();
  assert(memory && cpu);
  argument_memory = memory;
  gamepkg_x86_memory_bind(memory);
  assert(!gamepkg_x86_cpu_require_irql(cpu));
  const GamePkgX86Tables tables = {{0, 0}, {0, 0}};
  assert(!gamepkg_x86_cpu_init_tables(cpu, &tables));
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread) &&
         !gamepkg_x86_cpu_init_code(thread, &code));
  memset(&fp, 0xa5, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(xmm, 0x5a, sizeof(xmm));
  gamepkg_x86_x87_bind_segments(8, 16, 16, 24);
  g_eflags = 0x202;
  g_ebp = 0x5678;
  g_esp = 0x7000;
  if (!user) {
    GamePkgX86FpFault fault;
    assert(!gamepkg_x86_run_with_fp(initialize_priority, &fault, &source));
  }
}
static void teardown(void) {
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
static int touched(unsigned offset) {
  return kind ? offset < 32 || offset >= 36
              : offset < 16 && offset != 1 && offset != 3;
}
static void verify_image(void) {
  unsigned char expected[40];
  for (unsigned i = 0; i < 40; ++i)
    expected[i] = (unsigned char)(0x80 + i);
  if (kind) {
    memset(expected, 0, 32);
    expected[0] = (unsigned char)(8 + type);
    expected[2] = 10;
    memset(expected + 36, 0, 4);
  } else {
    expected[0] = (unsigned char)type;
    expected[2] = 4;
    put(expected + 4, signal_state);
  }
  put(expected + 8, base + 8);
  put(expected + 12, base + 8);
  assert(!memcmp(expected, arena, 40));
}
static void execute(void) {
  GamePkgX86FpFault before, after;
  GamePkgX86DispatchExit exit;
  g_ecx = base;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source));
  assert(gamepkg_x86_dispatch_run_cpu_kernel_events(
             entries, 1, 0x400000, &exit, &source, thread, NULL) == 2 &&
         exit.pc == 0x700000);
  assert(g_eax == get(arena) && g_ebx == get(arena + 4) &&
         g_edx == get(arena + 8) && g_esi == get(arena + 12) &&
         !gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
  ++executed;
}
int main(void) {
  for (kind = 0; kind < 2; ++kind)
    for (type = 0; type < 2; ++type)
      for (signal_state = 0; signal_state < (kind ? 1u : 2u); ++signal_state) {
        setup(0x2000, 40, 3, 0);
        run(0, 0, 0, 0);
        verify_image();
        execute();
        teardown();
        for (unsigned denied = 0; denied < 40; ++denied)
          for (unsigned permission = 1; permission <= 4; ++permission) {
            if (permission == 3)
              continue;
            setup(0x2000, denied, permission, 0);
            if (touched(denied) && permission != 2)
              run(14, permission == 1 ? 3 : 2, base + denied, 0);
            else {
              run(0, 0, 0, 0);
              verify_image();
            }
            teardown();
          }
      }
  for (kind = 0; kind < 2; ++kind) {
    setup(UINT32_MAX - 39, 40, 3, 0);
    type = 1;
    signal_state = 1;
    run(0, 0, 0, 0);
    verify_image();
    execute();
    address = UINT32_MAX - (kind ? 38u : 14u);
    run(0, 0, 0, -1);
    address = base;
    type = 2;
    run(0, 0, 0, -1);
    type = UINT32_MAX;
    run(0, 0, 0, -1);
    type = 0;
    if (!kind) {
      signal_state = 2;
      run(0, 0, 0, -1);
      signal_state = UINT32_MAX;
      run(0, 0, 0, -1);
      signal_state = 0;
    }
    argument_memory = NULL;
    run(0, 0, 0, -1);
    argument_memory = memory;
    gamepkg_x86_memory_bind(NULL);
    run(0, 0, 0, -1);
    gamepkg_x86_memory_bind(memory);
    for (unsigned i = 0; i < 4; ++i) {
      priority = (uint32_t[]){0, 1, 2, 31}[i];
      GamePkgX86FpFault fault;
      assert(!gamepkg_x86_run_with_fp(change_priority, &fault, &source));
      run(0, 0, 0, 0);
      verify_image();
    }
    priority = 0;
    GamePkgX86FpFault fault;
    assert(!gamepkg_x86_run_with_fp(change_priority, &fault, &source));
    GamePkgX86CpuExecution before;
    assert(!gamepkg_x86_cpu_snapshot_execution(thread, &before));
    GamePkgX86CpuExecution altered = before;
    altered.code.cs_base = 0x1000;
    assert(!gamepkg_x86_cpu_restore_execution(thread, &altered));
    run(0, 0, 0, -1);
    assert(!gamepkg_x86_cpu_restore_execution(thread, &before));
    teardown();
    setup(0x2000, 40, 3, 1);
    run(0, 0, 0, -1);
    teardown();
  }
  printf(
      "Dispatcher objects: %u transactional initialization cases, %u generated "
      "header reads; pending x87-MMX-XMM and IRQL preserved; pointer_bits=%u\n",
      checks, executed, (unsigned)(8 * sizeof(void *)));
  return 0;
}
