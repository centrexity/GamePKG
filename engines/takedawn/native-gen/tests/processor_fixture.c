/* Explicit field capabilities; unknown processor/thread gaps stay unbacked. */
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
static const struct {
  uint32_t address, bytes, permissions;
  unsigned char data[32];
} fields[] = {/* FIELDS */};
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
uint32_t xbox_MmioLoad(uint32_t address, int bytes) {
  uint32_t pc, bp, w[4];
  assert(!gamepkg_x86_dispatch_site(&pc, &bp));
  gamepkg_x86_memory_read(address, w, (uint32_t)bytes, pc, bp);
  return w[0];
}
/* PROGRAM */
static const GamePkgX86DispatchEntry entry[] = {
    {0x400000, gamepkg_cpu_block_00400000}};
static uint32_t probe_address;
static void read_gap(void) {
  uint32_t w[4];
  gamepkg_x86_memory_read(probe_address, w, 4, 0x3456, g_ebp);
}
static void write_gate(void) {
  uint32_t w[4] = {1, 0, 0, 0};
  gamepkg_x86_memory_write(0x2278, w, 4, 15, 0, 0x3456, g_ebp);
}
static void run(int missing) {
  unsigned char data[7][32], stack[4] = {0, 0, 0x70, 0};
  GamePkgX86Region regions[8];
  size_t count = 0;
  for (unsigned i = 0; i < 7; ++i) {
    memcpy(data[i], fields[i].data, 32);
    if ((int)i != missing)
      regions[count++] = (GamePkgX86Region){fields[i].address, fields[i].bytes,
                                            fields[i].permissions, data[i]};
  }
  regions[count++] = (GamePkgX86Region){0x11fa0, 4, 3, stack};
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(regions, count, 0);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  const GamePkgX86Tables tables = {{0, 0}, {0, 0}};
  const GamePkgX86InterruptState state = {0x202, 1, 0, 0, 0, 1};
  const GamePkgX86CpuCodeState code = {8, 0, UINT32_MAX, 0xc9b, 0, 0, 0, 0, 0};
  assert(!gamepkg_x86_cpu_init_tables(cpu, &tables));
  GamePkgX86CpuThread *thread = gamepkg_x86_cpu_thread_create(cpu, &state);
  assert(thread && !gamepkg_x86_cpu_enter(thread) &&
         !gamepkg_x86_cpu_init_code(thread, &code));
  memset(&fp, 0xa5, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(xmm, 0x5a, sizeof(xmm));
  gamepkg_x86_x87_bind_segments(8, 16, 16, 24);
  g_eax = 0x11223344;
  g_ecx = 0x22334455;
  g_edx = 0x33445566;
  g_ebx = 0x44556677;
  g_esp = 0x11fa0;
  g_ebp = 0x5678;
  g_esi = 0x55667788;
  g_edi = 0x66778899;
  g_eflags = 0x202;
  g_fs_base = missing == -2 ? 0 : 0x2000;
  g_df = 0;
  GamePkgX86FpFault before, after;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source));
  GamePkgX86DispatchExit out;
  int reason = gamepkg_x86_dispatch_run_cpu_code_events(
      entry, 1, 0x400000, &out, &source, thread, NULL);
  int success = missing == -1 || missing == 1;
  if (success) {
    assert(reason == 2 && out.pc == 0x700000 && g_esp == 0x11fa4 &&
           g_eax == 0 && g_ebx == 0x2000 && g_ecx == 0x11fc0 &&
           g_edx == 0x3000 && g_esi == 0x2050 && g_edi == 0x2000);
  } else {
    uint32_t address = missing == -2  ? 0x18
                       : missing == 0 ? 0x201c
                       : missing == 2 ? 0x2050
                       : missing == 3 ? 0x2278
                       : missing == 4 ? 0x2018
                       : missing == 5 ? 0x2028
                                      : 0x3028;
    uint32_t pc = missing == -2 || missing == 4 ? 0x400000
                  : missing == 0                ? 0x400007
                  : missing == 2                ? 0x400019
                  : missing == 3                ? 0x40001c
                  : missing == 5                ? 0x400014
                                                : 0x400016;
    assert(reason == 1 && out.pc == pc && out.fault.core.eip == pc &&
           out.fault.core.vector == 14 && !out.fault.core.error_code &&
           out.fault.core.fault_address == address && g_esp == 0x11fa0 &&
           !memcmp(&out.fault.fp, &before.fp, sizeof(before.fp)));
  }
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before.fp, &after.fp, sizeof(before.fp)) && g_ebp == 0x5678 &&
         g_eflags == 0x202 && !g_df);
  GamePkgX86CpuExecution actual;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &actual) &&
         !memcmp(&actual.code, &code, sizeof(code)) &&
         actual.interrupt.flags == 0x202 && !actual.interrupt.sti_shadow);
  GamePkgX86FpFault fault;
  assert(gamepkg_x86_run_with_fp(write_gate, &fault, &source) == 1 &&
         fault.core.vector == 14 &&
         fault.core.error_code == (missing == 3 ? 2u : 3u) &&
         fault.core.fault_address == 0x2278 &&
         !memcmp(&fault.fp, &before.fp, sizeof(before.fp)));
  for (unsigned i = 0; i < 2; ++i) {
    probe_address = i ? 0x3000 : 0x2040;
    assert(gamepkg_x86_run_with_fp(read_gap, &fault, &source) == 1 &&
           fault.core.vector == 14 && !fault.core.error_code &&
           fault.core.fault_address == probe_address &&
           !memcmp(&fault.fp, &before.fp, sizeof(before.fp)));
  }
  for (unsigned i = 0; i < 7; ++i)
    assert(!memcmp(data[i], fields[i].data, 32));
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
int main(void) {
  for (int missing = -2; missing < 7; ++missing)
    run(missing);
  printf("Processor fields:9 checked FS/PRCB/TLS traversals and27 "
         "readonly/unknown-gap gates; pending x87-MMX-XMM preserved; "
         "pointer_bits=%zu\n",
         sizeof(void *) * 8);
  return 0;
}
