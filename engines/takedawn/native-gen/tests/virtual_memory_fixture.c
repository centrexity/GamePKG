#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_irql.h"
#include "gamepkg_x86_virtual_memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big endian required
#endif
_Static_assert(sizeof(void *) == 4, "actual32bit required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static unsigned char bank[8192];
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
static GamePkgX86Memory *memory;
static GamePkgX86Cpu *cpu;
static GamePkgX86CpuThread *thread;
static GamePkgX86VirtualMemory *service;
static uint32_t action, a = 0x2000, n = 0x2010, zero, flags, protection;
static GamePkgX86VirtualMemoryResult result;
static unsigned checks;
uint32_t xbox_MmioLoad(uint32_t address, int bytes) {
  uint32_t ip, bp, w[4];
  assert(!gamepkg_x86_dispatch_site(&ip, &bp));
  gamepkg_x86_memory_read(address, w, (uint32_t)bytes, ip, bp);
  return w[0];
}
void xbox_MmioStore(uint32_t address, uint32_t value, int bytes) {
  uint32_t ip, bp, w[4] = {value, 0, 0, 0};
  assert(!gamepkg_x86_dispatch_site(&ip, &bp));
  gamepkg_x86_memory_write(address, w, (uint32_t)bytes, (1u << bytes) - 1, 0,
                           ip, bp);
}
/* PROGRAM */
static const GamePkgX86DispatchEntry entries[] = {
    {0x400000, gamepkg_cpu_block_00400000},
    {0x400002, gamepkg_cpu_block_00400000},
    {0x400004, gamepkg_cpu_block_00400000}};
static void put(unsigned char *p, uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(v >> (i * 8));
}
static uint32_t get(const unsigned char *p) {
  return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) |
         ((uint32_t)p[3] << 24);
}
static void body(void) {
  if (!action) {
    assert(
        !gamepkg_x86_irql_initialize(thread, memory, 0x1000, 0x1234, 0x5678));
    return;
  }
  if (action == 3 || action == 4) {
    GamePkgX86IrqlResult p = gamepkg_x86_irql(
        thread, action == 3 ? 1 : 2, action == 3 ? 1 : 0, 0x1234, 0x5678);
    assert(!p.status);
    return;
  }
  result = action == 1
               ? gamepkg_x86_virtual_memory_allocate(service, thread, a, zero,
                                                     n, flags, protection,
                                                     0x1234, 0x5678)
               : gamepkg_x86_virtual_memory_free(service, thread, a, n, flags,
                                                 0x1234, 0x5678);
}
static void call(unsigned vector, unsigned error, uint32_t address) {
  GamePkgX86FpFault before, after, f;
  assert(!gamepkg_x86_capture_registers(0, 0, &before, &source));
  int rc = gamepkg_x86_run_with_fp(body, &f, &source);
  if (rc != (vector ? 1 : 0))
    fprintf(stderr, "vm action=%u rc=%d vector=%u/%u address=%08x\n", action,
            rc, f.core.vector, vector, f.core.fault_address);
  assert(rc == (vector ? 1 : 0));
  if (vector)
    assert(f.core.vector == vector && f.core.error_code == error &&
           f.core.fault_address == address &&
           !memcmp(&before.fp, &f.fp, sizeof(before.fp)));
  assert(!gamepkg_x86_capture_registers(0, 0, &after, &source) &&
         !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
  ++checks;
}
static void setup(unsigned denied) {
  memset(bank, 0, sizeof(bank));
  GamePkgX86Region r[3] = {
      {0x1000, denied, 3, bank},
      {0x1000 + denied, 1, 1, bank + denied},
      {0x1001 + denied, 8191 - denied, 3, bank + denied + 1}};
  if (denied == 8192)
    r[0] = (GamePkgX86Region){0x1000, 8192, 3, bank};
  memory = gamepkg_x86_memory_create(r, denied == 8192 ? 1 : 3, 0);
  assert(memory);
  assert(!gamepkg_x86_virtual_configure(memory, 0x10000, 0x7ffe0000, 8 * 4096));
  gamepkg_x86_memory_bind(memory);
  cpu = gamepkg_x86_cpu_create();
  GamePkgX86Tables tables = {{0, 0}, {0, 0}};
  assert(cpu && !gamepkg_x86_cpu_require_irql(cpu) &&
         !gamepkg_x86_cpu_init_tables(cpu, &tables));
  GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 0};
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread));
  GamePkgX86CpuCodeState code = {8, 0, UINT32_MAX, 0xc9b, 0, 0, 0, 0, 0};
  assert(!gamepkg_x86_cpu_init_code(thread, &code));
  memset(&fp, 0, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(fp.significand, 0xa5, sizeof(fp.significand));
  memset(xmm, 0x5a, sizeof(xmm));
  g_eflags = 0x202;
  action = 0;
  call(0, 0, 0);
  service = gamepkg_x86_virtual_memory_create(cpu, memory);
  assert(service);
  a = 0x2000;
  n = 0x2010;
  zero = 0;
}
static void teardown(void) {
  gamepkg_x86_virtual_memory_release(service);
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
static void allocate(uint32_t base, uint32_t size, uint32_t kind,
                     uint32_t protect, uint32_t status) {
  put(bank + 0x1000, base);
  put(bank + 0x1010, size);
  flags = kind;
  protection = protect;
  action = 1;
  call(0, 0, 0);
  assert(!result.status && result.ntstatus == status);
  if (!status)
    assert(get(bank + 0x1000) == result.base &&
           get(bank + 0x1010) == result.length);
  else
    assert(get(bank + 0x1000) == base && get(bank + 0x1010) == size);
}
static void free_range(uint32_t base, uint32_t size, uint32_t kind) {
  put(bank + 0x1000, base);
  put(bank + 0x1010, size);
  flags = kind;
  action = 2;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus);
}
static void generated(uint32_t pc, uint32_t base, int expected, uint32_t stop) {
  g_ecx = base;
  g_edx = base + 4;
  if (pc == 0x400000) {
    g_esp = 0x2f00;
    put(bank + 0x1f00, 0x700000);
  }
  GamePkgX86FpFault before, after;
  assert(!gamepkg_x86_capture_registers(0, 0, &before, &source));
  GamePkgX86DispatchExit out;
  int rc = gamepkg_x86_dispatch_run_cpu_kernel_events(
      entries, sizeof(entries) / sizeof(entries[0]), pc, &out, &source, thread,
      NULL);
  assert(rc == expected && out.pc == stop);
  if (rc == 1)
    assert(out.fault.core.vector == 14 &&
           out.fault.core.fault_address == (pc == 0x400000 ? base : base + 4));
  assert(!gamepkg_x86_capture_registers(0, 0, &after, &source) &&
         !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
  ++checks;
}
int main(void) {
  setup(8192);
  allocate(0, 1048576, 0x2000, 4, 0);
  uint32_t base = result.base;
  assert(base == 0x10000 && result.length == 1048576);
  GamePkgX86VirtualStats s;
  assert(!gamepkg_x86_virtual_stats(memory, &s) && !s.committed);
  generated(0x400000, base, 1, 0x400000);
  allocate(base + 4096, 4096, 0x1000, 4, 0);
  assert(result.base == base + 4096);
  allocate(base, 4096, 0x1000, 4, 0);
  generated(0x400000, base, 2, 0x700000);
  assert(!g_eax && g_esp == 0x2f04);
  assert(!gamepkg_x86_virtual_protect(memory, base, 4096, 2).status);
  g_ecx = base;
  g_edx = base + 4;
  g_esp = 0x2f00;
  put(bank + 0x1f00, 0x700000);
  generated(0x400002, base, 1, 0x400002);
  assert(!gamepkg_x86_virtual_protect(memory, base, 4096, 4).status);
  g_eax = 0x12345678;
  generated(0x400002, base, 2, 0x700000);
  // Parameters inside the affected mapping cannot be invalidated beneath
  // output stores. Until that alias ABI is qualified, stop without mutation.
  g_esp = 0x2f00;
  put(bank + 0x1f00, 0x700000);
  g_eax = base;
  generated(0x400002, base, 2, 0x700000);
  a = base + 4;
  n = 0x2010;
  put(bank + 0x1010, 4096);
  action = 1;
  flags = 0x1000;
  protection = 1;
  call(0, 0, 0);
  assert(result.status == -2 &&
         gamepkg_x86_virtual_query(memory, base).protection == 4);
  put(bank + 0x1010, 0);
  action = 2;
  flags = 0x8000;
  call(0, 0, 0);
  assert(result.status == -2 &&
         gamepkg_x86_virtual_query(memory, base).state == 0x1000);
  a = 0x2000;
  n = 0x2010;
  // Region rounding, existing-page content and partial release use real state.
  allocate(base + 1, 4096, 0x1000, 2, 0);
  assert(result.base == base && result.length == 8192);
  free_range(base + 4096, 4096, 0x8000);
  assert(gamepkg_x86_virtual_query(memory, base + 4096).state == 0x10000);
  allocate(base + 8192, 4096, 0x1000, 4, 0);
  free_range(base + 8192, 0, 0x4000);
  assert(gamepkg_x86_virtual_query(memory, base + 8192).state == 0x2000);
  free_range(base + 8192, 0, 0x8000);
  free_range(base, 0, 0x8000);
  allocate(0, 9 * 4096, 0x3000, 4, 0xc0000017u);
  assert(!gamepkg_x86_virtual_stats(memory, &s) && !s.committed && !s.reserved);
  for (unsigned z = 0; z <= 21; ++z) {
    zero = z;
    allocate(0, 4096, 0x3000, 4, z <= 15 ? 0 : 0xc0000017u);
    if (z <= 15)
      free_range(result.base, 0, 0x8000);
  }
  zero = 0;
  allocate(0, 0, 0x2000, 4, 0xc000000du);
  allocate(0, 4096, 0, 4, 0xc000000du);
  allocate(0, 4096, 0x3000, 3, 0xc0000045u);
  zero = 22;
  allocate(0, 4096, 0x3000, 4, 0xc000000du);
  zero = 0;
  for (unsigned i = 0; i < 2; ++i) {
    put(bank + 0x1000, 0);
    put(bank + 0x1010, 4096);
    flags = i ? 0x2000 : 0x80000;
    protection = i ? 0x104 : 4;
    action = 1;
    call(0, 0, 0);
    assert(result.status == -2);
  }
  action = 3;
  call(0, 0, 0);
  action = 1;
  flags = 0x3000;
  protection = 4;
  call(0, 0, 0);
  assert(result.status == -1);
  action = 4;
  call(0, 0, 0);
  // Ordered overlapping output pointees are staged before either write.
  a = n = 0x2000;
  put(bank + 0x1000, 0x10001);
  action = 1;
  flags = 0x2000;
  protection = 4;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && result.base == 0x10000 &&
         result.length == 0x11000 && get(bank + 0x1000) == 0x11000);
  a = 0x2000;
  n = 0x2010;
  free_range(result.base, 0, 0x8000);
  teardown();
  for (unsigned output = 0; output < 2; ++output)
    for (unsigned byte = 0; byte < 4; ++byte) {
      unsigned denied = 0x1000 + output * 16 + byte;
      setup(denied);
      put(bank + 0x1000, 0);
      put(bank + 0x1010, 4096);
      action = 1;
      flags = 0x3000;
      protection = 4;
      call(14, 3, 0x1000 + denied);
      assert(!gamepkg_x86_virtual_stats(memory, &s) && !s.reserved &&
             !s.committed && get(bank + 0x1000) == 0 &&
             get(bank + 0x1010) == 4096);
      teardown();
    }
  printf("NT virtual memory: %u checked calls/generated transitions; "
         "reserve/commit/decommit/partial release, rounding/ZeroBits/OOM, "
         "readonly output fault atomicity and pending-FP preservation passed\n",
         checks);
  return 0;
}
