/* Explicit little-endian reference replay without compiler floating types. */
#include "gamepkg_x86_f128.h"
#include "softfloat/platform.h"
#include "softfloat/source/8086-SSE/specialize.h"
#include "softfloat/source/include/primitiveTypes.h"
#include "softfloat/source/include/softfloat.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
_Thread_local uint_fast8_t gamepkg_softfloat_unbounded_inexact;
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Expected big endian
#endif
_Static_assert(sizeof(void *) == 4, "expected actual32bit pointers");
#endif
static unsigned word(FILE *f) {
  unsigned char b[4];
  assert(fread(b, 1, 4, f) == 4);
  unsigned v = 0;
  for (unsigned i = 0; i < 4; ++i)
    v |= (unsigned)b[i] << (8 * i);
  return v;
}
static GamePkgF128 number(FILE *f) {
  GamePkgF128 v;
  for (unsigned i = 0; i < 4; ++i)
    v.words[i] = word(f);
  return v;
}

/* Common-NaN payload operations stay within their object and cannot depend
 * on adjacent memory. This catches the original little-endian address rule. */
static void nan_guards(void) {
  struct Guarded {
    struct commonNaN n;
    uint32_t after[4];
  } out;
  memset(&out, 0xa5, sizeof(out));
  uint32_t input[4], back[4], other[4];
  input[indexWord(4, 3)] = 0x7fff9234;
  input[indexWord(4, 2)] = 0x56789abc;
  input[indexWord(4, 1)] = 0xdef01234;
  input[indexWord(4, 0)] = 0x56789abc;
  uint_fast8_t old = softfloat_exceptionFlags;
  softfloat_f128MToCommonNaN(input, &out.n);
  for (unsigned i = 0; i < 4; ++i)
    assert(out.after[i] == 0xa5a5a5a5u);
  softfloat_commonNaNToF128M(&out.n, back);
  assert(!memcmp(input, back, 16));
  for (unsigned i = 0; i < 4; ++i)
    out.after[i] = 0x5a5a5a5a;
  softfloat_commonNaNToF128M(&out.n, other);
  assert(!memcmp(back, other, 16));
  softfloat_exceptionFlags = old;
}
int main(int argc, char **argv) {
  nan_guards();
  assert(argc == 2);
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && !memcmp(magic, "GPKF1281", 8));
  unsigned count = word(f);
  assert(count > 0 && count <= 2000000);
  for (unsigned row = 0; row < count; ++row) {
    unsigned op = word(f), rc = word(f);
    assert(op <= 6 && rc <= 3);
    GamePkgF128 a = number(f), b = number(f), expected = number(f);
    unsigned flags = word(f);
    assert(!(flags & ~61u));
    softfloat_roundingMode = softfloat_round_odd;
    softfloat_detectTininess = softfloat_tininess_beforeRounding;
    softfloat_exceptionFlags = 31;
    extF80_roundingPrecision = 32;
    gamepkg_softfloat_unbounded_inexact = 0x55;
    GamePkgF128Result result;
    if (op < 4)
      assert(!gamepkg_x86_f128_calculate(a, b, op, rc, &result));
    else if (op == 4) {
      assert(a.words[2] <= 65535 && !a.words[3]);
      assert(!gamepkg_x86_f128_from_ext80(
          (GamePkgExt80){(uint64_t)a.words[0] | ((uint64_t)a.words[1] << 32),
                         (uint16_t)a.words[2]},
          &result));
    } else if (op == 5) {
      result.value = gamepkg_x86_f128_from_u32(a.words[0]);
      result.exceptions = 0;
    } else {
      GamePkgF128To80 out;
      assert(!gamepkg_x86_f128_to_ext80(a, rc, &out));
      result = (GamePkgF128Result){{{(uint32_t)out.value.significand,
                                     (uint32_t)(out.value.significand >> 32),
                                     out.value.sign_exponent, 0}},
                                   out.exceptions};
    }
    if (memcmp(&result.value, &expected, 16) || result.exceptions != flags) {
      fprintf(stderr, "wide replay row%u op%u rc%u flags%x versus%x\n", row, op,
              rc, result.exceptions, flags);
      return 1;
    }
    assert(softfloat_roundingMode == softfloat_round_odd &&
           softfloat_detectTininess == softfloat_tininess_beforeRounding &&
           softfloat_exceptionFlags == 31 && extF80_roundingPrecision == 32 &&
           gamepkg_softfloat_unbounded_inexact == 0x55);
  }
  assert(fgetc(f) == EOF && !ferror(f) && !fclose(f));
  printf("Portable wide arithmetic: %u independent reference records passed\n",
         count);
  return 0;
}
