/* Host-import return planning against the independent synthetic near corpus. */
#include "gamepkg_x86_cpu_code.h"
#include "gamepkg_x86_fault.h"
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
static GamePkgX86CpuThread *thread;
static GamePkgX86NearReturn plan;
static uint32_t width, cleanup;
static unsigned cases;
static unsigned char stack[4096], top[4];
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
static void put(unsigned char *p, uint32_t n) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(n >> (8 * i));
}
static void entry(void) {
  plan =
      gamepkg_x86_cpu_plan_return(thread, g_esp, width, cleanup, 0x1234, g_ebp);
}
static void invoke(unsigned vector, unsigned error, uint32_t address,
                   uint32_t target, uint32_t next, int status) {
  GamePkgX86FpFault before, after, fault;
  GamePkgX86CpuExecution cpu_before, cpu_after;
  unsigned char saved[sizeof(stack)];
  unsigned char saved_top[sizeof(top)];
  memcpy(saved, stack, sizeof(stack));
  memcpy(saved_top, top, sizeof(top));
  assert(!gamepkg_x86_capture_registers(0, 0, &before, &source));
  int owned = !gamepkg_x86_cpu_snapshot_execution(thread, &cpu_before);
  plan = (GamePkgX86NearReturn){123, 0xdeadbeef, 0xcafebabe};
  int reason = gamepkg_x86_run_with_fp(entry, &fault, &source);
  assert(reason == (vector ? 1 : 0));
  if (vector) {
    assert(plan.status == 123 && plan.offset == 0xdeadbeef &&
           plan.esp == 0xcafebabe && fault.core.vector == vector &&
           fault.core.error_code == error &&
           fault.core.fault_address == address && fault.core.eip == 0x1234 &&
           fault.core.ebp == g_ebp &&
           !memcmp(&fault.fp, &before.fp, sizeof(before.fp)));
  } else {
    assert(plan.status == status && plan.offset == target && plan.esp == next);
  }
  assert(!gamepkg_x86_capture_registers(0, 0, &after, &source) &&
         !memcmp(&before, &after, sizeof(before)) &&
         !memcmp(saved, stack, sizeof(stack)) &&
         !memcmp(saved_top, top, sizeof(top)));
  if (owned)
    assert(!gamepkg_x86_cpu_snapshot_execution(thread, &cpu_after) &&
           !memcmp(&cpu_before, &cpu_after, sizeof(cpu_before)));
  ++cases;
}
static void replay(const uint32_t r[16]) {
  assert(r[0] >= 19 && r[0] <= 22 && r[3] < 6);
  width = r[0] >= 21 ? 2 : 4;
  cleanup = r[0] == 20 || r[0] == 22 ? 8 : 0;
  g_esp = r[4];
  g_ecx = r[5];
  g_eflags = r[6];
  memset(stack, 0xa5, sizeof(stack));
  if (g_esp >= 0x11001000 && g_esp <= 0x11001ffc)
    put(stack + g_esp - 0x11001000, r[2]);
  GamePkgX86Region regions[] = {
      {0x11001000, sizeof(stack), r[3] == 1 ? 1 : 3, stack}};
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(regions, 1, 1);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  /* A native fault at127 is the later sentinel fetch, after RET completed.
   * Planning only checks the target; it must accept this boundary offset. */
  int completed = r[8] == 6 || r[10] == 127;
  invoke(completed ? 0 : r[8], completed ? 0 : r[9], completed ? 0 : r[15],
         completed ? r[10] : 0, completed ? r[11] : 0, 0);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
static uint32_t word(FILE *f) {
  uint32_t n = 0;
  for (unsigned i = 0; i < 4; ++i) {
    int c = fgetc(f);
    assert(c != EOF);
    n |= (uint32_t)c << (8 * i);
  }
  return n;
}
int main(int argc, char **argv) {
  assert(argc == 1 || argc == 2);
  memset(&fp, 0xa5, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(xmm, 0x5a, sizeof(xmm));
  g_eax = 0x11223344;
  g_ebp = 0x5678;
  g_eflags = 0x202;
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  const GamePkgX86Tables tables = {{0, 0}, {0, 0}};
  assert(!gamepkg_x86_cpu_init_tables(cpu, &tables));
  const GamePkgX86InterruptState initial = {
      .flags = 0x202, .protected_mode = 1, .cpl = 3};
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread));
  width = 4;
  cleanup = 0;
  g_esp = 1; /* Missing code state rejects before an unreadable stack. */
  invoke(0, 0, 0, 0, 0, -1);
  const GamePkgX86CpuCodeState code = {7, 0, 127, 0x4fb, 0, 0, 0, 0, 0};
  assert(!gamepkg_x86_cpu_init_code(thread, &code));
  GamePkgX86CpuExecution original, other;
  assert(!gamepkg_x86_cpu_snapshot_execution(thread, &original));
  other = original;
  other.code.cs_base = 0x10000000;
  assert(!gamepkg_x86_cpu_restore_execution(thread, &other));
  invoke(0, 0, 0, 0, 0, -1);
  other = original;
  other.code.cs_attributes &= ~0x400u;
  assert(!gamepkg_x86_cpu_restore_execution(thread, &other));
  invoke(0, 0, 0, 0, 0, -1);
  assert(!gamepkg_x86_cpu_restore_execution(thread, &original));
  width = 3;
  invoke(0, 0, 0, 0, 0, -1);
  width = 4;
  cleanup = 65536;
  invoke(0, 0, 0, 0, 0, -1);
  cleanup = 0;
  assert(!gamepkg_x86_cpu_block(thread));
  invoke(0, 0, 0, 0, 0, -1);
  assert(!gamepkg_x86_cpu_enter(thread));
  for (unsigned kind = 19; kind < 23; ++kind)
    for (unsigned dest = 0; dest < 3; ++dest)
      for (unsigned mode = 0; mode < 6; ++mode) {
        uint32_t target = dest == 0 ? 64 : dest == 1 ? 128 : 127;
        uint32_t sp = mode == 2   ? 0x11001000
                      : mode == 3 ? 1
                      : mode == 5 ? 0x11000ffc
                                  : 0x11001040;
        uint32_t bytes = kind >= 21 ? 2 : 4;
        int unreadable = mode == 3 || mode == 5;
        uint32_t r[16] = {kind,
                          0,
                          target,
                          mode,
                          sp,
                          2,
                          0x202,
                          0,
                          unreadable      ? 14
                          : target == 128 ? 13
                                          : 6,
                          unreadable ? 4 : 0,
                          target,
                          sp + bytes + ((kind == 20 || kind == 22) ? 8 : 0)};
        if (r[8] != 6)
          r[10] = 0;
        r[15] = unreadable ? sp : 0;
        replay(r);
      }
  if (argc == 2) {
    FILE *f = fopen(argv[1], "rb");
    assert(f);
    unsigned char magic[8];
    assert(fread(magic, 1, 8, f) == 8 && !memcmp(magic, "GPKNEAR1", 8));
    uint32_t count = word(f), selected = 0;
    assert(count == 5796);
    for (uint32_t i = 0; i < count; ++i) {
      uint32_t r[16];
      unsigned char raw[40];
      for (unsigned j = 0; j < 16; ++j)
        r[j] = word(f);
      assert(fread(raw, 1, sizeof(raw), f) == sizeof(raw));
      if (r[0] >= 19 && r[0] <= 22) {
        assert(raw[0] == (r[0] >= 21 ? 0x66 : r[0] == 20 ? 0xc2 : 0xc3));
        replay(r);
        ++selected;
      }
    }
    assert(selected == 1008 && fgetc(f) == EOF && !fclose(f));
  }
  GamePkgX86Region last = {0xfffffffc, 4, 1, top};
  put(top, 64);
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(&last, 1, 1);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  g_esp = 0xfffffffc;
  width = 4;
  cleanup = 65535;
  invoke(0, 0, 0, 64, 65535, 0);
  g_esp = 0xffffffff;
  invoke(12, 0, 0, 0, 0, 0);
  width = 2;
  invoke(12, 0, 0, 0, 0, 0);
  g_esp = 0xfffffffe;
  invoke(0, 0, 0, 0, 65535, 0);
  put(top, 0x12340040);
  g_esp = 0xfffffffc;
  cleanup = 8;
  invoke(0, 0, 0, 64, 6, 0); /* RET16 ignores the next two stack bytes. */
  width = 4;
  invoke(13, 0, 0, 0, 0, 0);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  printf(
      "Host return planning: %u owner/cache/native RET fault and limit checks; "
      "no stack/raw FP/CPU mutation; pointer_bits=%zu\n",
      cases, sizeof(void *) * 8);
  return 0;
}
