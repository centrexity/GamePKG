/* Synthetic exact raw80 SINCOS oracle, without game code or floating casts. */
#include "gamepkg_x86_ext80.h"
#include "gamepkg_x86_f128.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_x87_decode.h"
#include "softfloat/platform.h"
#include "softfloat/source/include/softfloat.h"
#include "transcendentals/gamepkg_fsincos.h"
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
static _Thread_local unsigned which;
extern void gamepkg_test_sincos_reference(uint64_t, uint32_t, uint32_t,
                                          uint32_t, uint64_t *, uint32_t *,
                                          uint32_t *);
#include "sincos_reference/accuracy.h"
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
  if (which)
    __asm__ volatile("fcos" ::: "memory");
  else
    __asm__ volatile("fsin" ::: "memory");
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
  if (!(empty & 1) && exponent >= 16383 - 68 && exponent < 16383 + 63 &&
      (a.significand >> 63)) {
    uint64_t sig;
    uint32_t se, c1;
    gamepkg_test_sincos_reference(a.significand, a.sign_exponent, cw, which,
                                  &sig, &se, &c1);
    if (sig != get64(after.b + 32) || se != rd(after.b + 40) ||
        c1 != ((sw >> 9) & 1))
      ++profile_differences;
    put80(after.b + 32, (GamePkgExt80){sig, (uint16_t)se});
    sw = (sw & ~0x200u) | (c1 << 9);
    wr(after.b + 2, sw);
    accuracy(a, (GamePkgExt80){sig, (uint16_t)se}, cw, sw, which);
  }
  if (!(empty & 1) && !(sw & ~cw & 3u) && a.significand &&
      exponent < 16383 - 68 && (!exponent || (a.significand >> 63)))
    accuracy(a, (GamePkgExt80){get64(after.b + 32), (uint16_t)rd(after.b + 40)},
             cw, sw, which);
  if ((z->status & ~0x4100u) != (sw & ~0x4100u)) {
    fprintf(stderr,
            "SINCOS case%u cw%x top%u empty%u a%04x:%016llx b%04x:%016llx sw "
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
              "SINCOS case%u cw%x p%u a%04x:%016llx b%04x:%016llx value "
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
         z->instruction_pointer == 0x10000 && z->opcode == (0x1fe + which) &&
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
  expected.status = (uint16_t)((sw & ~0x4100u) | (s.status & 0x4100u));
  expected.instruction_pointer = 0x10000;
  expected.opcode = (0x1fe + which);
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
    word(which);
    state_bits(&s);
    state_bits(&expected);
  }
  queued += pending;
  ++cases;
}
static const GamePkgExt80 edges[] = {
    {UINT64_C(0x8000000000000000), 16314},
    {UINT64_C(0x8000000000000000), 16315},
    {UINT64_C(0x8000000000000000), 16382},
    {UINT64_MAX, 16445},
    {UINT64_C(0x8000000000000000), 16446},
    {UINT64_C(0x8000000000000000), 0x8000 | 16446},
    {0, 0},
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
  which = id & 1;
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

/* Independent MPFR construction of inputs on both sides of fixed-pi fold
 * boundaries. The production integer reducer is not used to select cases. */
static void reduction_boundaries(void) {
  mpfr_t p, v, t;
  mpfr_inits2(512, p, v, t, (mpfr_ptr)0);
  mpfr_set_uj(p, UINT64_C(0xc90fdaa22168c234), MPFR_RNDN);
  mpfr_mul_2si(p, p, -63, MPFR_RNDN);
  mpfr_set_ui(t, 3, MPFR_RNDN);
  mpfr_mul_2si(t, t, -65, MPFR_RNDN);
  mpfr_add(p, p, t, MPFR_RNDN);
  for (unsigned e = 0; e < 63; ++e)
    for (unsigned offset = 0; offset < 4; ++offset) {
      uint64_t n = (UINT64_C(1) << e) + offset;
      mpfr_set_uj(v, n, MPFR_RNDN);
      mpfr_mul(v, p, v, MPFR_RNDN);
      mpfr_div_2ui(v, v, 1, MPFR_RNDN);
      long exp = mpfr_get_exp(v) - 1;
      if (exp >= 63)
        continue;
      mpfr_mul_2si(t, v, 63 - exp, MPFR_RNDN);
      uint64_t base = mpfr_get_uj(t, MPFR_RNDZ);
      for (unsigned side = 0; side < 3; ++side)
        for (unsigned sign = 0; sign < 2; ++sign)
          for (unsigned rc = 0; rc < 4; ++rc) {
            GamePkgExt80 x = {base, (uint16_t)(16383 + exp)};
            if (side == 0) {
              if (x.significand == UINT64_C(0x8000000000000000)) {
                --x.sign_exponent;
                x.significand = UINT64_MAX;
              } else
                --x.significand;
            } else if (side == 2) {
              if (++x.significand == 0) {
                ++x.sign_exponent;
                x.significand = UINT64_C(0x8000000000000000);
              }
            }
            x.sign_exponent |= (uint16_t)(sign << 15);
            one(x, (GamePkgExt80){0, 0}, 0x37f | (rc << 10), (e + side) & 7, 0,
                0);
          }
    }
  mpfr_clears(p, v, t, (mpfr_ptr)0);
}
static void monotonicity(void) {
  unsigned pairs = 0;
  for (unsigned op = 0; op < 2; ++op)
    for (unsigned e = 16315; e <= 16382; ++e)
      for (unsigned i = 0; i < 16; ++i) {
        GamePkgExt80 x = {UINT64_C(0x8000000000000000) | ((uint64_t)i << 59),
                          (uint16_t)e},
                     y = x;
        ++y.significand;
        for (unsigned rc = 0; rc < 4; ++rc) {
          GamePkgExt80Result a, b;
          assert(!gamepkg_fsincos_evaluate(x, op, 0x37f | (rc << 10), &a) &&
                 !gamepkg_fsincos_evaluate(y, op, 0x37f | (rc << 10), &b));
          unsigned ae = a.value.sign_exponent & 0x7fff,
                   be = b.value.sign_exponent & 0x7fff;
          int cmp = ae < be   ? -1
                    : ae > be ? 1
                    : a.value.significand < b.value.significand
                        ? -1
                        : a.value.significand > b.value.significand;
          assert(op ? cmp >= 0 : cmp <= 0);
          ++pairs;
        }
      }
  printf("SINCOS %u local monotonic pairs passed\n", pairs);
}
int main(void) {
  const char *path = getenv("GAMEPKG_TAKEDAWN_X87_SINCOS_CORPUS");
  if (path) {
    corpus = fopen(path, "wb");
    assert(corpus && fwrite("GPKXSCN1", 1, 8, corpus) == 8);
    word(0);
  }
  gamepkg_x86_x87_bind_segments(cs, 0x23, 0x2b, 0x3b);
  uint64_t rng = 0x123456789abcdef;
  unsigned n = sizeof(edges) / sizeof(*edges);
  for (which = 0; which < 2; ++which) {
    for (unsigned top = 0; top < 8; ++top)
      for (unsigned pc = 0; pc < 4; ++pc)
        for (unsigned rc = 0; rc < 4; ++rc)
          for (unsigned masks = 0; masks < 64; ++masks)
            for (unsigned i = 0; i < n; ++i)
              one(edges[i], edges[(i + 1) % n],
                  0x40 | masks | (pc << 8) | (rc << 10), top, 0,
                  (unsigned)random64(&rng));
    for (unsigned top = 0; top < 8; ++top)
      for (unsigned pc = 0; pc < 4; ++pc)
        for (unsigned rc = 0; rc < 4; ++rc)
          for (unsigned masks = 0; masks < 64; ++masks)
            for (unsigned empty = 0; empty < 4; ++empty)
              one(edges[(top + masks) % n], edges[(top + masks + 1) % n],
                  0x40 | masks | (pc << 8) | (rc << 10), top, empty,
                  (unsigned)random64(&rng));
    for (unsigned i = 0; i < 32768; ++i) {
      GamePkgExt80 x = {random64(&rng) | UINT64_C(0x8000000000000000),
                        (uint16_t)((16383 - 128 + i % 191) | ((i & 1) << 15))};
      one(x, edges[i % n], 0x37f | ((i & 3) << 10), i & 7, 0, 0);
    }

    reduction_boundaries();
    static const unsigned exps[] = {0,     1,     2,     16314, 16315,
                                    16326, 16327, 16328, 16342, 16343,
                                    16381, 16382, 16445, 16446};
    static const uint64_t sigs[] = {UINT64_C(0x8000000000000000),
                                    UINT64_C(0x8000000000000001),
                                    UINT64_C(0xc90fdaa22168c234), UINT64_MAX};
    for (unsigned e = 0; e < 14; ++e)
      for (unsigned i = 0; i < 4; ++i)
        for (unsigned sign = 0; sign < 2; ++sign)
          for (unsigned pc = 0; pc < 4; ++pc)
            for (unsigned rc = 0; rc < 4; ++rc)
              for (unsigned masks = 0; masks < 64; ++masks)
                one((GamePkgExt80){sigs[i], (uint16_t)(exps[e] | (sign << 15))},
                    edges[0], 0x40 | masks | (pc << 8) | (rc << 10),
                    (e + i) & 7, 0, 0);
  }
  if (corpus) {
    assert(!fseek(corpus, 8, SEEK_SET));
    word(cases);
    assert(!fclose(corpus));
    corpus = 0;
  }
  accuracy_clear();
  monotonicity();
  threads();
  for (which = 0; which < 2; ++which) {
    GamePkgX87State before = *gamepkg_x86_x87_state();
    before.control = 0x37e;
    before.status = 0x8081;
    *gamepkg_x86_x87_state() = before;
    GamePkgX86Fault f;
    assert(gamepkg_x86_run(generated, &f) == 1 && f.vector == 16 &&
           !memcmp(&before, gamepkg_x86_x87_state(), sizeof(before)));
  }
  printf("SINCOS: %u states, %u deferred faults, %u hardware approximation "
         "differences plus8000 workers passed\n",
         cases, queued, profile_differences);
  return 0;
}
