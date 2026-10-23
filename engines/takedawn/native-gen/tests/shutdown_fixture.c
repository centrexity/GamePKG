#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_shutdown.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
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
static unsigned char head[8], records[256], stack[4096];
static GamePkgX86Memory *memory;
static GamePkgX86Cpu *cpu;
static _Thread_local GamePkgX86CpuThread *thread;
static GamePkgX86Shutdown *service;
static _Thread_local GamePkgX86ShutdownCall call;
static _Thread_local GamePkgX86ShutdownFrame frame;
static _Thread_local uint32_t action, address, add, call_esp, trap;
static _Thread_local int returned;
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
static unsigned checks, executed;
static const GamePkgX86CpuCodeState code = {8, 0, UINT32_MAX, 0xc9b, 0,
                                            0, 0, 0,          0};
static const GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 0};
static void put(unsigned char *p, uint32_t n) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(n >> (8 * i));
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
static const GamePkgX86DispatchEntry entries[] = {/* ENTRIES */};
static void invoke(void) {
  if (action == 0)
    returned = gamepkg_x86_shutdown_register(service, thread, address, add,
                                             0x1234, g_ebp);
  else if (action == 1)
    frame = gamepkg_x86_shutdown_prepare(service, thread, call, call_esp, trap,
                                         0x1234, g_ebp);
  else if (action == 2)
    call = gamepkg_x86_shutdown_next(service, thread, 0x1234, g_ebp);
  else
    returned = gamepkg_x86_shutdown_init(service, thread, 0x1234, g_ebp);
}
static void run(unsigned vector, unsigned error, uint32_t fault_address) {
  GamePkgX86FpFault before, after, fault;
  GamePkgX86CpuExecution old, now;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source));
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &old));
  unsigned char h[8], r[256], s[4096];
  memcpy(h, head, 8);
  memcpy(r, records, 256);
  memcpy(s, stack, 4096);
  int result = gamepkg_x86_run_with_fp(invoke, &fault, &source);
  assert(result == (vector ? 1 : 0));
  if (vector) {
    assert(fault.core.vector == vector && fault.core.error_code == error &&
           fault.core.fault_address == fault_address &&
           fault.core.eip == 0x1234 &&
           !memcmp(&fault.fp, &before.fp, sizeof(before.fp)) &&
           !memcmp(h, head, 8) && !memcmp(r, records, 256) &&
           !memcmp(s, stack, 4096));
  }
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before, &after, sizeof(before)) &&
         !gamepkg_x86_cpu_snapshot_execution(thread, &now) &&
         !memcmp(&old, &now, sizeof(old)));
  ++checks;
}
static void register_record(unsigned index, uint32_t priority,
                            unsigned callback) {
  put(records + index * 16, 0x400000 + callback * 0x40);
  put(records + index * 16 + 4, priority);
  address = 0x1100 + index * 16;
  add = 1;
  action = 0;
  run(0, 0, 0);
  assert(returned == 0);
}
static void setup(unsigned special) {
  memset(head, 0xa5, 8);
  memset(records, 0, sizeof(records));
  memset(stack, 0xa5, sizeof(stack));
  GamePkgX86Region regions[] = {
      {0x1000, 8, 3, head},
      {0x1100, 16, 3, records},
      {0x1110, 16, special == 1 ? 1 : 3, records + 16},
      {0x1120, 224, 3, records + 32},
      {0x2000, 4096, special == 2 ? 1 : 3, stack}};
  memory = gamepkg_x86_memory_create(regions, 5, 0);
  cpu = gamepkg_x86_cpu_create();
  assert(memory && cpu);
  gamepkg_x86_memory_bind(memory);
  const GamePkgX86Tables tables = {{0, 0}, {0, 0}};
  assert(!gamepkg_x86_cpu_init_tables(cpu, &tables));
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread) &&
         !gamepkg_x86_cpu_init_code(thread, &code));
  memset(&fp, 0xa5, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(xmm, 0x5a, sizeof(xmm));
  g_eflags = 0x202;
  g_esp = 0x3000;
  g_ebp = 0x5678;
  g_fs_base = 0;
  g_df = 0;
  gamepkg_x86_x87_bind_segments(8, 16, 16, 24);
  service = gamepkg_x86_shutdown_create(cpu, memory, 0x1000);
  assert(service);
  action = 3;
  run(0, 0, 0);
  assert(returned == 0 && get(head) == 0x1000 && get(head + 4) == 0x1000);
}
static void teardown(unsigned abandon) {
  assert(!gamepkg_x86_shutdown_release(service, thread, abandon));
  assert(!gamepkg_x86_cpu_retire_other(thread, g_eflags) &&
         !gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
static void reserve(unsigned index, unsigned cb) {
  action = 2;
  run(0, 0, 0);
  assert(call.status == 1 && call.registration == 0x1100 + index * 16 &&
         call.pc == 0x400000 + cb * 0x40);
}
static void execute_callback(unsigned index, unsigned cb) {
  action = 1;
  call_esp = g_esp;
  trap = 0x700000;
  run(0, 0, 0);
  assert(frame.status == 0 && frame.esp == g_esp - 8 &&
         get(stack + frame.esp - 0x2000) == trap &&
         get(stack + frame.esp - 0x2000 + 4) == 0x1100 + index * 16);
  assert(gamepkg_x86_shutdown_complete(service, thread, call, trap,
                                       g_esp - 4) == -1);
  g_esp = frame.esp;
  GamePkgX86FpFault before, after;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source));
  GamePkgX86DispatchExit out;
  int result = gamepkg_x86_dispatch_run_cpu_code_events(
      entries, 4, frame.pc, &out, &source, thread, NULL);
  if (cb == 3) {
    assert(result == 1 && out.fault.core.vector == 14 &&
           out.fault.core.fault_address == 0xdeadc000);
    assert(gamepkg_x86_shutdown_complete(service, thread, call, out.pc,
                                         g_esp) == -1);
    return;
  }
  assert(result == 2 && out.pc == trap && g_esp == 0x3000 &&
         get(records + index * 16 + 4) == 0xaabb0000 + cb &&
         !gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
  assert(!gamepkg_x86_shutdown_complete(service, thread, call, out.pc, g_esp));
  assert(gamepkg_x86_shutdown_complete(service, thread, call, out.pc, g_esp) ==
         -1);
  ++executed;
}
static void ordering(void) {
  setup(0);
  for (unsigned i = 0; i < 6; ++i)
    register_record(
        i,
        (uint32_t[]){0x80000000, 0, 0x7fffffff, 0xffffffff, 0, 0x80000001}[i],
        i % 3);
  assert(get(head) == 0x1128 && get(head + 4) == 0x1108);
  unsigned char saved[256], h[8];
  memcpy(saved, records, 256);
  memcpy(h, head, 8);
  address = 0x1100;
  add = 1;
  action = 0;
  run(0, 0, 0);
  assert(returned == -2 && !memcmp(saved, records, 256) && !memcmp(h, head, 8));
  for (unsigned bad = 0; bad < 3; ++bad) {
    address = (uint32_t[]){0x1000, 0x1104, 0xfffffff8}[bad];
    run(0, 0, 0);
    assert(returned == -2);
  }
  action = 2;
  run(0, 0, 0);
  assert(call.status == -1);
  assert(!gamepkg_x86_shutdown_begin(service, thread));
  unsigned order[] = {2, 1, 4, 3, 5, 0};
  for (unsigned j = 0; j < 6; ++j) {
    unsigned i = order[j];
    reserve(i, i % 3);
    if (j == 0) {
      GamePkgX86ShutdownCall old = call;
      action = 2;
      run(0, 0, 0);
      assert(call.status == -1);
      call = old;
    }
    execute_callback(i, i % 3);
  }
  action = 2;
  run(0, 0, 0);
  assert(call.status == 0 && get(head) == 0x1000 && get(head + 4) == 0x1000);
  address = 0x1100;
  action = 0;
  add = 1;
  run(0, 0, 0);
  assert(returned == -1);
  add = 0;
  run(0, 0, 0);
  assert(returned == 0);
  teardown(0);
}
static void mutations(void) {
  setup(0);
  register_record(0, 0, 0);
  register_record(1, 10, 1);
  put(records + 4, 100);
  put(records + 16, 0x400080);
  assert(!gamepkg_x86_shutdown_begin(service, thread));
  reserve(1, 2);
  execute_callback(1, 2);
  reserve(0, 0);
  register_record(2, 50, 1);
  address = 0x1120;
  add = 0;
  action = 0;
  run(0, 0, 0);
  assert(!returned);
  register_record(0, 20, 2);
  execute_callback(0, 0);
  GamePkgX86ShutdownCall stale = call;
  reserve(0, 2);
  assert(gamepkg_x86_shutdown_complete(service, thread, stale, 0x700000,
                                       0x3000) == -1);
  execute_callback(0, 2);
  action = 2;
  run(0, 0, 0);
  assert(!call.status);
  teardown(0);
}
static void failures(void) {
  setup(1);
  register_record(0, 0, 0);
  put(records + 16, 0x400040);
  address = 0x1110;
  add = 1;
  action = 0;
  run(14, 3, 0x1118);
  assert(get(head) == 0x1108 && get(head + 4) == 0x1108);
  address = 0xdeadc000;
  run(14, 0, 0xdeadc000);
  uint32_t old = get(head);
  put(head, 0xdeadc000);
  address = 0x1120;
  put(records + 32, 0x400000);
  run(0, 0, 0);
  assert(returned == -2);
  put(head, old);
  old = get(records + 8);
  put(records + 8, 0x1118);
  run(0, 0, 0);
  assert(returned == -2);
  put(records + 8, old);
  assert(!gamepkg_x86_shutdown_begin(service, thread));
  reserve(0, 0);
  action = 1;
  call_esp = 2;
  trap = 0x700000;
  run(12, 0, 0);
  GamePkgX86CpuExecution limited;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &limited));
  limited.code.cs_limit = 0x3fffff;
  assert(!gamepkg_x86_cpu_restore_execution(thread, &limited));
  call_esp = 0x3000;
  trap = 0x100000;
  run(13, 0, 0);
  limited.code.cs_limit = UINT32_MAX;
  assert(!gamepkg_x86_cpu_restore_execution(thread, &limited));
  execute_callback(0, 0);
  action = 2;
  run(0, 0, 0);
  assert(!call.status);
  teardown(0);
  setup(2);
  register_record(0, 0, 0);
  assert(!gamepkg_x86_shutdown_begin(service, thread));
  reserve(0, 0);
  action = 1;
  call_esp = 0x3000;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &limited));
  limited.code.cs_limit = 0x3fffff;
  assert(!gamepkg_x86_cpu_restore_execution(thread, &limited));
  trap = 0x100000;
  run(14, 3, 0x2ff8);
  teardown(1);
  setup(0);
  register_record(0, 0, 3);
  assert(!gamepkg_x86_shutdown_begin(service, thread));
  reserve(0, 3);
  execute_callback(0, 3);
  teardown(1);
}
static void guards(void) {
  setup(0);
  GamePkgX86Memory *other = gamepkg_x86_memory_create(NULL, 0, 0);
  assert(other);
  gamepkg_x86_memory_bind(other);
  assert(gamepkg_x86_shutdown_register(service, thread, 0x1100, 1, 0, 0) == -1);
  gamepkg_x86_memory_bind(memory);
  gamepkg_x86_memory_release(other);
  other = gamepkg_x86_memory_create(NULL, 0, 1);
  assert(other);
  gamepkg_x86_memory_bind(other);
  assert(!gamepkg_x86_shutdown_create(cpu, other, 0x1000));
  gamepkg_x86_memory_bind(memory);
  gamepkg_x86_memory_release(other);
  GamePkgX86Cpu *foreign = gamepkg_x86_cpu_create();
  assert(foreign);
  const GamePkgX86Tables tables = {{0, 0}, {0, 0}};
  assert(!gamepkg_x86_cpu_init_tables(foreign, &tables));
  GamePkgX86CpuThread *ft = gamepkg_x86_cpu_thread_create(foreign, &initial);
  assert(ft && !gamepkg_x86_cpu_enter(ft) &&
         !gamepkg_x86_cpu_init_code(ft, &code));
  assert(gamepkg_x86_shutdown_begin(service, ft) == -1);
  assert(!gamepkg_x86_cpu_block(ft) && !gamepkg_x86_cpu_thread_destroy(ft));
  gamepkg_x86_cpu_release(foreign);
  assert(!gamepkg_x86_cpu_block(thread));
  assert(gamepkg_x86_shutdown_register(service, thread, 0x1100, 1, 0, 0) == -1);
  assert(!gamepkg_x86_cpu_enter(thread));
  GamePkgX86CpuExecution e;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &e));
  e.code.cs_base = 0x1000;
  assert(!gamepkg_x86_cpu_restore_execution(thread, &e));
  assert(gamepkg_x86_shutdown_begin(service, thread) == -1);
  assert(!gamepkg_x86_shutdown_release(service, thread, 1));
  service = NULL;
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
#ifdef _WIN32
static DWORD WINAPI worker(LPVOID p)
#else
static void *worker(void *p)
#endif
{
  unsigned id = (unsigned)(uintptr_t)p;
  gamepkg_x86_memory_bind(memory);
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread) &&
         !gamepkg_x86_cpu_init_code(thread, &code));
  g_eflags = 0x202;
  g_ebp = 0x5678;
  for (unsigned n = 0; n < 128; ++n) {
    address = 0x1100 + id * 16;
    action = 0;
    add = 1;
    GamePkgX86Fault f;
    assert(!gamepkg_x86_run(invoke, &f) && returned == 0);
    add = 0;
    assert(!gamepkg_x86_run(invoke, &f) && returned == 0);
  }
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_memory_bind(NULL);
  return 0;
}
static void workers(void) {
  setup(0);
  for (unsigned n = 0; n < 8; ++n) {
    put(records + n * 16, 0x400000);
    put(records + n * 16 + 4, n);
  }
  GamePkgX86CpuThread *parent = thread;
  assert(!gamepkg_x86_cpu_block(thread));
#ifdef _WIN32
  HANDLE handles[8];
  for (unsigned i = 0; i < 8; ++i) {
    handles[i] = CreateThread(NULL, 0, worker, (void *)(uintptr_t)i, 0, NULL);
    assert(handles[i]);
  }
  assert(WaitForMultipleObjects(8, handles, TRUE, INFINITE) == WAIT_OBJECT_0);
  for (unsigned i = 0; i < 8; ++i)
    assert(CloseHandle(handles[i]));
#else
  pthread_t handles[8];
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_create(&handles[i], NULL, worker, (void *)(uintptr_t)i));
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_join(handles[i], NULL));
#endif
  thread = parent;
  assert(!gamepkg_x86_cpu_enter(thread) &&
         !gamepkg_x86_shutdown_begin(service, thread));
  action = 2;
  run(0, 0, 0);
  assert(!call.status);
  teardown(0);
}
int main(void) {
  ordering();
  mutations();
  failures();
  guards();
  workers();
  printf("Shutdown: %u checked transitions, %u executed guest callbacks, "
         "signed ordering/mutation/fault/alias/domain gates and1024 "
         "eight-worker register-remove cycles passed; pointer_bits=%zu\n",
         checks, executed, sizeof(void *) * 8);
  return 0;
}
