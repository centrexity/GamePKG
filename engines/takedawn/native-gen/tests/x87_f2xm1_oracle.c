/* Synthetic exact raw80 F2XM1 oracle, without game code or floating casts. */
#include "gamepkg_x86_ext80.h"
#include "gamepkg_x86_f128.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_x87_decode.h"
#include "softfloat/platform.h"
#include "softfloat/source/include/softfloat.h"
#include "transcendentals/gamepkg_f2xm1.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#ifdef __x86_64__
#define FX_CLOBBERS                                                            \
  "memory", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7",    \
      "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15"
#else
#define FX_CLOBBERS                                                            \
  "memory", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7"
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
typedef struct {
  _Alignas(16) unsigned char b[512];
} Image;
_Thread_local uint_fast8_t gamepkg_softfloat_unbounded_inexact;
static void generated(void);
extern void gamepkg_test_f2xm1_reference(uint64_t, uint32_t, uint32_t,
                                         uint64_t *, uint32_t *, uint32_t *);
#include "f2xm1_reference/accuracy.h"
static _Thread_local unsigned profile_differences;
static _Thread_local unsigned cases, queued, cs = 0x1b;
static FILE *corpus;
static void word(uint32_t v) {
  unsigned char b[4];
  for (unsigned i = 0; i < 4; ++i)
    b[i] = (unsigned char)(v >> (8 * i));
  assert(fwrite(b, 1, 4, corpus) == 4);
}
static void state_bits(const GamePkgX87State *s) {
  for (unsigned i = 0; i < 8; ++i) {
    word((uint32_t)s->significand[i]);
    word((uint32_t)(s->significand[i] >> 32));
    word(s->sign_exponent[i]);
  }
  word(s->control);
  word(s->status);
  word(s->tags);
  word(s->instruction_pointer);
  word(s->data_pointer);
  word(s->opcode);
  word(s->instruction_selector);
  word(s->data_selector);
}
static unsigned rd(const unsigned char *p) {
  return p[0] | ((unsigned)p[1] << 8);
}
static void wr(unsigned char *p, unsigned v) {
  p[0] = (unsigned char)v;
  p[1] = (unsigned char)(v >> 8);
}
static uint64_t get64(const unsigned char *p) {
  uint64_t v = 0;
  for (unsigned i = 0; i < 8; ++i)
    v |= (uint64_t)p[i] << (8 * i);
  return v;
}
static void put80(unsigned char *p, GamePkgExt80 v) {
  for (unsigned i = 0; i < 8; ++i)
    p[i] = (unsigned char)(v.significand >> (8 * i));
  wr(p + 8, v.sign_exponent);
}
static unsigned classify(GamePkgExt80 v) {
  unsigned e = v.sign_exponent & 0x7fff;
  return !e && !v.significand                        ? 1
         : e && e != 0x7fff && (v.significand >> 63) ? 0
                                                     : 2;
}
static uint64_t random64(uint64_t *v) {
  *v ^= *v << 13;
  *v ^= *v >> 7;
  *v ^= *v << 17;
  return *v;
}
static void next_wait(void) { gamepkg_x86_x87_wait(0x20000, 0x9876); }
static void one(GamePkgExt80 a, GamePkgExt80 b, unsigned cw, unsigned top,
                unsigned empty, unsigned sticky) {
  Image old, input, after;
  __asm__ volatile("fxsave %0" : "=m"(old)::"memory");
  input = old;
  GamePkgX87State s = {0};
  s.control = (uint16_t)cw;
  s.status = (uint16_t)((top << 11) | (sticky & 0x4740) | (sticky & cw & 63));
  s.instruction_pointer = 0x40000;
  s.data_pointer = 0x50000;
  s.opcode = 0x123;
  s.instruction_selector = 0x77;
  s.data_selector = 0x2b;
  wr(input.b, cw);
  wr(input.b + 2, s.status);
  input.b[4] = 0xff;
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    GamePkgExt80 v = i == 0   ? a
                     : i == 1 ? b
                              : (GamePkgExt80){UINT64_C(0x8000000000000000) + i,
                                               (uint16_t)(0x3fff + i)};
    s.significand[p] = v.significand;
    s.sign_exponent[p] = v.sign_exponent;
    unsigned tag = classify(v);
    if (i < 2 && (empty & (1u << i))) {
      tag = 3;
      input.b[4] &= (unsigned char)~(1u << p);
    }
    s.tags |= (uint16_t)(tag << (2 * p));
    put80(input.b + 32 + 16 * i, v);
  }
  __asm__ volatile("fxrstor %0" ::"m"(input) : FX_CLOBBERS);
  __asm__ volatile("f2xm1" ::: "memory");
  __asm__ volatile("fxsave %0" : "=m"(after)::"memory");
  __asm__ volatile("fxrstor %0" ::"m"(old) : FX_CLOBBERS);
  *gamepkg_x86_x87_state() = s;
  softfloat_roundingMode = softfloat_round_odd;
  softfloat_detectTininess = softfloat_tininess_beforeRounding;
  softfloat_exceptionFlags = 0x1f;
  extF80_roundingPrecision = 32;
  gamepkg_softfloat_unbounded_inexact = 0x55;
  g_eax = 0x1234;
  g_eflags = 0x8d7;
  g_fs_base = 0x789a;
  GamePkgX86Fault f;
  assert(!gamepkg_x86_run(generated, &f));
  GamePkgX87State *z = gamepkg_x86_x87_state();
  unsigned sw = rd(after.b + 2), valid = 0;
  /* The fixed P5/P6 numerical profile is checked bit-for-bit against the
   * independently built pinned reference. Hardware approximation last bits may
   * differ: all other registers, exception/status bits and commit responses
   * continue to use the physical oracle unchanged. */
  unsigned exponent = a.sign_exponent & 0x7fff;
  if (!(empty & 1) && exponent && exponent < 0x3fff && (a.significand >> 63)) {
    uint64_t sig;
    uint32_t se, c1;
    gamepkg_test_f2xm1_reference(a.significand, a.sign_exponent, cw, &sig, &se,
                                 &c1);
    if (sig != get64(after.b + 32) || se != rd(after.b + 40) ||
        c1 != ((sw >> 9) & 1))
      ++profile_differences;
    put80(after.b + 32, (GamePkgExt80){sig, (uint16_t)se});
    sw = (sw & ~0x200u) | (c1 << 9);
    wr(after.b + 2, sw);
    accuracy(a, (GamePkgExt80){sig, (uint16_t)se}, cw, sw);
  }
  /* Exact endpoints also have a fixed approximation profile. In particular,
   * Intel Xeon 8573C clears C1 for +1/nearest where the earlier qualified
   * hardware sets it. Do not select a profile from the machine running CI or
   * ignore C1: check the adapter's explicit endpoint table instead. Exception
   * flags, commit/stack behavior and every other register still use hardware.
   * This does not establish original Xbox endpoint equivalence. */
  if (!(empty & 1) && exponent == 0x3fff &&
      a.significand == UINT64_C(0x8000000000000000)) {
    static const GamePkgExt80 positive[4] = {
        {UINT64_C(0x8000000000000000), 0x3fff},
        {UINT64_MAX, 0x3ffe},
        {UINT64_C(0x8000000000000000), 0x3fff},
        {UINT64_MAX, 0x3ffe}};
    unsigned rc = (cw >> 10) & 3, negative = a.sign_exponent >> 15;
    GamePkgExt80 value = negative
                            ? (GamePkgExt80){UINT64_C(0x8000000000000000), 0xbffe}
                            : positive[rc];
    unsigned c1 = !negative && (rc == 0 || rc == 2);
    if (value.significand != get64(after.b + 32) ||
        value.sign_exponent != rd(after.b + 40) || c1 != ((sw >> 9) & 1))
      ++profile_differences;
    put80(after.b + 32, value);
    sw = (sw & ~0x200u) | (c1 << 9);
    wr(after.b + 2, sw);
    accuracy(a, value, cw, sw);
  }
  if ((z->status & ~0x4500u) != (sw & ~0x4500u)) {
    fprintf(stderr,
            "F2XM1 case%u cw%x top%u empty%u a%04x:%016llx b%04x:%016llx sw "
            "got%04x expected%04x\n",
            cases, cw, top, empty, a.sign_exponent,
            (unsigned long long)a.significand, b.sign_exponent,
            (unsigned long long)b.significand, z->status, sw);
    unsigned pp = (s.status >> 11) & 7;
    fprintf(stderr, "value got%04x:%016llx expected%04x:%016llx\n",
            z->sign_exponent[pp], (unsigned long long)z->significand[pp],
            rd(after.b + 40), (unsigned long long)get64(after.b + 32));
    abort();
  }
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    uint64_t sig = get64(after.b + 32 + 16 * i);
    unsigned se = rd(after.b + 40 + 16 * i);
    if (z->significand[p] != sig || z->sign_exponent[p] != se) {
      fprintf(stderr,
              "F2XM1 case%u cw%x p%u a%04x:%016llx b%04x:%016llx value "
              "got%04x:%016llx expected%04x:%016llx sw%x\n",
              cases, cw, p, a.sign_exponent, (unsigned long long)a.significand,
              b.sign_exponent, (unsigned long long)b.significand,
              z->sign_exponent[p], (unsigned long long)z->significand[p], se,
              (unsigned long long)sig, sw);
      abort();
    }
    if (((z->tags >> (2 * p)) & 3) != 3)
      valid |= 1u << p;
  }
  assert(valid == after.b[4] && z->control == cw &&
         z->instruction_pointer == 0x10000 && z->opcode == 0x1f0 &&
         z->data_pointer == s.data_pointer);
  assert(g_eax == 0x1234 && g_eflags == 0x8d7 && g_fs_base == 0x789a);
  assert(softfloat_roundingMode == softfloat_round_odd &&
         softfloat_detectTininess == softfloat_tininess_beforeRounding &&
         softfloat_exceptionFlags == 0x1f && extF80_roundingPrecision == 32 &&
         gamepkg_softfloat_unbounded_inexact == 0x55);
  GamePkgX87State before = *z;
  unsigned pending = (sw & ~cw & 63u) != 0;
  assert(gamepkg_x86_run(next_wait, &f) == (int)pending &&
         (!pending || (f.vector == 16 && f.eip == 0x20000 && f.ebp == 0x9876)));
  assert(!memcmp(&before, z, sizeof(before)));
  GamePkgX87State expected = s;
  expected.status = (uint16_t)((sw & ~0x4500u) | (s.status & 0x4500u));
  expected.instruction_pointer = 0x10000;
  expected.opcode = 0x1f0;
  expected.instruction_selector = (uint16_t)cs;
  expected.tags = 0;
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    GamePkgExt80 value = {get64(after.b + 32 + 16 * i),
                          (uint16_t)rd(after.b + 40 + 16 * i)};
    expected.significand[p] = value.significand;
    expected.sign_exponent[p] = value.sign_exponent;
    expected.tags |=
        (uint16_t)(((after.b[4] & (1u << p)) ? classify(value) : 3u)
                   << (2 * p));
  }
  assert(!memcmp(z, &expected, sizeof(expected)));
  if (corpus) {
    state_bits(&s);
    state_bits(&expected);
  }
  queued += pending;
  ++cases;
}
static const GamePkgExt80 edges[] = {{0, 0},
                                     {0, 0x8000},
                                     {UINT64_C(0x8000000000000000), 0x3fff},
                                     {UINT64_C(0x8000000000000000), 0xbfff},
                                     {UINT64_MAX, 0x3fff},
                                     {UINT64_MAX, 0x7ffe},
                                     {UINT64_MAX, 0xfffe},
                                     {1, 0},
                                     {1, 0x8000},
                                     {UINT64_C(0x7fffffffffffffff), 0},
                                     {UINT64_C(0x8000000000000000), 0},
                                     {UINT64_C(0x8000000000000000), 1},
                                     {UINT64_C(0x8000000000000000), 0x7fff},
                                     {UINT64_C(0x8000000000000000), 0xffff},
                                     {UINT64_C(0xc000000000000001), 0x7fff},
                                     {UINT64_C(0xc000000000000007), 0xffff},
                                     {UINT64_C(0x8000000000000001), 0x7fff},
                                     {UINT64_C(0x8000000000000007), 0xffff},
                                     {0, 1},
                                     {1, 1},
                                     {UINT64_C(0x4000000000000000), 0x3fff},
                                     {0, 0x7fff},
                                     {1, 0xffff},
                                     {UINT64_C(0x8000000000000001), 0x400c},
                                     {UINT64_C(0x8000000000000001), 0xc00c},
                                     {UINT64_C(0x8000000000000000), 0x400d},
                                     {UINT64_C(0x8000000000000000), 0xc00d},
                                     {UINT64_C(0x8000000000000000), 0x400e},
                                     {UINT64_C(0x8000000000000000), 0xc00e},
                                     {UINT64_MAX, 0x400e},
                                     {UINT64_MAX, 0xc00e},
                                     {UINT64_C(0x8000000000000000), 0x400f},
                                     {UINT64_C(0x8000000000000000), 0xc00f},
                                     {UINT64_C(0x8000000000000000), 0x3ffe},
                                     {UINT64_C(0x8000000000000000), 0xbffe},
                                     {UINT64_C(0x8000000000000000), 0x4020},
                                     {UINT64_C(0x8000000000000000), 0xc020}};

#ifdef _WIN32
static DWORD WINAPI worker(void *raw) {
#else
static void *worker(void *raw) {
#endif
  unsigned id = (unsigned)(uintptr_t)raw;
  cs = 0x10 + id;
  gamepkg_x86_x87_bind_segments(cs, 0x23, 0x2b, 0x3b);
  uint64_t rng = 0xcba9876543210123ULL + id;
  for (unsigned n = 0; n < 1000; ++n) {
    GamePkgExt80 a = {random64(&rng), (uint16_t)random64(&rng)},
                 b = {random64(&rng), (uint16_t)random64(&rng)};
    one(a, b, 0x40 | (n & 63) | ((n & 15) << 8), n & 7, (n >> 4) & 3,
        (unsigned)random64(&rng));
  }
  accuracy_clear();
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
static unsigned monotonic_pairs;
static uint64_t seed = 0x123456789abcdef;
static uint64_t next(void) {
  seed ^= seed << 13;
  seed ^= seed >> 7;
  seed ^= seed << 17;
  return seed;
}
static GamePkgExt80 eval(GamePkgExt80 a, unsigned rc) {
  GamePkgF128 q;
  int32_t scale;
  GamePkgExt80Result r;
  gamepkg_f2xm1_evaluate(a, rc, &q, &scale);
  gamepkg_x86_ext80_round_f128(q.words, scale, 0x37f | (rc << 10), &r);
  return r.value;
}
static void pair(GamePkgExt80 a) {
  GamePkgExt80 b = a;
  if (++b.significand == 0) {
    b.significand = UINT64_C(0x8000000000000000);
    ++b.sign_exponent;
  }
  if ((b.sign_exponent & 0x7fff) >= 0x3fff)
    return;
  for (unsigned sign = 0; sign < 2; ++sign)
    for (unsigned rc = 0; rc < 4; ++rc) {
      a.sign_exponent = (uint16_t)((a.sign_exponent & 0x7fff) | (sign << 15));
      b.sign_exponent = (uint16_t)((b.sign_exponent & 0x7fff) | (sign << 15));
      GamePkgExt80 x = eval(a, rc), y = eval(b, rc);
      unsigned xe = x.sign_exponent & 0x7fff, ye = y.sign_exponent & 0x7fff;
      if (ye < xe || (xe == ye && y.significand < x.significand)) {
        fprintf(stderr,
                "nonmonotonic sign%u rc%u a%04x:%016llx x%04x:%016llx "
                "y%04x:%016llx\n",
                sign, rc, a.sign_exponent, (unsigned long long)a.significand,
                x.sign_exponent, (unsigned long long)x.significand,
                y.sign_exponent, (unsigned long long)y.significand);
        _Exit(1);
      }
      ++monotonic_pairs;
    }
}
static void monotonicity(void) {
  for (unsigned i = 0; i < 32768; ++i)
    pair((GamePkgExt80){next() | UINT64_C(0x8000000000000000),
                        (uint16_t)(16382 - i % 128)});
  for (unsigned exp = 16314; exp <= 16382; ++exp)
    for (unsigned i = 0; i < 16; ++i) {
      uint64_t sig = UINT64_C(0x8000000000000000) | ((uint64_t)i << 59);
      pair((GamePkgExt80){sig, (uint16_t)exp});
      if (sig > UINT64_C(0x8000000000000000))
        pair((GamePkgExt80){sig - 1, (uint16_t)exp});
      else
        pair((GamePkgExt80){UINT64_MAX, (uint16_t)(exp - 1)});
    }
  printf("%u adjacent monotonic pairs passed\n", monotonic_pairs);
}

int main(void) {
  const char *path = getenv("GAMEPKG_TAKEDAWN_X87_F2XM1_CORPUS");
  if (path) {
    corpus = fopen(path, "wb");
    assert(corpus && fwrite("GPKXF2M1", 1, 8, corpus) == 8);
    word(0);
  }
  gamepkg_x86_x87_bind_segments(cs, 0x23, 0x2b, 0x3b);
  uint64_t rng = 0x123456789abcdef;
  unsigned n = sizeof(edges) / sizeof(*edges);
  /* Algebraically constructed wide-round carries, all PC/RC/masks/signs. */
  static const uint64_t carries[] = {
      UINT64_C(0xc3be0e51d9680e75), UINT64_C(0xcfb7d5be2ab50985),
      UINT64_C(0xdbb19d2a7c020495), UINT64_C(0xe7ab6496cd4effa5)};
  for (unsigned i = 0; i < 4; ++i)
    for (unsigned pc = 0; pc < 4; ++pc)
      for (unsigned rc = 0; rc < 4; ++rc)
        for (unsigned masks = 0; masks < 64; ++masks)
          for (unsigned sign = 0; sign < 2; ++sign)
            one((GamePkgExt80){carries[i], (uint16_t)(16283 | (sign << 15))},
                edges[0], 0x40 | masks | (pc << 8) | (rc << 10), i, 0, 0);
  for (unsigned pc = 0; pc < 4; ++pc)
    for (unsigned rc = 0; rc < 4; ++rc)
      for (unsigned masks = 0; masks < 64; ++masks) {
        unsigned cw = 0x40 | masks | (pc << 8) | (rc << 10);
        for (unsigned a = 0; a < n; ++a)
          one(edges[a], edges[(a + 1) % n], cw, a & 7, 0,
              (unsigned)random64(&rng));
      }
  for (unsigned rc = 0; rc < 4; ++rc)
    for (unsigned i = 0; i < 8192; ++i) {
      GamePkgExt80 a = {random64(&rng) | UINT64_C(0x8000000000000000),
                        (uint16_t)(16383 - 1 - (i % 128))};
      if (i & 1)
        a.sign_exponent |= 0x8000;
      one(a, edges[i % n], 0x37f | (rc << 10), i & 7, 0,
          (unsigned)random64(&rng));
    }

  /* Exercise all physical TOP positions and absent operands independently. */
  for (unsigned top = 0; top < 8; ++top)
    for (unsigned pc = 0; pc < 4; ++pc)
      for (unsigned rc = 0; rc < 4; ++rc)
        for (unsigned masks = 0; masks < 64; ++masks)
          for (unsigned empty = 0; empty < 4; ++empty)
            one(edges[(top + masks) % n], edges[(top + masks + 1) % n],
                0x40 | masks | (pc << 8) | (rc << 10), top, empty,
                (unsigned)random64(&rng));
  /* Adjacent values across every reduction table boundary, quarter and tiny
   * path transitions. Each pair is also independently accuracy checked. */
  for (unsigned e = 16314; e <= 16382; ++e)
    for (unsigned index = 0; index < 16; ++index)
      for (unsigned side = 0; side < 3; ++side)
        for (unsigned sign = 0; sign < 2; ++sign)
          for (unsigned rc = 0; rc < 4; ++rc) {
            uint64_t sig =
                UINT64_C(0x8000000000000000) | ((uint64_t)index << 59);
            unsigned exp = e;
            if (side == 0 && sig == UINT64_C(0x8000000000000000)) {
              sig = UINT64_MAX;
              --exp;
            } else if (side == 0)
              --sig;
            else if (side == 2)
              ++sig;
            one((GamePkgExt80){sig, (uint16_t)(exp | (sign << 15))}, edges[0],
                0x37f | (rc << 10), (index + side) & 7, 0, 0);
          }
  /* The exponent endpoints and gradual/wrapped underflow boundary. */
  for (unsigned e = 0; e <= 3; ++e)
    for (unsigned sign = 0; sign < 2; ++sign)
      for (unsigned rc = 0; rc < 4; ++rc)
        for (unsigned mask = 0; mask < 64; ++mask)
          for (unsigned i = 0; i < 8; ++i) {
            uint64_t sig = UINT64_MAX - i;
            if (i < 4)
              sig = UINT64_C(0x8000000000000000) + i;
            one((GamePkgExt80){sig, (uint16_t)(e | (sign << 15))}, edges[0],
                0x340 | mask | (rc << 10), i, 0, 0);
          }
  if (corpus) {
    assert(!fseek(corpus, 8, SEEK_SET));
    word(cases);
    assert(!fclose(corpus));
    corpus = 0;
  }
  monotonicity();
  accuracy_clear();
  threads();
  GamePkgX87State before = *gamepkg_x86_x87_state();
  before.control = 0x37e;
  before.status = 0x8081;
  *gamepkg_x86_x87_state() = before;
  GamePkgX86Fault f;
  assert(gamepkg_x86_run(generated, &f) == 1 && f.vector == 16 &&
         !memcmp(&before, gamepkg_x86_x87_state(), sizeof(before)));
  printf("F2XM1: %u state cases, %u queued faults, %u hardware approximation "
         "differences and 8000 workers passed\n",
         cases, queued, profile_differences);
  return 0;
}
