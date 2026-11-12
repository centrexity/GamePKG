/* Own synthetic virtual kernel, executable INIT and real checked RAM pages. */
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
static const unsigned char image[0x6000] = {/* IMAGE */};
static unsigned char data[0x1000], stack[16];
static unsigned stores, cases;
static uint32_t protected_address;
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
uint32_t xbox_MmioLoad(uint32_t address, int bytes) {
  uint32_t pc, bp, words[4];
  assert(!gamepkg_x86_dispatch_site(&pc, &bp));
  gamepkg_x86_memory_read(address, words, (uint32_t)bytes, pc, bp);
  return words[0];
}
void xbox_MmioStore(uint32_t address, uint32_t value, int bytes) {
  uint32_t pc, bp, words[4] = {value, 0, 0, 0};
  assert(!gamepkg_x86_dispatch_site(&pc, &bp));
  gamepkg_x86_memory_write(address, words, (uint32_t)bytes, (1u << bytes) - 1u,
                           0, pc, bp);
  ++stores;
}
/* PROGRAM */
static const GamePkgX86DispatchEntry entries[] = {
    {0x80015000, gamepkg_cpu_block_80015000},
    {0x8001500a, gamepkg_cpu_block_80015000},
    {0x80015014, gamepkg_cpu_block_80015000},
    {0x8001501e, gamepkg_cpu_block_80015000},
    {0x80015028, gamepkg_cpu_block_80015000}};
static void protect_write(void) {
  uint32_t w[4] = {0, 0, 0, 0};
  gamepkg_x86_memory_write(protected_address, w, 4, 15, 0, 0x3456, g_ebp);
}
static void run(unsigned mode, unsigned narrow) {
  memset(data, 0xa5, sizeof(data));
  memset(stack, 0, sizeof(stack));
  stack[2] = 0x70; /* LE return PC00700000. */
  GamePkgX86Region regions[] = {
      {0x80010000, 0x1000, 1, (unsigned char *)image},
      {0x80011000, 0x1000, 1, (unsigned char *)image + 0x1000},
      {0x80012000, 0x2000, 1, (unsigned char *)image + 0x2000},
      {0x80014000, mode == 2 ? 8 : 0x1000, mode == 1 ? 1 : 3, data},
      {0x80015000, 0x1000, 1, (unsigned char *)image + 0x5000},
      {0x10000, sizeof(stack), 3, stack},
      {0x80014008, 0xff8, 1, data + 8}};
  GamePkgX86Memory *memory =
      gamepkg_x86_memory_create(regions, mode == 2 ? 7 : 6, 0);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  const GamePkgX86Tables tables = {{0x80014000, 15}, {0, 0}};
  const GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 1};
  const GamePkgX86CpuCodeState code = {
      8, 0, narrow ? 0x80015fff : UINT32_MAX, 0xc9b, 0, 0, 0, 0, 0};
  assert(!gamepkg_x86_cpu_init_tables(cpu, &tables));
  GamePkgX86CpuThread *thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread) &&
         !gamepkg_x86_cpu_init_code(thread, &code));
  g_eax = 0x11223344;
  g_ecx = 0x22334455;
  g_edx = 0x33445566;
  g_ebx = 0x44556677;
  g_esi = 0x55667788;
  g_edi = 0x66778899;
  g_esp = 0x10000;
  g_ebp = 0x5678;
  g_eflags = 0x202;
  g_df = 0;
  g_fs_base = 0;
  memset(&fp, 0xa5, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(xmm, 0x5a, sizeof(xmm));
  gamepkg_x86_x87_bind_segments(8, 16, 16, 24);
  GamePkgX86FpFault before, after;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source));
  stores = 0;
  GamePkgX86DispatchExit out;
  int reason = gamepkg_x86_dispatch_run_cpu_code_events(
      entries, 5, 0x80015000, &out, &source, thread, NULL);
  if (!mode) {
    assert(reason == 2 && out.pc == 0x700000 && !out.fault.core.vector &&
           stores == 4 && g_esp == 0x10004);
    const unsigned char expected[16] = {0,   0,   0, 0, 0, 0,    0,    0,
                                        255, 255, 0, 0, 0, 0x9b, 0xcf, 0};
    assert(!memcmp(data, expected, 16));
    for (unsigned i = 16; i < sizeof(data); ++i)
      assert(data[i] == 0xa5);
  } else {
    assert(reason == 1 && out.fault.core.vector == 14 &&
           out.fault.core.error_code == 3 &&
           out.fault.core.fault_address ==
               (mode == 1 ? 0x80014000 : 0x80014008) &&
           out.pc == (mode == 1 ? 0x80015000 : 0x80015014) &&
           stores == (mode == 1 ? 0 : 2) && g_esp == 0x10000 &&
           !memcmp(&out.fault.fp, &before.fp, sizeof(before.fp)));
    for (unsigned i = 0; i < sizeof(data); ++i)
      assert(data[i] == (mode == 2 && i < 8 ? 0 : 0xa5));
  }
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &source));
  before.core.esp = after.core.esp;
  assert(!memcmp(&before, &after, sizeof(before)));
  GamePkgX86CpuExecution state;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &state) &&
         !memcmp(&state.code, &code, sizeof(code)) &&
         !memcmp(&state.tables, &tables, sizeof(tables)) &&
         state.interrupt.flags == 0x202 && !state.interrupt.sti_shadow);
  for (unsigned i = 0; i < 4; ++i) {
    protected_address = i == 0   ? 0x80010000
                        : i == 1 ? 0x80011000
                        : i == 2 ? 0x80012f00
                                 : 0x80015000;
    GamePkgX86FpFault fault;
    assert(gamepkg_x86_run_with_fp(protect_write, &fault, &source) == 1 &&
           fault.core.vector == 14 && fault.core.error_code == 3 &&
           fault.core.fault_address == protected_address &&
           fault.core.eip == 0x3456 &&
           !memcmp(&fault.fp, &before.fp, sizeof(before.fp)));
    ++cases;
  }
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
  ++cases;
}
int main(void) {
  assert(!memcmp(image, "MZ", 2) && !memcmp(image + 0x80, "PE\0\0", 4) &&
         image[0x86] == 4 && !memcmp(image + 0x178 + 120, "INIT\0\0\0\0", 8));
  for (unsigned m = 0; m < 3; ++m)
    for (unsigned n = 0; n < 2; ++n)
      run(m, n);
  printf("Virtual kernel: %u backed INIT/cache/fault/section-protection cases; "
         "pending x87-MMX-XMM preserved; pointer_bits=%zu\n",
         cases, sizeof(void *) * 8);
  return 0;
}
