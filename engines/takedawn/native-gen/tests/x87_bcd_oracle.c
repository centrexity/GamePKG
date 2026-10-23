/* Synthetic packed-decimal oracle; no title code or assets. Intel SDM vol1
 * packed BCD / vol2 FBLD/FBSTP define the valid digit domain and indefinite. */
#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_x87_decode.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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
typedef struct {
  unsigned char b[10];
} Bcd;
static unsigned char ram[16];
static unsigned get16(const unsigned char *p) {
  return p[0] | ((unsigned)p[1] << 8);
}
static uint64_t get64(const unsigned char *p) {
  uint64_t v = 0;
  for (unsigned i = 0; i < 8; ++i)
    v |= (uint64_t)p[i] << (i * 8);
  return v;
}
static void put(unsigned char *p, uint64_t v, unsigned n) {
  for (unsigned i = 0; i < n; ++i)
    p[i] = (unsigned char)(v >> (8 * i));
}
static uint64_t random64(uint64_t *p) {
  *p ^= *p << 13;
  *p ^= *p >> 7;
  *p ^= *p << 17;
  return *p;
}
static unsigned tag(GamePkgExt80 v) {
  unsigned e = v.sign_exponent & 0x7fff;
  return !e && !v.significand                        ? 1
         : e && e != 0x7fff && (v.significand >> 63) ? 0
                                                     : 2;
}
static void prepare(Image *image, GamePkgX87State *s, unsigned cw, unsigned top,
                    unsigned empty, GamePkgExt80 input) {
  memset(s, 0, sizeof(*s));
  s->control = (uint16_t)cw;
  s->status = (uint16_t)((top << 11) | 0x4700);
  s->instruction_pointer = 0x40000;
  s->data_pointer = 0x50000;
  s->opcode = 0x123;
  put(image->b, cw, 2);
  put(image->b + 2, s->status, 2);
  image->b[4] = 0;
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    GamePkgExt80 v = i ? gamepkg_x87_decode_integer(i + 19, 64) : input;
    unsigned vacant = (empty == 1 && i == 0) || (empty == 2 && i == 7);
    s->significand[p] = v.significand;
    s->sign_exponent[p] = v.sign_exponent;
    s->tags |= (uint16_t)((vacant ? 3 : tag(v)) << (2 * p));
    if (!vacant)
      image->b[4] |= (unsigned char)(1u << p);
    put(image->b + 32 + 16 * i, v.significand, 8);
    put(image->b + 40 + 16 * i, v.sign_exponent, 2);
  }
}
static unsigned expected_opcode, expected_data_pointer;
static void compare(const Image *native, unsigned op, unsigned cw, unsigned top,
                    unsigned empty, GamePkgExt80 input) {
  const GamePkgX87State *s = gamepkg_x86_x87_state();
  unsigned sw = get16(native->b + 2), next = (sw >> 11) & 7, valid = 0;
  if ((s->status & ~0x4500u) != (sw & ~0x4500u)) {
    fprintf(stderr,
            "BCD op%u cw%04x top%u empty%u input%04x:%016llx actualSW%04x "
            "expectedSW%04x\n",
            op, cw, top, empty, input.sign_exponent,
            (unsigned long long)input.significand, s->status, sw);
    assert(0);
  }
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (next + i) & 7;
    assert(s->significand[p] == get64(native->b + 32 + 16 * i));
    assert(s->sign_exponent[p] == get16(native->b + 40 + 16 * i));
    if (((s->tags >> (2 * p)) & 3) != 3)
      valid |= 1u << p;
  }
  assert(s->tags == get16(native->b + 6));
  assert(valid == native->b[4] && s->control == cw &&
         s->instruction_pointer == 0x10000 && s->opcode == expected_opcode &&
         s->data_pointer == expected_data_pointer);
}
static void load(void) {
  uint32_t words[4];
  gamepkg_x86_x87_wait(0x10000, 0x1234);
  gamepkg_x86_memory_read(1025, words, 10, 0x10000, 0x1234);
  gamepkg_x86_x87_load(14, words[0], words[1], words[2], 0x10000, 0x720, 1025,
                       0x1234);
}
static void store(void) {
  gamepkg_x86_x87_store(7, 0, 1, 0x10000, 0x730, 1025, 0x1234);
}
static unsigned reads;
uint32_t xbox_MmioLoad(uint32_t address, int width) {
  ++reads;
  uint32_t words[4];
  assert(width == 2 || width == 4);
  gamepkg_x86_memory_read(address, words, (uint32_t)width, 0x10000, 1025);
  return words[0];
}
/* GENERATED */
static void wait_next(void) { gamepkg_x86_x87_wait(0x20000, 0x5678); }
static void bind(unsigned bytes, unsigned permissions) {
  GamePkgX86Region region = {1024, bytes, permissions, ram};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&region, 1, 0);
  assert(m);
  gamepkg_x86_memory_bind(m);
  gamepkg_x86_memory_release(m);
}
static FILE *corpus;
static void corpus_record(Image *native, Bcd *expected) {
  unsigned char record[95];
#ifdef GAMEPKG_BCD_REPLAY
  assert(corpus && fread(record, 1, sizeof(record), corpus) == sizeof(record));
  put(native->b + 2, get16(record), 2);
  native->b[4] = record[2];
  put(native->b + 6, get16(record + 3), 2);
  for (unsigned i = 0; i < 8; ++i)
    memcpy(native->b + 32 + 16 * i, record + 5 + 10 * i, 10);
  memcpy(expected->b, record + 85, 10);
#else
  if (!corpus)
    return;
  put(record, get16(native->b + 2) & ~0x4500u, 2);
  record[2] = native->b[4];
  put(record + 3, get16(native->b + 6), 2);
  for (unsigned i = 0; i < 8; ++i)
    memcpy(record + 5 + 10 * i, native->b + 32 + 16 * i, 10);
  memcpy(record + 85, expected->b, 10);
  assert(fwrite(record, 1, sizeof(record), corpus) == sizeof(record));
#endif
}
int main(void) {
  const char *path = getenv("GAMEPKG_BCD_CORPUS");
#ifdef GAMEPKG_BCD_REPLAY
  assert(path);
  corpus = fopen(path, "rb");
  assert(corpus);
  unsigned char header[8];
  assert(fread(header, 1, 8, corpus) == 8 && !memcmp(header, "GPKBCD02", 8));
#else
  if (path) {
    corpus = fopen(path, "wb");
    assert(corpus && fwrite("GPKBCD02", 1, 8, corpus) == 8);
  }
#endif
  Image original = {{0}}, initial, native;
#ifndef GAMEPKG_BCD_REPLAY
  __asm__ volatile("fxsave %0" : "=m"(original)::"memory");
#endif
  uint64_t rng = UINT64_C(0x9a412fbc357);
  unsigned long checks = 0;
  GamePkgX86Fault fault;
  bind(sizeof(ram), 3);
  static const GamePkgExt80 edge[] = {
      {0, 0},
      {0, 0x8000},
      {UINT64_C(0x8000000000000000), 0x3ffe},
      {UINT64_C(0x8000000000000000), 0xbffe},
      {UINT64_C(0xc000000000000000), 0x3fff},
      {UINT64_C(0xc000000000000000), 0xbfff},
      {1, 0},
      {1, 0x8000},
      {UINT64_C(0x8000000000000000), 0},
      {UINT64_C(0x8000000000000000), 0x8000},
      {UINT64_C(0x8000000000000000), 0x7fff},
      {UINT64_C(0x8000000000000000), 0xffff},
      {UINT64_C(0xc000000000000001), 0x7fff},
      {UINT64_C(0x8000000000000001), 0x7fff},
      {1, 0x4000},
      {UINT64_MAX, 0x7ffe},
      {(UINT64_C(1000000000000000000) << 4) - 16, 0x403a},
      {(UINT64_C(1000000000000000000) << 4) - 8, 0x403a},
      {(UINT64_C(1000000000000000000) << 4) - 8, 0xc03a},
      {(UINT64_C(1000000000000000000) << 4) + 8, 0x403a},
      {(UINT64_C(1000000000000000000) << 4) + 8, 0xc03a}};
  for (unsigned form = 0; form < 7; ++form)
    for (unsigned op = 0; op < 2; ++op)
      for (unsigned rc = 0; rc < 4; ++rc)
        for (unsigned masks = 0; masks < 64; ++masks)
          for (unsigned pc = 0; pc < 4; ++pc)
            for (unsigned top = 0; top < 8; ++top)
              for (unsigned empty = 0; empty < 3; ++empty) {
                unsigned cw = 0x40 | masks | (pc << 8) | (rc << 10);
                GamePkgExt80 value = edge[(masks + top + pc + empty) %
                                          (sizeof(edge) / sizeof(*edge))];
                if (masks % 3 == 0) {
                  uint64_t magnitude = masks % 2
                                           ? UINT64_C(999999999999999999)
                                           : UINT64_C(1000000000000000000);
                  magnitude += (top % 3);
                  value = gamepkg_x87_decode_integer(magnitude, 64);
                  if (top & 1)
                    value.sign_exponent |= 0x8000;
                }
                if (masks % 7 == 0)
                  value =
                      (GamePkgExt80){random64(&rng), (uint16_t)random64(&rng)};
                Bcd input = {{0}}, expected;
                for (unsigned i = 0; i < 9; ++i)
                  input.b[i] = (unsigned char)(((random64(&rng) % 10) << 4) |
                                               (random64(&rng) % 10));
                if (masks % 5 == 0)
                  memset(input.b, 0, 9);
                if (masks % 11 == 0)
                  memset(input.b, 0x99, 9);
                input.b[9] = (unsigned char)random64(&rng);
                expected = input;
                initial = original;
                GamePkgX87State state;
                prepare(&initial, &state, cw, top, empty, value);

#ifndef GAMEPKG_BCD_REPLAY
                __asm__ volatile("fxrstor %0" ::"m"(initial) : FX_CLOBBERS);
                if (op)
                  __asm__ volatile("fbstp %0" : "+m"(expected)::"memory");
                else
                  __asm__ volatile("fbld %0" ::"m"(input) : "memory");
                __asm__ volatile("fxsave %0" : "=m"(native)::"memory");
                struct {
                  unsigned char b[28];
                } environment;
                __asm__ volatile("fnstenv %0" : "=m"(environment)::"memory");
                put(native.b + 6, get16(environment.b + 8), 2);
                __asm__ volatile("fxrstor %0" ::"m"(original) : FX_CLOBBERS);
#endif
                corpus_record(&native, &expected);

                memset(ram, 0x5a, sizeof(ram));
                memcpy(ram + 1, input.b, 10);
                *gamepkg_x86_x87_state() = state;
                g_esi = 1025;
                g_edi = 0;
                g_ebx = 0;
                g_fs_base = 0;
                expected_opcode = op ? 0x730 : 0x720;
                expected_data_pointer = 1025;
                if (form < 6) {
                  expected_opcode = opcodes[op][form];
                  if (form == 1) {
                    g_fs_base = 0xfffffff0;
                    g_esi += 16;
                    expected_data_pointer = 1041;
                  }
                  if (form == 2) {
                    g_ebx = 0xffff;
                    g_esi = 0x10402;
                  }
                  if (form == 3) {
                    g_edi = 0x20000001;
                    g_esi -= 40;
                  }
                }
                assert(gamepkg_x86_run(form < 6 ? generated[op][form]
                                       : op     ? store
                                                : load,
                                       &fault) == 0);
                compare(&native, op, cw, top, empty, value);
                assert(!memcmp(ram + 1, op ? expected.b : input.b, 10) &&
                       ram[0] == 0x5a && ram[11] == 0x5a);
                if (gamepkg_x86_x87_state()->status & 0x80) {
                  GamePkgX87State before = *gamepkg_x86_x87_state();
                  assert(gamepkg_x86_run(wait_next, &fault) == 1 &&
                         fault.vector == 16 && fault.eip == 0x20000);
                  assert(!memcmp(&before, gamepkg_x86_x87_state(),
                                 sizeof(before)));
                }
                ++checks;
              }
  /* Fault priority, no partial writes/pops, and old-pending delivery before
   * RAM permission checks. Both BCD operations use the same checked boundary.
   */
  for (unsigned op = 0; op < 2; ++op)
    for (unsigned scenario = 0; scenario < 3; ++scenario) {
      initial = original;
      GamePkgX87State state;
      prepare(&initial, &state, 0x37f, 3, 0, edge[4]);
      *gamepkg_x86_x87_state() = state;
      bind(scenario == 0 ? 10 : 16, scenario == 1 ? 1 : 3);
      unsigned char before[16];
      memcpy(before, ram, sizeof(ram));
      if (scenario == 2) {
        state.control = 0x37e;
        state.status |= 0x8081;
        *gamepkg_x86_x87_state() = state;
      }
      if (!op && scenario == 1)
        continue;
      assert(gamepkg_x86_run(op ? store : load, &fault) == 1 &&
             fault.vector == (scenario == 2 ? 16u : 14u));
      assert(!memcmp(before, ram, sizeof(ram)) &&
             !memcmp(&state, gamepkg_x86_x87_state(), sizeof(state)));
    }
  /* A full BCD operand may not cross the flat segment limit. Check it
   * before the first staged read, while old pending exceptions win first. */
  bind(sizeof(ram), 3);
  for (unsigned op = 0; op < 2; ++op)
    for (unsigned pending = 0; pending < 2; ++pending) {
      initial = original;
      GamePkgX87State state;
      prepare(&initial, &state, 0x37f, 2, 0, edge[4]);
      if (pending) {
        state.control = 0x37e;
        state.status |= 0x8081;
      }
      *gamepkg_x86_x87_state() = state;
      g_esi = 0xfffffff7;
      g_fs_base = 0;
      reads = 0;
      unsigned char before[16];
      memcpy(before, ram, sizeof(ram));
      assert(gamepkg_x86_run(generated[op][0], &fault) == 1 &&
             fault.vector == (pending ? 16u : 13u));
      assert(reads == 0 &&
             !memcmp(&state, gamepkg_x86_x87_state(), sizeof(state)) &&
             !memcmp(before, ram, sizeof(ram)));
    }
  bind(sizeof(ram), 3);
  for (unsigned part = 4; part <= 8; part += 4) {
    initial = original;
    GamePkgX87State state;
    prepare(&initial, &state, 0x37f, 2, 2, edge[4]);
    *gamepkg_x86_x87_state() = state;
    g_esi = 1025;
    g_fs_base = 0;
    reads = 0;
    bind(1 + part, 3);
    assert(gamepkg_x86_run(generated[0][0], &fault) == 1 &&
           fault.vector == 14 && reads == part / 4 + 1);
    assert(!memcmp(&state, gamepkg_x86_x87_state(), sizeof(state)));
  }
  for (unsigned empty = 0; empty < 2; ++empty) {
    initial = original;
    GamePkgX87State state;
    prepare(&initial, &state, 0x40, 2, empty, edge[12]);
    *gamepkg_x86_x87_state() = state;
    bind(16, 1);
    unsigned char before[16];
    memcpy(before, ram, sizeof(ram));
    assert(gamepkg_x86_run(store, &fault) == 1 && fault.vector == 14);
    assert(!memcmp(&state, gamepkg_x86_x87_state(), sizeof(state)) &&
           !memcmp(before, ram, sizeof(ram)));
  }
  if (corpus) {
#ifdef GAMEPKG_BCD_REPLAY
    assert(fgetc(corpus) == EOF && !ferror(corpus));
#endif
    assert(!fclose(corpus));
  }
  gamepkg_x86_memory_bind(NULL);
  printf("BCD x87 qualification: %lu full-state cases and fault-priority "
         "checks passed\n",
         checks);
  return 0;
}
