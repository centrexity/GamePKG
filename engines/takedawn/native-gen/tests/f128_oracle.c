/* Independent maintainer-only GCC binary128/libgcc arithmetic oracle.
 * Compiler floating types occur only in this fixture, never in the runtime. */
#include "gamepkg_x86_ext80.h"
#include "gamepkg_x86_f128.h"
#include "softfloat/platform.h"
#include "softfloat/source/8086-SSE/specialize.h"
#include "softfloat/source/include/primitiveTypes.h"
#include "softfloat/source/include/softfloat.h"
#include <assert.h>
#include <fenv.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
_Thread_local uint_fast8_t gamepkg_softfloat_unbounded_inexact;
typedef struct {
  GamePkgF128 value;
  unsigned flags;
} Expected;
static _Thread_local unsigned checks;
static FILE *corpus;
static unsigned rows;
static void word(uint32_t v) {
  unsigned char b[4];
  for (unsigned i = 0; i < 4; ++i)
    b[i] = (unsigned char)(v >> (8 * i));
  assert(fwrite(b, 1, 4, corpus) == 4);
}
static void record(unsigned op, unsigned rc, GamePkgF128 a, GamePkgF128 b,
                   GamePkgF128 out, unsigned raised) {
  if (!corpus)
    return;
  word(op);
  word(rc);
  for (unsigned i = 0; i < 4; ++i)
    word(a.words[i]);
  for (unsigned i = 0; i < 4; ++i)
    word(b.words[i]);
  for (unsigned i = 0; i < 4; ++i)
    word(out.words[i]);
  word(raised);
  ++rows;
}
static void sentinel(void) {
  softfloat_roundingMode = softfloat_round_odd;
  softfloat_detectTininess = softfloat_tininess_beforeRounding;
  softfloat_exceptionFlags = 31;
  extF80_roundingPrecision = 32;
  gamepkg_softfloat_unbounded_inexact = 0x55;
}
static void intact(void) {
  assert(softfloat_roundingMode == softfloat_round_odd &&
         softfloat_detectTininess == softfloat_tininess_beforeRounding &&
         softfloat_exceptionFlags == 31 && extF80_roundingPrecision == 32 &&
         gamepkg_softfloat_unbounded_inexact == 0x55);
}
static GamePkgF128 bits(__float128 x) {
  GamePkgF128 a;
  memcpy(&a, &x, 16);
  return a;
}
static __float128 value(GamePkgF128 a) {
  __float128 x;
  memcpy(&x, &a, 16);
  return x;
}
static unsigned flags(void) {
  unsigned f = (unsigned)fetestexcept(FE_ALL_EXCEPT);
  return ((f & FE_INVALID) ? 1u : 0u) | ((f & FE_DIVBYZERO) ? 4u : 0u) |
         ((f & FE_OVERFLOW) ? 8u : 0u) | ((f & FE_UNDERFLOW) ? 16u : 0u) |
         ((f & FE_INEXACT) ? 32u : 0u);
}
static uint64_t random64(uint64_t *v) {
  *v ^= *v << 13;
  *v ^= *v >> 7;
  *v ^= *v << 17;
  return *v;
}
__attribute__((noinline)) static __float128
reference(__float128 a, __float128 b, unsigned op) {
  switch (op) {
  case 0:
    return a + b;
  case 1:
    return a - b;
  case 2:
    return a * b;
  default:
    return a / b;
  }
}
static void one(GamePkgF128 a, GamePkgF128 b, unsigned op, unsigned rc) {
  static const int modes[] = {FE_TONEAREST, FE_DOWNWARD, FE_UPWARD,
                              FE_TOWARDZERO};
  int saved = fegetround();
  assert(!fesetround(modes[rc]) && !feclearexcept(FE_ALL_EXCEPT));
  volatile __float128 x = value(a), y = value(b);
  __float128 z = reference(x, y, op);
  Expected e = {bits(z), flags()};
  unsigned cpu = flags();
  softfloat_roundingMode = softfloat_round_odd;
  softfloat_detectTininess = softfloat_tininess_beforeRounding;
  softfloat_exceptionFlags = 31;
  extF80_roundingPrecision = 32;
  gamepkg_softfloat_unbounded_inexact = 0x55;
  GamePkgF128Result r;
  assert(!gamepkg_x86_f128_calculate(a, b, op, rc, &r));
  if (memcmp(&r.value, &e.value, 16) || r.exceptions != e.flags) {
    fprintf(stderr,
            "wide case%u op%u rc%u a%08x:%08x:%08x:%08x b%08x:%08x:%08x:%08x "
            "got%08x:%08x:%08x:%08x/%x expected%08x:%08x:%08x:%08x/%x\n",
            checks, op, rc, a.words[3], a.words[2], a.words[1], a.words[0],
            b.words[3], b.words[2], b.words[1], b.words[0], r.value.words[3],
            r.value.words[2], r.value.words[1], r.value.words[0], r.exceptions,
            e.value.words[3], e.value.words[2], e.value.words[1],
            e.value.words[0], e.flags);
    abort();
  }
  assert(fegetround() == modes[rc] && flags() == cpu);
  assert(softfloat_roundingMode == softfloat_round_odd &&
         softfloat_detectTininess == softfloat_tininess_beforeRounding &&
         softfloat_exceptionFlags == 31 && extF80_roundingPrecision == 32 &&
         gamepkg_softfloat_unbounded_inexact == 0x55);
  assert(!fesetround(saved));
  if (corpus) {
    word(op);
    word(rc);
    for (unsigned i = 0; i < 4; ++i)
      word(a.words[i]);
    for (unsigned i = 0; i < 4; ++i)
      word(b.words[i]);
    for (unsigned i = 0; i < 4; ++i)
      word(e.value.words[i]);
    word(e.flags);
    ++rows;
  }
  ++checks;
}

__attribute__((noinline)) static __float128 extend(long double x) {
  return (__float128)x;
}
__attribute__((noinline)) static long double narrow(__float128 x) {
  return (long double)x;
}
static void from80(GamePkgExt80 input) {
  unsigned char bytes[sizeof(long double)];
  memset(bytes, 0, sizeof(bytes));
  for (unsigned i = 0; i < 8; ++i)
    bytes[i] = (unsigned char)(input.significand >> (8 * i));
  /* C long-double conversion helpers need a canonical representation. The
   * x87 pseudo-denormal value equals the same significand at exponent1. */
  unsigned canonical = input.sign_exponent;
  if (!(canonical & 0x7fff) && (input.significand >> 63))
    canonical |= 1;
  bytes[8] = (unsigned char)canonical;
  bytes[9] = (unsigned char)(canonical >> 8);
  long double x;
  memcpy(&x, bytes, sizeof(x));
  assert(!feclearexcept(FE_ALL_EXCEPT));
  GamePkgF128 expected = bits(extend(x));
  unsigned f = flags();
  sentinel();
  GamePkgF128Result r;
  assert(!gamepkg_x86_f128_from_ext80(input, &r));
  if (memcmp(&r.value, &expected, 16) || r.exceptions != f) {
    fprintf(stderr, "extend%04x:%016llx flags got%x expected%x\n",
            input.sign_exponent, (unsigned long long)input.significand,
            r.exceptions, f);
    abort();
  }
  intact();
  assert(flags() == f);
  GamePkgF128 a = {{0}};
  a.words[0] = (uint32_t)input.significand;
  a.words[1] = (uint32_t)(input.significand >> 32);
  a.words[2] = input.sign_exponent;
  record(4, 0, a, (GamePkgF128){{0}}, expected, f);
  ++checks;
}
static void integer(unsigned input) {
  sentinel();
  GamePkgF128 r = gamepkg_x86_f128_from_u32(input),
              expected = bits((__float128)input);
  assert(!memcmp(&r, &expected, 16));
  intact();
  record(5, 0, (GamePkgF128){{input, 0, 0, 0}}, (GamePkgF128){{0}}, expected,
         0);
  ++checks;
}
static void to80(GamePkgF128 input, unsigned rc) {
  static const int modes[] = {FE_TONEAREST, FE_DOWNWARD, FE_UPWARD,
                              FE_TOWARDZERO};
  int saved = fegetround();
  assert(!fesetround(modes[rc]) && !feclearexcept(FE_ALL_EXCEPT));
  long double expected = narrow(value(input));
  unsigned f = flags();
  unsigned char bytes[sizeof(expected)];
  memcpy(bytes, &expected, sizeof(expected));
  uint64_t sig = 0;
  for (unsigned i = 0; i < 8; ++i)
    sig |= (uint64_t)bytes[i] << (8 * i);
  unsigned se = bytes[8] | ((unsigned)bytes[9] << 8);
  sentinel();
  GamePkgF128To80 r;
  assert(!gamepkg_x86_f128_to_ext80(input, rc, &r));
  if (r.value.significand != sig || r.value.sign_exponent != se ||
      r.exceptions != f) {
    fprintf(stderr,
            "narrow rc%u src%x:%x:%x:%x got%04x:%016llx/%x "
            "expected%04x:%016llx/%x\n",
            rc, input.words[3], input.words[2], input.words[1], input.words[0],
            r.value.sign_exponent, (unsigned long long)r.value.significand,
            r.exceptions, se, (unsigned long long)sig, f);
    abort();
  }
  intact();
  /* Independent compiler reference also qualifies the raw x87 final rounder.
   * PC must not reduce transcendental result precision. */
  for (unsigned pc = 0; pc < 4; ++pc) {
    GamePkgExt80Result raw;
    assert(!gamepkg_x86_ext80_round_f128(input.words, 0,
                                         0x7f | (pc << 8) | (rc << 10), &raw));
    assert(raw.value.significand == sig && raw.value.sign_exponent == se &&
           raw.exceptions == f);
    intact();
  }
  assert(flags() == f && fegetround() == modes[rc]);
  assert(!fesetround(saved));
  record(6, rc, input, (GamePkgF128){{0}},
         (GamePkgF128){{(uint32_t)sig, (uint32_t)(sig >> 32), se, 0}}, f);
  ++checks;
}
static const GamePkgF128 edges[] = {
    {{0, 0, 0, 0}},
    {{0, 0, 0, 0x80000000}},
    {{0, 0, 0, 0x3fff0000}},
    {{1, 0, 0, 0x3fff0000}},
    {{0xffffffff, 0xffffffff, 0xffffffff, 0x3ffeffff}},
    {{0, 0, 0, 0xbfff0000}},
    {{0, 0, 0, 0x40000000}},
    {{0, 0, 0, 0x3ffe0000}},
    {{0, 0, 0, 0x3fff8000}},
    {{0, 0, 0, 0x00010000}},
    {{1, 0, 0, 0x00010000}},
    {{0xffffffff, 0xffffffff, 0xffffffff, 0xffff}},
    {{1, 0, 0, 0}},
    {{1, 0, 0, 0x80000000}},
    {{0xffffffff, 0xffffffff, 0xffffffff, 0x7ffeffff}},
    {{0xffffffff, 0xffffffff, 0xffffffff, 0xfffeffff}},
    {{0, 0, 0, 0x7fff0000}},
    {{0, 0, 0, 0xffff0000}}};
#ifdef _WIN32
static DWORD WINAPI worker(void *raw) {
#else
static void *worker(void *raw) {
#endif
  uint64_t rng = 0xcba9876543210123ULL + (unsigned)(uintptr_t)raw;
  for (unsigned n = 0; n < 1000; ++n) {
    GamePkgF128 a, b;
    for (unsigned i = 0; i < 4; ++i) {
      a.words[i] = (uint32_t)random64(&rng);
      b.words[i] = (uint32_t)random64(&rng);
    }
    a.words[3] &= 0x3fffffff;
    b.words[3] &= 0x3fffffff;
    one(a, b, n & 3, (n >> 2) & 3);
    integer(n);
    from80((GamePkgExt80){random64(&rng) | UINT64_C(0x8000000000000000),
                          (uint16_t)(random64(&rng) & 0xfffe)});
    to80(a, n & 3);
  }
  return 0;
}
static void threads(void) {
#ifdef _WIN32
  HANDLE h[8];
  for (unsigned i = 0; i < 8; ++i) {
    h[i] = CreateThread(0, 0, worker, (void *)(uintptr_t)i, 0, 0);
    assert(h[i]);
  }
  assert(WaitForMultipleObjects(8, h, 1, INFINITE) == WAIT_OBJECT_0);
  for (unsigned i = 0; i < 8; ++i)
    CloseHandle(h[i]);
#else
  pthread_t h[8];
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_create(&h[i], 0, worker, (void *)(uintptr_t)i));
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_join(h[i], 0));
#endif
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
int main(void) {
  nan_guards();
  const char *path = getenv("GAMEPKG_TAKEDAWN_F128_CORPUS");
  if (path) {
    corpus = fopen(path, "wb");
    assert(corpus);
    assert(fwrite("GPKF1281", 1, 8, corpus) == 8);
    word(0);
  }
  uint64_t rng = 0x123456789abcdef;
  unsigned n = sizeof(edges) / sizeof(*edges);
  for (unsigned op = 0; op < 4; ++op)
    for (unsigned rc = 0; rc < 4; ++rc) {
      for (unsigned i = 0; i < n; ++i)
        for (unsigned j = 0; j < n; ++j)
          one(edges[i], edges[j], op, rc);
      for (unsigned i = 0; i < 32768; ++i) {
        GamePkgF128 a, b;
        for (unsigned j = 0; j < 4; ++j) {
          a.words[j] = (uint32_t)random64(&rng);
          b.words[j] = (uint32_t)random64(&rng);
        }
        if ((a.words[3] & 0x7fff0000) == 0x7fff0000)
          a.words[3] ^= 0x10000;
        if ((b.words[3] & 0x7fff0000) == 0x7fff0000)
          b.words[3] ^= 0x10000;
        one(a, b, op, rc);
      }
    }
  for (unsigned rc = 0; rc < 4; ++rc) {
    for (unsigned i = 0; i < n; ++i)
      to80(edges[i], rc);
    for (unsigned i = 0; i < 16384; ++i) {
      GamePkgF128 a;
      for (unsigned j = 0; j < 4; ++j)
        a.words[j] = (uint32_t)random64(&rng);
      if ((a.words[3] & 0x7fff0000) == 0x7fff0000)
        a.words[3] ^= 0x10000;
      to80(a, rc);
    }
  }
  for (unsigned i = 0; i < 32768; ++i) {
    integer((uint32_t)random64(&rng));
    GamePkgExt80 a = {random64(&rng) | UINT64_C(0x8000000000000000),
                      (uint16_t)random64(&rng)};
    from80(a);
  }
  integer(0);
  integer(1);
  integer(2);
  integer(UINT32_MAX);
  const GamePkgF128 nan[] = {{{1, 0, 0, 0x7fff0000}},
                             {{0x123, 0, 0, 0x7fff8000}},
                             {{1, 0, 0, 0xffff0000}},
                             {{0, 0x123, 0, 0xffff8000}}};
  for (unsigned rc = 0; rc < 4; ++rc)
    for (unsigned i = 0; i < 4; ++i)
      to80(nan[i], rc);
  static const GamePkgExt80 special[] = {
      {0, 0},
      {0, 0x8000},
      {1, 0},
      {1, 0x8000},
      {UINT64_MAX, 0},
      {UINT64_C(0x8000000000000000), 0},
      {UINT64_C(0x8000000000000000), 0x7fff},
      {UINT64_C(0x8000000000000000), 0xffff},
      {UINT64_C(0xc000000000000001), 0x7fff},
      {UINT64_C(0x8000000000000001), 0x7fff}};
  for (unsigned i = 0; i < sizeof(special) / sizeof(*special); ++i)
    from80(special[i]);
  if (corpus) {
    assert(!fseek(corpus, 8, SEEK_SET));
    word(rows);
    assert(!fclose(corpus));
    corpus = 0;
  }
  threads();
  GamePkgF128Result r, copy;
  memset(&r, 0xa5, sizeof(r));
  copy = r;
  assert(gamepkg_x86_f128_calculate(edges[0], edges[0], 4, 0, &r) == -1 &&
         !memcmp(&r, &copy, sizeof(r)));
  assert(gamepkg_x86_f128_calculate(edges[0], edges[0], 0, 4, &r) == -1 &&
         !memcmp(&r, &copy, sizeof(r)));
  assert(gamepkg_x86_f128_calculate(edges[0], edges[0], 0, 0, 0) == -1);
  for (unsigned i = 0; i < 16; ++i) {
    GamePkgF128Result invalid;
    GamePkgExt80 a = {(uint64_t)i, (uint16_t)(0x3fff | (i & 1 ? 0x8000 : 0))};
    assert(!gamepkg_x86_f128_from_ext80(a, &invalid) &&
           invalid.exceptions == 1 && invalid.value.words[3] == 0xffff8000 &&
           !invalid.value.words[0] && !invalid.value.words[1] &&
           !invalid.value.words[2]);
  }
  printf("Wide arithmetic: %u independent GCC binary128 arithmetic/conversion "
         "cases; private/host "
         "state and8000 thread cases passed\n",
         checks);
  return 0;
}
