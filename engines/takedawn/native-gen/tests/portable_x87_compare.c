/* Synthetic explicit-LE hardware corpus, replayed without x86 instructions. */
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
static uint64_t wide(FILE *f) {
  uint64_t lo = word(f);
  return lo | ((uint64_t)word(f) << 32);
}
static void next_wait(void) { gamepkg_x86_x87_wait(0x20000, 0x9876); }
int main(int argc, char **argv) {
  assert(argc == 2);
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && memcmp(magic, "GPKXCMP1", 8) == 0);
  uint32_t count = word(f);
  assert(count > 0 && count <= 4000000);
  uint32_t queued = 0;
  for (uint32_t row = 0; row < count; ++row) {
    uint32_t kind = word(f), source = word(f), pop = word(f), quiet = word(f),
             integer_flags = word(f), flags = word(f), low = word(f),
             high = word(f), cw = word(f), sw = word(f), tags = word(f);
    uint64_t a = wide(f);
    unsigned ae = word(f);
    uint64_t b = wide(f);
    unsigned be = word(f), expected_sw = word(f), expected_tags = word(f),
             expected_flags = word(f);
    assert(kind <= 5 && source <= 7 && pop <= 2 && quiet <= 1 &&
           integer_flags <= 1 && cw <= 65535 && sw <= 65535 && tags <= 65535 &&
           ae <= 65535 && be <= 65535 && expected_sw <= 65535 &&
           expected_tags <= 65535);
    GamePkgX87State initial = {0};
    unsigned top = (sw >> 11) & 7, src = (top + source) & 7;
    for (unsigned i = 0; i < 8; ++i) {
      initial.significand[i] = UINT64_C(0x8000000000000010) + i;
      initial.sign_exponent[i] = (uint16_t)(0x4000 + i);
    }
    initial.significand[top] = a;
    initial.sign_exponent[top] = (uint16_t)ae;
    if (kind == 0) {
      if (src == top)
        assert(a == b && ae == be);
      initial.significand[src] = b;
      initial.sign_exponent[src] = (uint16_t)be;
    }
    initial.control = (uint16_t)cw;
    initial.status = (uint16_t)sw;
    initial.tags = (uint16_t)tags;
    initial.instruction_pointer = 0x40000;
    initial.data_pointer = 0x50000;
    initial.opcode = 0x123;
    *gamepkg_x86_x87_state() = initial;
    uint32_t out_flags =
        gamepkg_x86_x87_compare(kind, source, pop, quiet, integer_flags, flags,
                                low, high, 0x10000, 0x456, 1025, 0xabcdef12);
    GamePkgX87State expected = initial;
    expected.status = (uint16_t)expected_sw;
    expected.tags = (uint16_t)expected_tags;
    expected.instruction_pointer = 0x10000;
    if (kind >= 2)
      expected.data_pointer = 1025;
    expected.opcode = 0x456;
    GamePkgX87State *actual = gamepkg_x86_x87_state();
    if (out_flags != expected_flags ||
        memcmp(actual, &expected, sizeof(expected))) {
      fprintf(stderr,
              "portable comparison row%u kind%u cw%04x sw actual%04x "
              "expected%04x flags actual%x expected%x\n",
              row, kind, cw, actual->status, expected_sw, out_flags,
              expected_flags);
      return 1;
    }
    if (expected_sw & ~cw & 63u) {
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(next_wait, &fault) == 1 && fault.vector == 16 &&
             fault.eip == 0x20000 && fault.ebp == 0x9876 &&
             memcmp(actual, &expected, sizeof(expected)) == 0);
      ++queued;
    }
  }
  assert(fgetc(f) == EOF && !ferror(f) && fclose(f) == 0);
  printf("portable x87 comparison: %u exact state/flags cases; %u deferred "
         "faults\n",
         count, queued);
  return 0;
}
