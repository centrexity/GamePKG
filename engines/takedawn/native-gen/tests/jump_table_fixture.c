#include "gamepkg_x86_dispatch.h"
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
static unsigned char image[768], stack[64];
static GamePkgX86Memory *memory;
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
/* PROGRAM */
static void put(unsigned char *p, uint32_t n) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(n >> (8 * i));
}
static void bind(unsigned denied) {
  GamePkgX86Region r[3] = {
      {0x1000, denied, 3, image},
      {0x1001 + denied, sizeof(image) - 1 - denied, 3, image + denied + 1},
      {0x2000, 64, 3, stack}};
  memory = gamepkg_x86_memory_create(r, 3, 0);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
}
static void unbind(void) {
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}
int main(void) {
  /* INIT */
  memset(&fp, 0, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  fp.tags = 0;
  memset(fp.significand, 0xa5, sizeof(fp.significand));
  memset(xmm, 0x5a, sizeof(xmm));
  unsigned checks = 0;
  for (unsigned negative = 0; negative < 2; ++negative)
    for (unsigned index = 0; index < 4; ++index)
      for (unsigned swap = 0; swap < 2; ++swap) {
        unsigned base = negative * 256;
        put(image + base + 16 + index * 4,
            0x1000 + base + 32 + (swap ? 3 - index : index) * 6);
        bind(510);
        g_ecx = index;
        g_esp = 0x2000;
        put(stack, 0x700000);
        g_eflags = 0x202;
        GamePkgX86FpFault before, after;
        assert(!gamepkg_x86_capture_registers(0, 0, &before, &source));
        GamePkgX86DispatchExit out;
        int rc = gamepkg_x86_dispatch_run(gamepkg_x86_generated_entries,
                                          gamepkg_x86_generated_count,
                                          0x1000 + base, &out, &source);
        assert(rc == 2 && out.pc == 0x700000 && g_esp == 0x2004 &&
               g_eax == 40 + (swap ? 3 - index : index));
        assert(!gamepkg_x86_capture_registers(0, 0, &after, &source) &&
               !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
        unbind();
        ++checks;
      }
  for (unsigned negative = 0; negative < 2; ++negative)
    for (unsigned byte = 0; byte < 4; ++byte) {
      unsigned base = negative * 256;
      bind(base + 16 + byte);
      g_ecx = 0;
      g_esp = 0x2000;
      g_eax = 0xabcdef;
      g_eflags = 0x202;
      GamePkgX86FpFault before, after;
      assert(!gamepkg_x86_capture_registers(0, 0, &before, &source));
      GamePkgX86DispatchExit out;
      int rc = gamepkg_x86_dispatch_run(gamepkg_x86_generated_entries,
                                        gamepkg_x86_generated_count,
                                        0x1000 + base, &out, &source);
      assert(rc == 1 && out.fault.core.vector == 14 &&
             out.fault.core.error_code == 0 &&
             out.fault.core.fault_address == 0x1000 + base + 16 + byte &&
             g_esp == 0x2000 && g_eax == 0xabcdef);
      assert(!gamepkg_x86_capture_registers(0, 0, &after, &source) &&
             !memcmp(&before.fp, &after.fp, sizeof(before.fp)) &&
             !memcmp(&before.fp, &out.fault.fp, sizeof(before.fp)));
      unbind();
      ++checks;
    }
  for (unsigned negative = 0; negative < 2; ++negative) {
    unsigned base = negative * 256;
    put(image + base + 16, 0x666666);
    bind(510);
    g_ecx = 0;
    g_esp = 0x2000;
    g_eflags = 0x202;
    GamePkgX86DispatchExit out;
    int rc = gamepkg_x86_dispatch_run(gamepkg_x86_generated_entries,
                                      gamepkg_x86_generated_count,
                                      0x1000 + base, &out, &source);
    assert(rc == 2 && out.pc == 0x666666 && g_esp == 0x2000);
    unbind();
    ++checks;
  }
  /* An eight-way bounded jump with its table directly after all code. */
  for (unsigned index = 0; index < 8; ++index)
    for (unsigned swap = 0; swap < 2; ++swap) {
      put(image + 512 + 66 + index * 4,
          0x1200 + 12 + (swap ? 7 - index : index) * 6);
      bind(766);
      g_eax = index;
      g_esp = 0x2000;
      put(stack, 0x700000);
      g_eflags = 0x202;
      GamePkgX86FpFault before, after;
      assert(!gamepkg_x86_capture_registers(0, 0, &before, &source));
      GamePkgX86DispatchExit out;
      int rc = gamepkg_x86_dispatch_run(gamepkg_x86_generated_entries,
                                        gamepkg_x86_generated_count, 0x1200,
                                        &out, &source);
      assert(rc == 2 && out.pc == 0x700000 && g_esp == 0x2004 &&
             g_eax == 100 + (swap ? 7 - index : index));
      assert(!gamepkg_x86_capture_registers(0, 0, &after, &source) &&
             !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
      unbind();
      ++checks;
    }
  const uint32_t invalid_index[] = {8, 9, 0x80000000u, UINT32_MAX};
  for (unsigned i = 0; i < sizeof(invalid_index) / sizeof(*invalid_index);
       ++i) {
    bind(512 + 66);
    g_eax = invalid_index[i];
    g_esp = 0x2000;
    put(stack, 0x700000);
    g_eflags = 0x202;
    GamePkgX86DispatchExit out;
    int rc = gamepkg_x86_dispatch_run(gamepkg_x86_generated_entries,
                                      gamepkg_x86_generated_count, 0x1200, &out,
                                      &source);
    assert(rc == 2 && out.pc == 0x700000 && g_eax == 0xdead && g_esp == 0x2004);
    unbind();
    ++checks;
  }
  for (unsigned index = 0; index < 8; ++index)
    for (unsigned byte = 0; byte < 4; ++byte) {
      bind(512 + 66 + index * 4 + byte);
      g_eax = index;
      g_esp = 0x2000;
      g_eflags = 0x202;
      GamePkgX86FpFault before, after;
      assert(!gamepkg_x86_capture_registers(0, 0, &before, &source));
      GamePkgX86DispatchExit out;
      int rc = gamepkg_x86_dispatch_run(gamepkg_x86_generated_entries,
                                        gamepkg_x86_generated_count, 0x1200,
                                        &out, &source);
      assert(rc == 1 && out.fault.core.vector == 14 &&
             out.fault.core.fault_address == 0x1200 + 66 + index * 4 + byte &&
             g_esp == 0x2000 && g_eax == index);
      assert(!gamepkg_x86_capture_registers(0, 0, &after, &source) &&
             !memcmp(&before.fp, &after.fp, sizeof(before.fp)) &&
             !memcmp(&before.fp, &out.fault.fp, sizeof(before.fp)));
      unbind();
      ++checks;
    }
  put(image + 512 + 66, 0x666666);
  bind(766);
  g_eax = 0;
  g_esp = 0x2000;
  g_eflags = 0x202;
  GamePkgX86DispatchExit out;
  int rc = gamepkg_x86_dispatch_run(gamepkg_x86_generated_entries,
                                    gamepkg_x86_generated_count, 0x1200, &out,
                                    &source);
  assert(rc == 2 && out.pc == 0x666666 && g_esp == 0x2000);
  unbind();
  ++checks;
  printf("Jump tables: %u live-table, wrapped-index, missing-target and fault "
         "preservation cases passed\n",
         checks);
  return 0;
}
