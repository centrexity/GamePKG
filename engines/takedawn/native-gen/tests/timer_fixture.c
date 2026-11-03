#include "gamepkg_x86_cpu_code.h"
#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_irql.h"
#include "gamepkg_x86_timer.h"
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
static GamePkgX86Memory *memory;
static GamePkgX86DpcQueue *dpcs;
static GamePkgX86Timers *timers;
static unsigned char controls[64], objects[2048], stack[4096], spare[4096];
static uint32_t action, address, callback, arg1, arg2, period,
    return_trap = 0x700000, frame_esp = 0x4000, priority;
static uint64_t due;
static GamePkgX86Clock now;
static GamePkgX86TimerResult timer_result;
static GamePkgX86TimerPump pump_result;
static GamePkgX86DpcCall call;
static GamePkgX86DpcFrame frame;
static int status;
static unsigned checks, callbacks;
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
uint32_t xbox_MmioLoad(uint32_t a, int n) {
  uint32_t pc, bp, w[4];
  assert(!gamepkg_x86_dispatch_site(&pc, &bp));
  gamepkg_x86_memory_read(a, w, (uint32_t)n, pc, bp);
  return w[0];
}
void xbox_MmioStore(uint32_t a, uint32_t v, int n) {
  uint32_t pc, bp, w[4] = {v, 0, 0, 0};
  assert(!gamepkg_x86_dispatch_site(&pc, &bp));
  gamepkg_x86_memory_write(a, w, (uint32_t)n, (1u << n) - 1, 0, pc, bp);
}
/* PROGRAM */
static const GamePkgX86DispatchEntry entries[] = {/* ENTRIES */};
static void invoke(void) {
  if (action == 0)
    timer_result = gamepkg_x86_timer_set(timers, thread, address, due, period,
                                         callback, 0x1234, g_ebp);
  else if (action == 1)
    timer_result =
        gamepkg_x86_timer_cancel(timers, thread, address, 0x1234, g_ebp);
  else if (action == 2)
    pump_result = gamepkg_x86_timers_pump(timers, thread, 0x1234, g_ebp);
  else if (action == 3)
    status = gamepkg_x86_dpc_queue_insert(dpcs, thread, address, arg1, arg2,
                                          0x1234, g_ebp);
  else if (action == 4)
    status = gamepkg_x86_dpc_queue_remove(dpcs, thread, address, 0x1234, g_ebp);
  else if (action == 5)
    call = gamepkg_x86_dpc_queue_claim(dpcs, thread, 0x1234, g_ebp);
  else if (action == 6)
    frame = gamepkg_x86_dpc_queue_prepare(dpcs, thread, call, frame_esp,
                                          return_trap, 0x1234, g_ebp);
  else if (action == 7)
    status = gamepkg_x86_dpc_queue_complete(dpcs, thread, call, return_trap,
                                            frame_esp, 0x1234, g_ebp);
  else if (action == 8)
    status = gamepkg_x86_dpc_queue_init(dpcs, thread, 0x1234, g_ebp);
  else if (action == 9)
    status = gamepkg_x86_timers_init(timers, thread, 0x1234, g_ebp);
}
static void initialize_priority(void) {
  assert(!gamepkg_x86_irql_initialize(thread, memory, 0x1020, 0x1234, g_ebp));
}
static void set_priority(void) {
  GamePkgX86IrqlResult r =
      gamepkg_x86_irql(thread, priority ? 1 : 2, priority, 0x1234, g_ebp);
  assert(!r.status);
}
static void run(unsigned vector, unsigned error, uint32_t bad) {
  unsigned char a[64], b[2048], c[4096], d[4096];
  memcpy(a, controls, 64);
  memcpy(b, objects, 2048);
  memcpy(c, stack, 4096);
  memcpy(d, spare, 4096);
  GamePkgX86FpFault before, after, fault;
  GamePkgX86CpuExecution old, next;
  GamePkgX86CpuIrql p, q;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source) &&
         !gamepkg_x86_cpu_snapshot_execution(thread, &old) &&
         !gamepkg_x86_cpu_snapshot_irql(thread, &p));
  int rc = gamepkg_x86_run_with_fp(invoke, &fault, &source);
  assert(rc == (vector ? 1 : 0));
  if (vector)
    assert(fault.core.vector == vector && fault.core.error_code == error &&
           fault.core.fault_address == bad && fault.core.eip == 0x1234 &&
           !memcmp(&before.fp, &fault.fp, sizeof(before.fp)) &&
           !memcmp(a, controls, 64) && !memcmp(b, objects, 2048) &&
           !memcmp(c, stack, 4096) && !memcmp(d, spare, 4096));
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before, &after, sizeof(before)) &&
         !gamepkg_x86_cpu_snapshot_execution(thread, &next) &&
         !memcmp(&old, &next, sizeof(old)) &&
         !gamepkg_x86_cpu_snapshot_irql(thread, &q));
  if (vector || (action != 5 && action != 7))
    assert(q.level == p.level);
  ++checks;
}
static void record_timer(unsigned i, unsigned type) {
  unsigned char *b = objects + 0x200 + i * 64;
  memset(b, 0, 40);
  b[0] = (unsigned char)(8 + type);
  b[2] = 10;
  put(b + 8, 0x2200 + i * 64 + 8);
  put(b + 12, 0x2200 + i * 64 + 8);
}
static void record_dpc(unsigned i, uint32_t pc) {
  unsigned char *b = objects + i * 64;
  memset(b, 0xa5, 28);
  put(b, 0xa5000013);
  b[2] = 0;
  put(b + 12, pc);
  put(b + 16, 0x2600 + i * 16);
}
static void setup(unsigned denied, unsigned permission, unsigned stack_ro) {
  memset(controls, 0xa5, 64);
  memset(objects, 0, 2048);
  memset(stack, 0xa5, 4096);
  memset(spare, 0xa5, 4096);
  controls[32] = 0;
  for (unsigned i = 0; i < 8; ++i) {
    record_timer(i, i & 1);
    record_dpc(i, 0x400000);
  }
  GamePkgX86Region r[8];
  size_t n = 0;
  r[n++] = (GamePkgX86Region){0x1000, 64, 3, controls};
  if (denied < 2048) {
    if (denied)
      r[n++] = (GamePkgX86Region){0x2000, denied, 3, objects};
    if (permission != 4)
      r[n++] =
          (GamePkgX86Region){0x2000 + denied, 1, permission, objects + denied};
    if (denied + 1 < 2048)
      r[n++] = (GamePkgX86Region){0x2001 + denied, 2048 - denied - 1, 3,
                                  objects + denied + 1};
  } else
    r[n++] = (GamePkgX86Region){0x2000, 2048, 3, objects};
  r[n++] = (GamePkgX86Region){0x3000, 4096, stack_ro ? 1 : 3, stack};
  r[n++] = (GamePkgX86Region){0x5000, 4096, 3, spare};
  memory = gamepkg_x86_memory_create(r, n, 0);
  cpu = gamepkg_x86_cpu_create();
  assert(memory && cpu);
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
  g_esp = 0x4000;
  g_ebp = 0x5678;
  GamePkgX86FpFault f;
  assert(!gamepkg_x86_run_with_fp(initialize_priority, &f, &source));
  dpcs = gamepkg_x86_dpc_queue_create(cpu, memory, 0x1000);
  assert(dpcs);
  action = 8;
  run(0, 0, 0);
  assert(!status);
  now = (GamePkgX86Clock){100, 1000};
  timers = gamepkg_x86_timers_create(cpu, memory, dpcs, 0x1010, now);
  assert(timers);
  action = 9;
  run(0, 0, 0);
  assert(!status);
  return_trap = 0x700000;
  frame_esp = 0x4000;
}
static void teardown(void) {
  assert(!gamepkg_x86_timers_release(timers, thread, 0) &&
         !gamepkg_x86_dpc_queue_release(dpcs, thread, 0) &&
         !gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
static void arm(uint32_t timer, uint64_t target, uint32_t repeat, uint32_t dpc,
                unsigned old) {
  action = 0;
  address = timer;
  due = target;
  period = repeat;
  callback = dpc;
  run(0, 0, 0);
  assert(!timer_result.status && timer_result.previous == old);
}
static void cancel(uint32_t timer, unsigned old) {
  action = 1;
  address = timer;
  run(0, 0, 0);
  assert(!timer_result.status && timer_result.previous == old);
}
static void clock_to(uint64_t interrupt, uint64_t system) {
  now = (GamePkgX86Clock){interrupt, system};
  assert(!gamepkg_x86_timers_clock(timers, thread, now));
}
static void pump(int expected) {
  action = 2;
  run(0, 0, 0);
  assert(pump_result.status == expected);
}
static void insert(uint32_t dpc, uint32_t a, uint32_t b, int expected) {
  action = 3;
  address = dpc;
  arg1 = a;
  arg2 = b;
  run(0, 0, 0);
  assert(status == expected);
}
static void remove_dpc(uint32_t dpc, int expected) {
  action = 4;
  address = dpc;
  run(0, 0, 0);
  assert(status == expected);
}
static void claim(int expected) {
  action = 5;
  run(0, 0, 0);
  assert(call.status == expected);
}
static void hle_set(void) {
  uint32_t w[4];
  gamepkg_x86_memory_read(g_esp + 4, w, 16, 0x700100, g_ebp);
  GamePkgX86TimerResult r = gamepkg_x86_timer_set(
      timers, thread, w[0], (uint64_t)w[1] | ((uint64_t)w[2] << 32), 0, w[3],
      0x700100, g_ebp);
  assert(!r.status);
  g_eax = (g_eax & 0xffffff00u) | r.previous;
  GamePkgX86NearReturn ret =
      gamepkg_x86_cpu_plan_return(thread, g_esp, 4, 16, 0x700100, g_ebp);
  assert(!ret.status);
  g_esp = ret.esp;
  g_ecx = ret.offset;
  assert(!gamepkg_x86_cpu_retire_other(thread, g_eflags));
}
static void execute(unsigned faulted) {
  action = 6;
  run(0, 0, 0);
  assert(!frame.status);
  g_esp = frame.esp;
  GamePkgX86DispatchExit out;
  uint32_t pc = frame.pc;
  GamePkgX86FpFault before, after;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source));
  for (;;) {
    int rc = gamepkg_x86_dispatch_run_cpu_kernel_events(
        entries, sizeof(entries) / sizeof(entries[0]), pc, &out, &source,
        thread, NULL);
    if (rc == 2 && out.pc == 0x700100) {
      GamePkgX86FpFault f;
      assert(!gamepkg_x86_run_with_fp(hle_set, &f, &source));
      pc = g_ecx;
      continue;
    }
    if (faulted) {
      assert(rc == 1 && out.pc == 0x400100 && out.fault.core.vector == 14 &&
             out.fault.core.fault_address == 0xdeadbeef &&
             gamepkg_x86_cpu_block(thread) == 1);
      assert(gamepkg_x86_dpc_queue_complete(dpcs, thread, call, out.pc, g_esp,
                                            0x1234, g_ebp) == -1);
      g_edx = 0x2600;
      pc = 0x400100;
      faulted = 0;
      continue;
    }
    if (rc != 2 || out.pc != return_trap || g_esp != frame_esp)
      fprintf(stderr,
              "DPC #%u entry=%08x rc=%d pc=%08x esp=%08x expected=%08x/%08x "
              "fault=%u/%u at=%08x\n",
              callbacks, frame.pc, rc, out.pc, g_esp, return_trap, frame_esp,
              out.fault.core.vector, out.fault.core.error_code,
              out.fault.core.fault_address);
    assert(rc == 2 && out.pc == return_trap && g_esp == frame_esp);
    break;
  }
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
  assert(gamepkg_x86_dpc_queue_complete(dpcs, thread, call, return_trap + 1,
                                        frame_esp, 0x1234, g_ebp) == -1);
  action = 7;
  run(0, 0, 0);
  assert(!status);
  run(0, 0, 0);
  assert(status == -1);
  ++callbacks;
}
typedef struct ClockCase {
  uint64_t i0, s0, due, i1, s1, next;
  uint32_t period, expired, armed, signal;
} ClockCase;
static const ClockCase clock_cases[] = {
    /* CLOCK_CASES */
};
int main(void) {
  for (unsigned i = 0; i < sizeof(clock_cases) / sizeof(clock_cases[0]); ++i) {
    const ClockCase *c = &clock_cases[i];
    setup(2048, 3, 0);
    clock_to(c->i0, c->s0);
    arm(0x2200, c->due, c->period, 0, 0);
    clock_to(c->i1, c->s1);
    action = 2;
    run(0, 0, 0);
    if (pump_result.status == 2)
      run(0, 0, 0);
    assert(pump_result.status == (int)c->expired);
    assert(objects[0x203] == c->armed && get(objects + 0x204) == c->signal);
    if (c->expired && c->period)
      assert(((uint64_t)get(objects + 0x210) |
              ((uint64_t)get(objects + 0x214) << 32)) == c->next);
    cancel(0x2200, c->armed);
    teardown();
  }
  setup(2048, 3, 0);
  arm(0x2200, UINT64_C(0) - 100, 0, 0x2000, 0);
  assert(objects[0x203] == 1 && get(objects + 0x210) == 200);
  pump(0);
  clock_to(199, 1099);
  pump(0);
  clock_to(200, 1100);
  pump(1);
  assert(pump_result.queued == 1 && get(objects + 0x204) == 1 &&
         objects[0x203] == 0);
  claim(1);
  assert(call.arg1 == 1100 && call.arg2 == 0 && call.pc == 0x400000 &&
         controls[32] == 2 && get(controls + 8) == 1 &&
         gamepkg_x86_cpu_yield(thread) == 1);
  execute(0);
  assert(get(objects + 0x600) == 1100 && get(objects + 0x608) == 0x2000 &&
         controls[32] == 0);
  pump(0);
  cancel(0x2200, 0);
  arm(0x2200, UINT64_C(0) - 216000000000, 0, 0x2000, 0);
  clock_to(216000000199, 216000001099);
  pump(0);
  clock_to(216000000200, 216000001100);
  pump(1);
  claim(1);
  execute(0);
  // Absolute time follows wall-clock changes; relative time follows interrupt
  // time.
  arm(0x2200, 216000010000, 0, 0, 0);
  clock_to(216000000300, 216000000000);
  pump(2);
  pump(0);
  clock_to(216000000301, 216000010000);
  pump(1);
  arm(0x2200, UINT64_C(0) - 100, 0, 0, 0);
  clock_to(216000000350, 0);
  pump(0);
  clock_to(216000000401, 0);
  pump(1);
  // Rearm/cancel, immediate signal, periodic coalescing and duplicate DPCs.
  arm(0x2200, UINT64_C(0) - 100, 0, 0x2000, 0);
  arm(0x2200, UINT64_C(0) - 200, 0, 0x2000, 1);
  cancel(0x2200, 1);
  cancel(0x2200, 0);
  pump(0);
  clock_to(300000000000, 1000);
  arm(0x2200, 0, 0, 0x2000, 0);
  assert(objects[0x203] == 0 && get(objects + 0x204) == 1);
  cancel(0x2200, 0);
  claim(1);
  execute(0);
  arm(0x2200, UINT64_C(0) - 10, 2, 0x2000, 0);
  clock_to(300000100009, 101009);
  pump(1);
  assert(pump_result.queued == 1 && objects[0x203] == 1);
  clock_to(300000120009, 121009);
  pump(1);
  assert(pump_result.queued == 0);
  claim(1);
  assert(call.arg1 == 101009);
  execute(0);
  cancel(0x2200, 1);
  // A generated callback performs a real checked import to rearm itself.
  put(objects + 12, 0x400200);
  arm(0x2200, UINT64_C(0) - 1, 0, 0x2000, 0);
  clock_to(300000120010, 121010);
  pump(1);
  claim(1);
  execute(0);
  assert(objects[0x203] == 1 && get(objects + 0x204) == 0);
  cancel(0x2200, 1);
  put(objects + 12, 0x400000);
  insert(0x2000, 11, 22, 1);
  insert(0x2000, 33, 44, 0);
  insert(0x2040, 55, 66, 1);
  claim(1);
  assert(call.arg1 == 11);
  insert(0x2000, 77, 88, 1);
  execute(0);
  claim(1);
  assert(call.dpc == 0x2040 && call.arg1 == 55);
  execute(0);
  claim(1);
  assert(call.dpc == 0x2000 && call.arg1 == 77);
  execute(0);
  remove_dpc(0x2000, 0);
  insert(0x2000, 1, 2, 1);
  insert(0x2040, 3, 4, 1);
  remove_dpc(0x2000, 1);
  remove_dpc(0x2040, 1);
  // IF/STI and priority inhibition retain queued work.
  insert(0x2000, 1, 2, 1);
  assert(!gamepkg_x86_cpu_step(thread, g_eflags, 0));
  g_eflags &= ~0x200u;
  claim(0);
  assert(!gamepkg_x86_cpu_step(thread, g_eflags, 1));
  g_eflags |= 0x200;
  claim(0);
  assert(!gamepkg_x86_cpu_retire_other(thread, g_eflags));
  priority = 2;
  GamePkgX86FpFault f;
  assert(!gamepkg_x86_run_with_fp(set_priority, &f, &source));
  claim(0);
  priority = 0;
  assert(!gamepkg_x86_run_with_fp(set_priority, &f, &source));
  claim(1);
  execute(0);
  // A faulted DPC stays active until its operand is repaired and real RET16
  // runs.
  put(objects + 12, 0x400100);
  insert(0x2000, 1, 2, 1);
  claim(1);
  g_edx = 0xdeadbeef;
  execute(1);
  put(objects + 12, 0x400000);
  // Corrupt and overlapping opaque records reject without pointer chasing.
  insert(0x2000, 1, 2, 1);
  uint32_t saved = get(objects + 4);
  put(objects + 4, 0xdeadbeef);
  insert(0x2040, 3, 4, -2);
  put(objects + 4, saved);
  remove_dpc(0x2000, 1);
  insert(0xfffffff0, 1, 2, -2);
  insert(0x1000, 1, 2, -2);
  arm(0x2200, UINT64_C(0) - 100, 0, 0, 0);
  saved = get(objects + 0x210);
  put(objects + 0x210, saved + 1);
  action = 1;
  address = 0x2200;
  run(0, 0, 0);
  assert(timer_result.status == -2);
  put(objects + 0x210, saved);
  cancel(0x2200, 1);
  action = 0;
  address = 0x2200;
  due = 0;
  period = 0x80000000u;
  callback = 0;
  run(0, 0, 0);
  assert(timer_result.status == -1);
  period = 0;
  address = 0xfffffff0;
  run(0, 0, 0);
  assert(timer_result.status == -2);
  assert(gamepkg_x86_timers_clock(timers, thread, (GamePkgX86Clock){1, 1}) ==
         -1);
  // INT64_MIN relative arithmetic and counter-end deadlines do not fire early.
  arm(0x2200, UINT64_C(1) << 63, 0, 0, 0);
  clock_to(UINT64_MAX, 0);
  pump(1);
  arm(0x2200, UINT64_C(0) - 100, 0, 0, 0);
  pump(0);
  cancel(0x2200, 1);
  teardown();
  // Every byte of the timer write and late DPC-argument writes can fault
  // atomically.
  for (unsigned part = 0; part < 2; ++part)
    for (unsigned i = 0; i < (part ? 8u : 40u); ++i) {
      unsigned denied = part ? 20 + i : 0x200 + i;
      setup(denied, 1, 0);
      action = 0;
      address = 0x2200;
      due = 0;
      period = 0;
      callback = 0x2000;
      run(14, 3, 0x2000 + denied);
      pump(0);
      teardown();
    }
  // Stack write admission wins over target GP. Both can be repaired without
  // discarding an active callback or lowering priority to force cleanup.
  setup(2048, 3, 1);
  put(objects + 12, 0x600000);
  insert(0x2000, 1, 2, 1);
  claim(1);
  GamePkgX86CpuExecution old, limited;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &old));
  limited = old;
  limited.code.cs_limit = 0x500fff;
  assert(!gamepkg_x86_cpu_restore_execution(thread, &limited));
  return_trap = 0x4ff000;
  action = 6;
  run(14, 3, 0x3fec);
  frame_esp = 0x6000;
  run(13, 0, 0);
  assert(!gamepkg_x86_cpu_restore_execution(thread, &old));
  execute(0);
  teardown();
  printf("Timers/DPC: %u checked transitions, %u completed generated "
         "callbacks, 1024 widened clock oracle rows; six-hour, clock shifts, "
         "periodic/cancel/requeue and "
         "fault/IRQL preservation passed\n",
         checks, callbacks);
  return 0;
}
