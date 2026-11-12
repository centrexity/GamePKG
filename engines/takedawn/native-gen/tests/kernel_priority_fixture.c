#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_dpc.h"
#include "gamepkg_x86_irql.h"
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
_Static_assert(sizeof(void *) == 4, "big-endian32 required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static _Thread_local GamePkgX87State fp;
static _Thread_local uint32_t xmm[8][4];
static _Thread_local GamePkgX86CpuThread *thread;
static GamePkgX86Cpu *cpu;
static GamePkgX86Memory *memory;
static unsigned char mirror[1], record[28];
static _Thread_local uint32_t action, op, level, address;
static _Thread_local int status;
static _Thread_local GamePkgX86IrqlResult result;
static unsigned checks;
static const GamePkgX86CpuCodeState code = {8, 0, UINT32_MAX, 0xc9b, 0,
                                            0, 0, 0,          0};
static const GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 0};
static GamePkgX86FpSource banks(void) {
  GamePkgX86FpSource s = {
      &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
  return s;
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
static const GamePkgX86DispatchEntry entries[] = {/* ENTRIES */};
static void invoke(void) {
  if (action == 0)
    status =
        gamepkg_x86_irql_initialize(thread, memory, address, 0x1234, g_ebp);
  else if (action == 1)
    result = gamepkg_x86_irql(thread, op, level, 0x1234, g_ebp);
  else
    status = gamepkg_x86_dpc_initialize(thread, memory, address, 0xdeadbeef,
                                        0x12345678, 0x1234, g_ebp);
}
static void run(unsigned vector, unsigned error, uint32_t bad) {
  GamePkgX86FpSource s = banks();
  GamePkgX86FpFault before, after, fault;
  GamePkgX86CpuExecution old, now;
  GamePkgX86CpuIrql priority, next;
  unsigned char bytes[28], byte = mirror[0];
  memcpy(bytes, record, 28);
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &s) &&
         !gamepkg_x86_cpu_snapshot_execution(thread, &old) &&
         !gamepkg_x86_cpu_snapshot_irql(thread, &priority));
  int rc = gamepkg_x86_run_with_fp(invoke, &fault, &s);
  assert(rc == (vector ? 1 : 0));
  if (vector) {
    assert(fault.core.vector == vector && fault.core.error_code == error &&
           fault.core.fault_address == bad && fault.core.eip == 0x1234 &&
           !memcmp(&before.fp, &fault.fp, sizeof(before.fp)) &&
           !memcmp(bytes, record, 28) && byte == mirror[0]);
    assert(!gamepkg_x86_cpu_snapshot_irql(thread, &next) &&
           priority.level == next.level &&
           priority.configured == next.configured);
  }
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &s) &&
         !memcmp(&before, &after, sizeof(before)) &&
         !gamepkg_x86_cpu_snapshot_execution(thread, &now) &&
         !memcmp(&old, &now, sizeof(old)));
  ++checks;
}
static void change(uint32_t operation, uint32_t value, int expected) {
  action = 1;
  op = operation;
  level = value;
  run(0, 0, 0);
  assert(result.status == expected);
}
static void setup(unsigned mode) {
  memset(record, 0xa5, 28);
  mirror[0] = 0xa5;
  GamePkgX86Region regions[] = {{0x1000, 1, mode == 1 ? 1 : 3, mirror},
                                {0x2000, 3, 3, record},
                                {0x2003, 9, 1, record + 3},
                                {0x200c, 8, mode == 2 ? 1 : 3, record + 12},
                                {0x2014, 8, 1, record + 20}};
  memory = gamepkg_x86_memory_create(regions, 5, mode == 3);
  cpu = gamepkg_x86_cpu_create();
  assert(memory && cpu && !gamepkg_x86_cpu_require_irql(cpu) &&
         gamepkg_x86_cpu_require_irql(cpu) == -1);
  gamepkg_x86_memory_bind(memory);
  const GamePkgX86Tables tables = {{0, 0}, {0, 0}};
  assert(!gamepkg_x86_cpu_init_tables(cpu, &tables));
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread) &&
         !gamepkg_x86_cpu_init_code(thread, &code) &&
         gamepkg_x86_cpu_require_irql(cpu) == -1 &&
         gamepkg_x86_cpu_retire_other(thread, 0x202) == -1);
  memset(&fp, 0xa5, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(xmm, 0x5a, sizeof(xmm));
  g_eflags = 0x202;
  g_esp = 0x3000;
  g_ebp = 0x5678;
  gamepkg_x86_x87_bind_segments(8, 16, 16, 24);
}
static void teardown(void) {
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
static unsigned polls;
static int poll_stop(void *p, const GamePkgX86InterruptState *s, uint32_t k,
                     uint32_t pc, uint32_t bp) {
  (void)p;
  (void)s;
  (void)k;
  (void)pc;
  (void)bp;
  ++polls;
  return 1;
}
static void execute(uint32_t pc, int expected, uint32_t end) {
  GamePkgX86FpSource s = banks();
  GamePkgX86DispatchExit exit;
  GamePkgX86FpFault before, after;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &s));
  assert(gamepkg_x86_dispatch_run_cpu_kernel_events(entries, 2, pc, &exit, &s,
                                                    thread, NULL) == expected);
  assert(exit.pc == end &&
         !gamepkg_x86_capture_registers(0, g_ebp, &after, &s) &&
         !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
}
#ifdef _WIN32
static DWORD WINAPI worker(void *opaque) {
#else
static void *worker(void *opaque) {
#endif
  thread = opaque;
  gamepkg_x86_memory_bind(memory);
  assert(!gamepkg_x86_cpu_enter(thread) &&
         !gamepkg_x86_cpu_init_code(thread, &code));
  action = 0;
  address = 0x1000;
  g_eflags = 0x202;
  GamePkgX86FpSource s = banks();
  GamePkgX86FpFault fault;
  assert(!gamepkg_x86_run_with_fp(invoke, &fault, &s) && status == 0);
  for (unsigned i = 0; i < 128; ++i) {
    /* Another owner may have left its APC-level mirror behind. Dispatch
     * activates this continuation before any guest read. */
    GamePkgX86DispatchExit exit;
    assert(gamepkg_x86_dispatch_run_cpu_kernel_events(
               entries, 2, 0x400000, &exit, &s, thread, NULL) == 2 &&
           exit.pc == 0x700000 && g_eax == 0);
    action = 1;
    op = 1;
    level = 1;
    assert(!gamepkg_x86_run_with_fp(invoke, &fault, &s) && !result.status);
    assert(!gamepkg_x86_cpu_block(thread) && !gamepkg_x86_cpu_enter(thread));
    /* Entry synchronizes retained level1 even if another owner ran meanwhile.
     */
    assert(gamepkg_x86_dispatch_run_cpu_kernel_events(
               entries, 2, 0x400000, &exit, &s, thread, NULL) == 2 &&
           g_eax == 1);
    op = 2;
    level = 0;
    assert(!gamepkg_x86_run_with_fp(invoke, &fault, &s) && !result.status);
  }
  assert(!gamepkg_x86_cpu_block(thread));
  gamepkg_x86_memory_bind(NULL);
  return 0;
}
int main(void) {
  setup(0);
  GamePkgX86FpSource s = banks();
  GamePkgX86DispatchExit exit;
  assert(gamepkg_x86_dispatch_run_cpu_code_events(entries, 2, 0x400000, &exit,
                                                  &s, thread, NULL) == -1);
  action = 0;
  address = 0x1001;
  run(14, 2, 0x1001);
  address = 0x1000;
  run(0, 0, 0);
  assert(status == 0 && mirror[0] == 0);
  run(0, 0, 0);
  assert(status == -1);
  for (uint32_t old = 0; old < 32; ++old) {
    change(1, old, 0);
    for (uint32_t next = 0; next < 32; ++next) {
      if (next < old) {
        change(1, next, -2);
      }
      if (next > old) {
        change(2, next, -2);
      }
    }
    assert(gamepkg_x86_cpu_yield(thread) == (old >= 2));
    if (old >= 2) {
      assert(gamepkg_x86_cpu_block(thread) == 1 &&
             !gamepkg_x86_cpu_suspend(thread) &&
             !gamepkg_x86_cpu_enter(thread));
      GamePkgX86CpuExecution saved;
      assert(!gamepkg_x86_cpu_snapshot_execution(thread, &saved) &&
             !gamepkg_x86_cpu_restore_execution(thread, &saved) &&
             gamepkg_x86_cpu_block(thread) == 1);
    }
    execute(0x400000, 2, 0x700000);
    assert(g_eax == old);
    change(0, 0, 0);
    assert(result.level == old);
    change(2, 0, 0);
  }
  change(1, 32, -1);
  change(1, UINT32_MAX, -1);
  change(3, 0, -1);
  mirror[0] = 7;
  change(1, 2, -2);
  assert(mirror[0] == 7);
  execute(0x400000, 2, 0x700000);
  assert(g_eax == 0);
  change(1, 2, 0);
  const GamePkgX86CpuEventSource events = {NULL, poll_stop};
  assert(gamepkg_x86_dispatch_run_cpu_kernel_events(entries, 2, 0x400000, &exit,
                                                    &s, thread, &events) == 4 &&
         polls == 1);
  change(2, 0, 0);
  execute(0x400100, 3, 0x400107);
  assert(mirror[0] == 7);
  mirror[0] = 0;
  gamepkg_x86_memory_bind(NULL);
  change(0, 0, -1);
  gamepkg_x86_memory_bind(memory);
  action = 2;
  address = 0xfffffff0;
  run(0, 0, 0);
  assert(status == -1);
  address = 0x2000;
  run(0, 0, 0);
  assert(status == 0);
  for (unsigned i = 0; i < 28; ++i) {
    unsigned char expected =
        i == 0              ? 19
        : i == 1 || i == 2  ? 0
        : i >= 12 && i < 16 ? (unsigned char)(0xdeadbeefu >> (8 * (i - 12)))
        : i >= 16 && i < 20 ? (unsigned char)(0x12345678u >> (8 * (i - 16)))
                            : 0xa5;
    assert(record[i] == expected);
  }
  assert(!gamepkg_x86_cpu_block(thread));
  GamePkgX86CpuThread *workers[8];
#ifdef _WIN32
  HANDLE handles[8];
#else
  pthread_t handles[8];
#endif
  for (unsigned i = 0; i < 8; ++i) {
    workers[i] = gamepkg_x86_cpu_thread_create(cpu, &initial);
    assert(workers[i]);
#ifdef _WIN32
    handles[i] = CreateThread(NULL, 0, worker, workers[i], 0, NULL);
    assert(handles[i]);
#else
    assert(!pthread_create(&handles[i], NULL, worker, workers[i]));
#endif
  }
  for (unsigned i = 0; i < 8; ++i) {
#ifdef _WIN32
    assert(WaitForSingleObject(handles[i], INFINITE) == WAIT_OBJECT_0 &&
           CloseHandle(handles[i]));
#else
    assert(!pthread_join(handles[i], NULL));
#endif
    assert(!gamepkg_x86_cpu_thread_destroy(workers[i]));
  }
  assert(!gamepkg_x86_cpu_enter(thread));
  teardown();
  setup(1);
  action = 0;
  address = 0x1000;
  run(14, 3, 0x1000);
  teardown();
  setup(2);
  action = 2;
  address = 0x2000;
  run(14, 3, 0x200c);
  teardown();
  setup(3);
  action = 0;
  address = 0x1000;
  run(0, 0, 0);
  assert(status == -1);
  action = 2;
  address = 0x2000;
  run(0, 0, 0);
  assert(status == -1);
  teardown();
  puts("Kernel priority: checked transitions, generated boundaries, pending "
       "x87/MMX/XMM, transactional DPC and 1024 eight-worker activations "
       "passed");
  printf("Kernel priority checked transitions: %u\n", checks);
  return 0;
}
