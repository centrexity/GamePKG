/* Replay hardware numerical/save images without native x86 instructions. */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_x87.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Expected big endian
#endif
_Static_assert(sizeof(void *) == 4, "expected actual32bit pointers");
#endif
static unsigned rd(const unsigned char *p) {
  return p[0] | ((unsigned)p[1] << 8);
}
static unsigned word(FILE *f) {
  unsigned char p[4];
  assert(fread(p, 1, 4, f) == 4);
  unsigned v = 0;
  for (unsigned i = 0; i < 4; ++i)
    v |= (unsigned)p[i] << (8 * i);
  return v;
}
int main(int argc, char **argv) {
  assert(argc == 2);
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && !memcmp(magic, "GPKXENV1", 8));
  unsigned count = word(f);
  assert(count > 0 && count <= 4000000);
  unsigned char ram[512];
  GamePkgX86Region region = {1024, sizeof(ram), 3, ram};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&region, 1, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  for (unsigned row = 0; row < count; ++row) {
    unsigned form = word(f), op = form % 5, small = form / 5,
             env = small ? 14 : 28;
    assert(form < 10);
    unsigned char initial[108], input[108], output[108], state[108];
    assert(fread(initial, 1, 108, f) == 108 && fread(input, 1, 108, f) == 108 &&
           fread(output, 1, 108, f) == 108 && fread(state, 1, 108, f) == 108);
    /* Decode the input fixture independently of the service being tested. */
    GamePkgX87State s = {0};
    s.control = (unsigned short)rd(initial);
    s.status = (unsigned short)rd(initial + 4);
    s.tags = (unsigned short)rd(initial + 8);
    unsigned top = (s.status >> 11) & 7;
    for (unsigned i = 0; i < 8; ++i) {
      unsigned n = (top + i) & 7;
      for (unsigned j = 0; j < 8; ++j)
        s.significand[n] |= (uint64_t)initial[28 + 10 * i + j] << (8 * j);
      s.sign_exponent[n] = (unsigned short)rd(initial + 36 + 10 * i);
    }
    *gamepkg_x86_x87_state() = s;
    memcpy(ram, input, 108);
    gamepkg_x86_x87_environment(op, small, 1024, 0x10000, 0xabcdef12);
    if (op >= 3) {
      assert(!memcmp(ram, output, small ? 6 : 12));
      if (op == 4)
        assert(!memcmp(ram + env, output + env, 80));
    } else
      assert(!memcmp(ram, input, 108));
    unsigned size = op == 0 ? 2 : env + ((op == 2 || op == 4) ? 80 : 0);
    assert(!memcmp(ram + size, input + size, 108 - size));
    gamepkg_x86_x87_environment(4, 0, 1280, 0x20000, 0x1234);
    assert(!memcmp(ram + 256, state, 12) && !memcmp(ram + 284, state + 28, 80));
  }
  assert(fgetc(f) == EOF && !ferror(f));
  assert(!fclose(f));
  gamepkg_x86_memory_bind(0);
  gamepkg_x86_memory_release(m);
  printf("Portable x87 environment: %u hardware records passed\n", count);
  return 0;
}
