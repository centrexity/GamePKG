#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_irql.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_nv.h"
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
static GamePkgX86Nv *service;
static unsigned char arena[2048], high[4], mirror[1], expected_payload[256];
static uint32_t index, type_address, value_address, value_length,
    result_address, priority;
static GamePkgX86NvResult result;
static unsigned checks, executed, creation_checks;
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
static const GamePkgX86DispatchEntry entries[] = {
    {0x400000, gamepkg_cpu_block_00400000}};
static void invoke(void) {
  result =
      gamepkg_x86_nv_query(service, thread, index, type_address, value_address,
                           value_length, result_address, 0x1234, g_ebp);
}
static void initialize_priority(void) {
  assert(!gamepkg_x86_irql_initialize(thread, memory, 0x3000, 0x1234, g_ebp));
}
static void change_priority(void) {
  GamePkgX86IrqlResult r =
      gamepkg_x86_irql(thread, priority ? 1 : 2, priority, 0x1234, g_ebp);
  assert(!r.status);
}
static void reset(void) {
  memset(arena, 0xa5, sizeof(arena));
  memset(high, 0x5a, 4);
  index = 17;
  type_address = 0x1000;
  value_address = 0x1010;
  value_length = 4;
  result_address = 0x1600;
}
static void setup(unsigned denied, unsigned permission) {
  reset();
  mirror[0] = 0;
  GamePkgX86Region regions[6];
  size_t count = 0;
  if (denied < sizeof(arena)) {
    if (denied)
      regions[count++] = (GamePkgX86Region){0x1000, denied, 3, arena};
    if (permission != 4)
      regions[count++] =
          (GamePkgX86Region){0x1000 + denied, 1, permission, arena + denied};
    if (denied + 1 < sizeof(arena))
      regions[count++] = (GamePkgX86Region){
          0x1001 + denied, (uint32_t)sizeof(arena) - denied - 1, 3,
          arena + denied + 1};
  } else
    regions[count++] = (GamePkgX86Region){0x1000, sizeof(arena), 3, arena};
  regions[count++] = (GamePkgX86Region){0xfffffffc, 4, 3, high};
  regions[count++] = (GamePkgX86Region){0x3000, 1, 3, mirror};
  memory = gamepkg_x86_memory_create(regions, count, 0);
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
  g_ebp = 0x5678;
  g_esp = 0x2000;
  GamePkgX86FpFault fault;
  assert(!gamepkg_x86_run_with_fp(initialize_priority, &fault, &source));
  unsigned char misc[4], language[4], payload[256], serial[12];
  put(misc, 0x89abcdef);
  put(language, 1);
  for (unsigned i = 0; i < 256; ++i)
    expected_payload[i] = payload[i] = (unsigned char)(i ^ 0x96);
  for (unsigned i = 0; i < 12; ++i)
    serial[i] = (unsigned char)('0' + i % 10);
  const GamePkgX86NvValue values[] = {{17, 4, 4, misc},
                                      {7, 4, 4, language},
                                      {0xffff, 3, 256, payload},
                                      {0x100, 3, 12, serial}};
  service = gamepkg_x86_nv_create(cpu, memory, values, 4);
  assert(service);
  memset(misc, 0, 4);
  memset(language, 0, 4);
  memset(payload, 0, 256);
  memset(serial, 0, 12);
}
static void teardown(void) {
  gamepkg_x86_nv_release(service);
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
static void reference_write(unsigned char *a, unsigned char *h, uint32_t at,
                            const unsigned char *bytes, uint32_t n) {
  if (at >= 0xfffffffc) {
    assert(n <= 4 && at - 0xfffffffc <= 4 - n);
    memcpy(h + (at - 0xfffffffc), bytes, n);
  } else {
    assert(at >= 0x1000 && at - 0x1000 <= sizeof(arena) - n);
    memcpy(a + (at - 0x1000), bytes, n);
  }
}
static void run(unsigned vector, unsigned error, uint32_t bad,
                int expected_host, uint32_t expected_nt) {
  unsigned char expected[2048], expected_high[4];
  memcpy(expected, arena, sizeof(arena));
  memcpy(expected_high, high, 4);
  GamePkgX86FpFault before, after, fault;
  GamePkgX86CpuExecution old, now;
  GamePkgX86CpuIrql p, q;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source) &&
         !gamepkg_x86_cpu_snapshot_execution(thread, &old) &&
         !gamepkg_x86_cpu_snapshot_irql(thread, &p));
  int rc = gamepkg_x86_run_with_fp(invoke, &fault, &source);
  assert(rc == (vector ? 1 : 0));
  if (vector)
    assert(fault.core.vector == vector && fault.core.error_code == error &&
           fault.core.fault_address == bad && fault.core.eip == 0x1234 &&
           !memcmp(&before.fp, &fault.fp, sizeof(before.fp)));
  else {
    assert(result.status == expected_host);
    if (!expected_host) {
      assert(result.ntstatus == expected_nt);
      if (expected_nt != 0xc0000034u) {
        uint32_t bytes = index == 0xffff  ? 256
                         : index == 0x100 ? 12
                                          : 4,
                 registry = index == 0xffff || index == 0x100 ? 3 : 4;
        assert(result.type == registry && result.length == bytes);
        unsigned char w[4];
        put(w, bytes);
        if (result_address)
          reference_write(expected, expected_high, result_address, w, 4);
        if (!expected_nt) {
          put(w, registry);
          reference_write(expected, expected_high, type_address, w, 4);
          unsigned char image[1024] = {0};
          assert(value_length <= sizeof(image));
          if (index == 0xffff)
            memcpy(image, expected_payload, 256);
          else if (index == 0x100)
            for (unsigned i = 0; i < 12; ++i)
              image[i] = (unsigned char)('0' + i % 10);
          else
            put(image, index == 7 ? 1 : 0x89abcdef);
          reference_write(expected, expected_high, value_address, image,
                          value_length);
        }
      } else
        assert(!result.type && !result.length);
    }
  }
  assert(!memcmp(expected, arena, sizeof(arena)) &&
         !memcmp(expected_high, high, 4));
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before, &after, sizeof(before)) &&
         !gamepkg_x86_cpu_snapshot_execution(thread, &now) &&
         !memcmp(&old, &now, sizeof(old)) &&
         !gamepkg_x86_cpu_snapshot_irql(thread, &q) && p.level == q.level &&
         p.configured == q.configured && p.memory == q.memory);
  ++checks;
}
static void execute(void) {
  GamePkgX86FpFault before, after;
  GamePkgX86DispatchExit exit;
  g_ecx = type_address;
  g_edx = value_address;
  g_ebx = result_address;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source));
  assert(gamepkg_x86_dispatch_run_cpu_kernel_events(
             entries, 1, 0x400000, &exit, &source, thread, NULL) == 2 &&
         exit.pc == 0x700000);
  assert(g_eax == get(arena + type_address - 0x1000) &&
         g_esi == get(arena + value_address - 0x1000) &&
         g_edi == get(arena + result_address - 0x1000) &&
         !gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
  ++executed;
}
static void creation(void) {
  unsigned char data[257] = {0};
  GamePkgX86NvValue v = {17, 4, 4, data}, pair[2] = {v, v};
#define REJECT(vp, n)                                                          \
  do {                                                                         \
    assert(!gamepkg_x86_nv_create(cpu, memory, vp, n));                        \
    ++creation_checks;                                                         \
  } while (0)
  REJECT(pair, 2);
  REJECT(NULL, 1);
  REJECT(&v, SIZE_MAX);
  REJECT((const GamePkgX86NvValue *)((uintptr_t)&v + 1), 1);
  v.index = 0xdead;
  REJECT(&v, 1);
  v.index = 17;
  v.type = 2;
  REJECT(&v, 1);
  v.type = 4;
  v.length = 3;
  REJECT(&v, 1);
  v.length = 0;
  REJECT(&v, 1);
  v.type = 3;
  v.length = 257;
  REJECT(&v, 1);
  v.length = 4;
  v.data = NULL;
  REJECT(&v, 1);
  v.data = (const unsigned char *)(UINTPTR_MAX - 1);
  REJECT(&v, 1);
  GamePkgX86Nv *empty = gamepkg_x86_nv_create(cpu, memory, NULL, 0);
  assert(empty);
  gamepkg_x86_nv_release(empty);
#undef REJECT
}
int main(void) {
  setup(2048, 3);
  creation();
  const uint32_t keys[] = {17, 7, 0xffff, 0x100},
                 lengths[] = {0,   1,   3,   4,   5,   11,  12,
                              255, 256, 257, 511, 512, 513, 1024};
  for (unsigned k = 0; k < 4; ++k)
    for (unsigned l = 0; l < 14; ++l) {
      reset();
      index = keys[k];
      value_length = lengths[l];
      uint32_t required = k == 2 ? 256 : k == 3 ? 12 : 4;
      run(0, 0, 0, 0, value_length < required ? 0xc0000023u : 0);
    }
  reset();
  run(0, 0, 0, 0, 0);
  execute();
  for (unsigned ta = 0; ta < 8; ++ta)
    for (unsigned va = 0; va < 8; ++va)
      for (unsigned ra = 0; ra < 8; ++ra) {
        reset();
        type_address = 0x1000 + ta;
        value_address = 0x1000 + va;
        result_address = 0x1000 + ra;
        value_length = 12;
        run(0, 0, 0, 0, 0);
      }
  reset();
  result_address = 0;
  run(0, 0, 0, 0, 0);
  reset();
  value_length = 0;
  type_address = UINT32_MAX;
  value_address = UINT32_MAX;
  run(0, 0, 0, 0, 0xc0000023u);
  reset();
  index = 0xdeadbeef;
  type_address = value_address = result_address = UINT32_MAX;
  value_length = UINT32_MAX;
  run(0, 0, 0, 0, 0xc0000034u);
  for (unsigned k = 0; k < 3; ++k) {
    reset();
    index = (uint32_t[]){0, 8, 0xfffe}[k];
    type_address = value_address = result_address = UINT32_MAX;
    run(0, 0, 0, -2, 0);
  }
  reset();
  type_address = UINT32_MAX;
  run(0, 0, 0, -1, 0);
  reset();
  result_address = UINT32_MAX;
  run(0, 0, 0, -1, 0);
  reset();
  value_address = 0xfffffffc;
  value_length = 5;
  run(0, 0, 0, -1, 0);
  reset();
  value_length = UINT32_MAX;
  run(0, 0, 0, -1, 0);
  reset();
  value_address = 0xfffffffc;
  run(0, 0, 0, 0, 0);
  reset();
  type_address = 0xfffffffc;
  run(0, 0, 0, 0, 0);
  reset();
  result_address = 0xfffffffc;
  run(0, 0, 0, 0, 0);
  reset();
  value_address = 0;
  run(14, 2, 0, 0, 0);
  reset();
  type_address = 0;
  run(14, 2, 0, 0, 0);
  reset();
  gamepkg_x86_memory_bind(NULL);
  run(0, 0, 0, -1, 0);
  gamepkg_x86_memory_bind(memory);
  reset();
  for (unsigned i = 0; i < 4; ++i) {
    priority = (uint32_t[]){0, 1, 2, 31}[i];
    GamePkgX86FpFault f;
    assert(!gamepkg_x86_run_with_fp(change_priority, &f, &source));
    run(0, 0, 0, 0, 0);
  }
  priority = 0;
  GamePkgX86FpFault f;
  assert(!gamepkg_x86_run_with_fp(change_priority, &f, &source));
  GamePkgX86CpuExecution old, altered;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &old));
  altered = old;
  altered.code.cs_base = 0x1000;
  assert(!gamepkg_x86_cpu_restore_execution(thread, &altered));
  run(0, 0, 0, -1, 0);
  assert(!gamepkg_x86_cpu_restore_execution(thread, &old));
  assert(!gamepkg_x86_cpu_block(thread));
  assert(!gamepkg_x86_run_with_fp(invoke, &f, &source) && result.status == -1);
  assert(!gamepkg_x86_cpu_enter(thread));
  teardown();
  for (unsigned group = 0; group < 3; ++group) {
    unsigned start = group == 0   ? 0
                     : group == 1 ? 16
                                  : 1536,
             count = group == 1 ? 1024 : 4;
    for (unsigned i = 0; i < count; ++i)
      for (unsigned p = 1; p <= 4; ++p) {
        if (p == 3)
          continue;
        setup(start + i, p);
        value_length = 1024;
        if (p == 2)
          run(0, 0, 0, 0, 0);
        else
          run(14, p == 1 ? 3 : 2, 0x1000 + start + i, 0, 0);
        teardown();
      }
  }
  printf("NV queries: %u checked state/fault/alias cases, %u constructor "
         "rejections, %u generated reads; frozen values and pending FP/IRQL "
         "preserved; pointer_bits=%u\n",
         checks, creation_checks, executed, (unsigned)(8 * sizeof(void *)));
  return 0;
}
