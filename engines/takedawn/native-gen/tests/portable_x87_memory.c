/* Explicit-LE, synthetic hardware-qualified instruction state, no x86 asm. */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_x87.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
_Thread_local uint_fast8_t gamepkg_softfloat_unbounded_inexact;
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
static void wait_next(void) { gamepkg_x86_x87_wait(0x20000, 0x9876); }
int main(int argc, char **argv) {
  assert(argc == 2);
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && memcmp(magic, "GPKXMEM1", 8) == 0);
  uint32_t count = word(f);
  assert(count > 0 && count <= 4000000);
  uint32_t queued = 0;
  for (uint32_t row = 0; row < count; ++row) {
    uint32_t op = word(f), reverse = word(f), kind = word(f), low = word(f),
             high = word(f), cw = word(f), sw = word(f), tags = word(f);
    uint64_t sig = word(f);
    sig |= (uint64_t)word(f) << 32;
    uint32_t se = word(f), expected_sw = word(f), expected_tags = word(f);
    uint64_t expected_sig = word(f);
    expected_sig |= (uint64_t)word(f) << 32;
    uint32_t expected_se = word(f);
    assert(op <= 3 && reverse <= 1 && kind <= 3 && cw <= 65535 && sw <= 65535 &&
           tags <= 65535 && se <= 65535 && expected_sw <= 65535 &&
           expected_tags <= 65535 && expected_se <= 65535);
    GamePkgX87State initial = {0};
    unsigned top = (sw >> 11) & 7;
    for (unsigned i = 0; i < 8; ++i) {
      initial.significand[i] = UINT64_C(0x8000000000000010) + i;
      initial.sign_exponent[i] = (uint16_t)(0x4000 + i);
    }
    initial.significand[top] = sig;
    initial.sign_exponent[top] = (uint16_t)se;
    initial.control = (uint16_t)cw;
    initial.status = (uint16_t)sw;
    initial.tags = (uint16_t)tags;
    initial.instruction_pointer = 0x40000;
    initial.data_pointer = 0x50000;
    initial.opcode = 0x123;
    *gamepkg_x86_x87_state() = initial;
    gamepkg_x86_x87_memory_arithmetic(op, reverse, kind, low, high, 0x10000,
                                      0x456, 1025, 0xabcdef12);
    GamePkgX87State *actual = gamepkg_x86_x87_state();
    GamePkgX87State expected = initial;
    expected.significand[top] = expected_sig;
    expected.sign_exponent[top] = (uint16_t)expected_se;
    expected.status = (uint16_t)expected_sw;
    expected.tags = (uint16_t)expected_tags;
    expected.instruction_pointer = 0x10000;
    expected.data_pointer = 1025;
    expected.opcode = 0x456;
    if (memcmp(actual, &expected, sizeof(expected))) {
      fprintf(stderr,
              "portable x87 memory row%u op%u kind%u cw%04x "
              "actual%04x:%016llx/sw%04x expected%04x:%016llx/sw%04x\n",
              row, op, kind, cw, actual->sign_exponent[top],
              (unsigned long long)actual->significand[top], actual->status,
              expected_se, (unsigned long long)expected_sig, expected_sw);
      return 1;
    }
    if (expected_sw & ~cw & 63u) {
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(wait_next, &fault) == 1 && fault.vector == 16 &&
             fault.eip == 0x20000 && fault.ebp == 0x9876 &&
             memcmp(actual, &expected, sizeof(expected)) == 0);
      ++queued;
    }
  }
  assert(fgetc(f) == EOF && !ferror(f) && fclose(f) == 0);
  printf("portable x87 memory: %u exact state cases; %u deferred faults\n",
         count, queued);
  return 0;
}
