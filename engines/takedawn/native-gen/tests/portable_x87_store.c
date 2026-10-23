/* Replay explicit-LE, synthetic hardware-qualified stores without x86 asm. */
#include "gamepkg_x86_ext80.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
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
static uint32_t kind, destination, pop;
static void store(void) {
  gamepkg_x86_x87_store(kind, destination, pop, 0x10000, 0x456, 1025,
                        0xabcdef12);
}
static void next_wait(void) { gamepkg_x86_x87_wait(0x20000, 0x9876); }
static uint32_t word(FILE *f) {
  unsigned char b[4];
  assert(fread(b, 1, 4, f) == 4);
  uint32_t v = 0;
  for (unsigned i = 0; i < 4; ++i)
    v |= (uint32_t)b[i] << (8 * i);
  return v;
}
static GamePkgExt80 value(FILE *f) {
  uint64_t lo = word(f), hi = word(f);
  unsigned se = word(f);
  assert(se <= 65535);
  return (GamePkgExt80){lo | (hi << 32), (uint16_t)se};
}
static void install(GamePkgX87State *s, unsigned p, GamePkgExt80 v) {
  s->significand[p] = v.significand;
  s->sign_exponent[p] = v.sign_exponent;
}
int main(int argc, char **argv) {
  assert(argc == 2);
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && memcmp(magic, "GPKXSTO1", 8) == 0);
  unsigned count = word(f);
  assert(count > 0 && count <= 4000000);
  unsigned queued = 0;
  unsigned char memory[16];
  GamePkgX86Region region = {1025, 16, 3, memory};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&region, 1, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  for (unsigned row = 0; row < count; ++row) {
    kind = word(f);
    destination = word(f);
    pop = word(f);
    unsigned cw = word(f), sw = word(f), tags = word(f);
    GamePkgExt80 before_source = value(f), before_destination = value(f),
                 after_source = value(f), after_destination = value(f);
    unsigned expected_sw = word(f), expected_tags = word(f);
    unsigned char expected_memory[16];
    for (unsigned i = 0; i < 4; ++i) {
      uint32_t v = word(f);
      for (unsigned j = 0; j < 4; ++j)
        expected_memory[i * 4 + j] = (unsigned char)(v >> (8 * j));
    }
    assert(kind <= 6 && destination <= 7 && pop <= 1 && cw <= 65535 &&
           sw <= 65535 && tags <= 65535 && expected_sw <= 65535 &&
           expected_tags <= 65535);
    unsigned top = (sw >> 11) & 7, dst = (top + destination) & 7;
    GamePkgX87State initial = {0};
    for (unsigned i = 0; i < 8; ++i) {
      initial.significand[i] = UINT64_C(0x8000000000000010) + i;
      initial.sign_exponent[i] = (uint16_t)(0x4000 + i);
    }
    if (top == dst)
      assert(before_source.significand == before_destination.significand &&
             before_source.sign_exponent == before_destination.sign_exponent &&
             after_source.significand == after_destination.significand &&
             after_source.sign_exponent == after_destination.sign_exponent);
    install(&initial, top, before_source);
    install(&initial, dst, before_destination);
    initial.control = (uint16_t)cw;
    initial.status = (uint16_t)sw;
    initial.tags = (uint16_t)tags;
    initial.instruction_pointer = 0x40000;
    initial.data_pointer = 0x50000;
    initial.opcode = 0x123;
    *gamepkg_x86_x87_state() = initial;
    for (unsigned i = 0; i < 16; ++i)
      memory[i] = (unsigned char)(0xa5 ^ i);
    GamePkgX86Fault fault;
    assert(gamepkg_x86_run(store, &fault) == 0);
    GamePkgX87State expected = initial;
    install(&expected, top, after_source);
    install(&expected, dst, after_destination);
    expected.status = (uint16_t)expected_sw;
    expected.tags = (uint16_t)expected_tags;
    expected.instruction_pointer = 0x10000;
    if (kind != 6)
      expected.data_pointer = 1025;
    expected.opcode = 0x456;
    GamePkgX87State *actual = gamepkg_x86_x87_state();
    if (memcmp(actual, &expected, sizeof(expected)) ||
        memcmp(memory, expected_memory, 16)) {
      fprintf(
          stderr,
          "portable store row%u kind%u cw%04x status actual%04x expected%04x\n",
          row, kind, cw, actual->status, expected_sw);
      return 1;
    }
    if (expected_sw & ~cw & 63u) {
      assert(gamepkg_x86_run(next_wait, &fault) == 1 && fault.vector == 16 &&
             fault.eip == 0x20000 && fault.ebp == 0x9876 &&
             memcmp(actual, &expected, sizeof(expected)) == 0);
      ++queued;
    }
  }
  gamepkg_x86_memory_bind(0);
  gamepkg_x86_memory_release(m);
  assert(fgetc(f) == EOF && !ferror(f) && fclose(f) == 0);
  printf(
      "portable x87 stores: %u exact state/memory cases; %u deferred faults\n",
      count, queued);
  return 0;
}
