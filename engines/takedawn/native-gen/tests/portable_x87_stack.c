/* Explicit-LE hardware-qualified state replay, without x86 instructions. */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_x87.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Expected a big-endian target
#endif
_Static_assert(sizeof(void *) == 4, "expected actual32bit pointers");
#endif
static uint32_t word(FILE *f) {
  unsigned char b[4];
  assert(fread(b, 1, 4, f) == 4);
  uint32_t v = 0;
  for (unsigned i = 0; i < 4; ++i)
    v |= (uint32_t)b[i] << (8 * i);
  return v;
}
static GamePkgX87State state(FILE *f) {
  GamePkgX87State s = {0};
  for (unsigned i = 0; i < 8; ++i) {
    uint64_t low = word(f), high = word(f);
    unsigned se = word(f);
    assert(se <= 65535);
    s.significand[i] = low | (high << 32);
    s.sign_exponent[i] = (uint16_t)se;
  }
  unsigned cw = word(f), sw = word(f), tags = word(f);
  assert(cw <= 65535 && sw <= 65535 && tags <= 65535);
  s.control = (uint16_t)cw;
  s.status = (uint16_t)sw;
  s.tags = (uint16_t)tags;
  s.instruction_pointer = word(f);
  s.data_pointer = word(f);
  unsigned opcode = word(f);
  assert(opcode <= 0x7ff);
  s.opcode = (uint16_t)opcode;
  return s;
}
static void next_wait(void) { gamepkg_x86_x87_wait(0x20000, 0x9876); }
int main(int argc, char **argv) {
  assert(argc == 2);
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && memcmp(magic, "GPKXSTK1", 8) == 0);
  unsigned count = word(f), queued = 0;
  assert(count > 0 && count <= 4000000);
  for (unsigned row = 0; row < count; ++row) {
    unsigned op = word(f), index = word(f), flags = word(f);
    assert(op <= 17 && index <= 7);
    GamePkgX87State initial = state(f), expected = state(f);
    assert(initial.instruction_pointer == 0x40000 &&
           initial.data_pointer == 0x50000 && initial.opcode == 0x123 &&
           expected.instruction_pointer == 0x10000 &&
           expected.data_pointer == initial.data_pointer &&
           expected.control == initial.control &&
           !(initial.status & ~initial.control & 63u));
    *gamepkg_x86_x87_state() = initial;
    g_eax = 0x1234;
    g_ecx = 0x2345;
    g_edx = 0x3456;
    g_ebx = 0x4567;
    g_esp = 0x800;
    g_ebp = 0x777;
    g_esi = 0x5678;
    g_edi = 0x6789;
    g_eflags = 0xdeadbafe;
    g_fs_base = 0x789a;
    g_df = 1;
    gamepkg_x86_x87_stack(op, index, flags, 0x10000, expected.opcode,
                          0xabcdef12);
    GamePkgX87State *actual = gamepkg_x86_x87_state();
    if (memcmp(actual, &expected, sizeof(expected))) {
      fprintf(stderr,
              "portable stack row%u op%u index%u cw%04x flags%x status "
              "actual%04x expected%04x tags actual%04x expected%04x\n",
              row, op, index, initial.control, flags, actual->status,
              expected.status, actual->tags, expected.tags);
      return 1;
    }
    assert(g_eax == 0x1234 && g_ecx == 0x2345 && g_edx == 0x3456 &&
           g_ebx == 0x4567 && g_esp == 0x800 && g_ebp == 0x777 &&
           g_esi == 0x5678 && g_edi == 0x6789 && g_eflags == 0xdeadbafe &&
           g_fs_base == 0x789a && g_df == 1);
    if (expected.status & ~expected.control & 63u) {
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(next_wait, &fault) == 1 && fault.vector == 16 &&
             fault.eip == 0x20000 && fault.ebp == 0x9876 &&
             memcmp(actual, &expected, sizeof(expected)) == 0 &&
             g_eflags == 0xdeadbafe);
      ++queued;
    }
  }
  assert(fgetc(f) == EOF && !ferror(f) && fclose(f) == 0);
  printf("portable x87 stack: %u exact full-state cases; %u deferred faults\n",
         count, queued);
  return 0;
}
