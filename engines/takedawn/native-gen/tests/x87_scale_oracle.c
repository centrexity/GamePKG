/* Synthetic exact raw80 FSCALE oracle, without game code or floating casts. */
#include "gamepkg_x86_ext80.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_x87_decode.h"
#include "softfloat/platform.h"
#include "softfloat/source/include/softfloat.h"
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
  __asm__ volatile("fscale" ::: "memory");
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
  if ((z->status & ~0x4500u) != (sw & ~0x4500u)) {
    fprintf(stderr,
            "scale case%u cw%x top%u empty%u a%04x:%016llx b%04x:%016llx sw "
            "got%04x expected%04x\n",
            cases, cw, top, empty, a.sign_exponent,
            (unsigned long long)a.significand, b.sign_exponent,
            (unsigned long long)b.significand, z->status, sw);
    abort();
  }
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    uint64_t sig = get64(after.b + 32 + 16 * i);
    unsigned se = rd(after.b + 40 + 16 * i);
    if (z->significand[p] != sig || z->sign_exponent[p] != se) {
      fprintf(stderr,
              "scale case%u cw%x p%u a%04x:%016llx b%04x:%016llx value "
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
         z->instruction_pointer == 0x10000 && z->opcode == 0x1fd &&
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
  expected.opcode = 0x1fd;
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
int main(void) {
  gamepkg_x86_x87_bind_segments(cs, 0x23, 0x2b, 0x3b);
  const char *path = getenv("GAMEPKG_TAKEDAWN_X87_SCALE_CORPUS");
  if (path) {
    corpus = fopen(path, "wb");
    assert(corpus);
    assert(fwrite("GPKXSCL1", 1, 8, corpus) == 8);
    word(0);
  }
  uint64_t rng = 0x123456789abcdef;
  unsigned n = sizeof(edges) / sizeof(*edges);
  for (unsigned pc = 0; pc < 4; ++pc)
    for (unsigned rc = 0; rc < 4; ++rc)
      for (unsigned masks = 0; masks < 64; ++masks) {
        unsigned cw = 0x40 | masks | (pc << 8) | (rc << 10);
        for (unsigned a = 0; a < n; ++a)
          for (unsigned b = 0; b < n; ++b)
            one(edges[a], edges[b], cw, (a + b) & 7, 0,
                (unsigned)random64(&rng));
      }
  for (unsigned top = 0; top < 8; ++top)
    for (unsigned pc = 0; pc < 4; ++pc)
      for (unsigned rc = 0; rc < 4; ++rc)
        for (unsigned masks = 0; masks < 64; ++masks)
          for (unsigned empty = 0; empty < 4; ++empty) {
            GamePkgExt80 a = {random64(&rng), (uint16_t)random64(&rng)},
                         b = {random64(&rng), (uint16_t)random64(&rng)};
            one(a, b, 0x40 | masks | (pc << 8) | (rc << 10), top, empty,
                (unsigned)random64(&rng));
          }
  const GamePkgExt80 boundaries[] = {{1, 0},
                                     {UINT64_C(0x8000000000000001), 1},
                                     {UINT64_C(0x8000000000000001), 0x3fff},
                                     {UINT64_MAX, 0x7ffe},
                                     {UINT64_C(0x8000000000000001), 0}};
  for (int32_t k = -65537; k <= 65537; ++k)
    for (unsigned i = 0; i < 5; ++i) {
      unsigned phase = ((uint32_t)k + i) & 15;
      static const unsigned masks[] = {63, 55, 47, 61};
      GamePkgExt80 scale = gamepkg_x87_decode_integer((uint32_t)k, 32);
      one(boundaries[i], scale,
          0x40 | masks[phase & 3] | ((phase & 3) << 8) | ((phase >> 2) << 10),
          ((uint32_t)k + i) & 7, 0, (unsigned)random64(&rng));
    }
  if (corpus) {
    assert(fseek(corpus, 8, SEEK_SET) == 0);
    word(cases);
    assert(fclose(corpus) == 0);
    corpus = 0;
  }
  threads();
  GamePkgX87State before = *gamepkg_x86_x87_state();
  before.control = 0x37e;
  before.status = 0x8081;
  *gamepkg_x86_x87_state() = before;
  GamePkgX86Fault f;
  assert(gamepkg_x86_run(generated, &f) == 1 && f.vector == 16 &&
         !memcmp(&before, gamepkg_x86_x87_state(), sizeof(before)));
  printf("FSCALE: %u hardware state cases, %u deferred next-wait faults and "
         "old-pending gate plus8000 workers passed\n",
         cases, queued);
  return 0;
}
