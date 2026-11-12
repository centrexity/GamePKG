/* Synthetic protected-mode legacy FPU images. Never contains game assets. */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_x87.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#include <string.h>
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static void (*const generated[10])(void);
static void (*const physical[10])(unsigned char *);
static _Thread_local unsigned char ram[512];
static uint64_t rng = 0xcba9876543210123ULL;
static uint64_t random64(void) {
  rng ^= rng << 13;
  rng ^= rng >> 7;
  rng ^= rng << 17;
  return rng;
}
static unsigned rd(const unsigned char *p) {
  return p[0] | ((unsigned)p[1] << 8);
}
static void wr(unsigned char *p, unsigned v) {
  p[0] = (unsigned char)v;
  p[1] = (unsigned char)(v >> 8);
}
static unsigned classify(uint64_t v, unsigned e) {
  e &= 0x7fff;
  return !e && !v ? 1 : e && e != 0x7fff && (v >> 63) ? 0 : 2;
}
static void image(const GamePkgX87State *s, unsigned char *p, unsigned small) {
  memset(p, 0, 108);
  unsigned step = small ? 2 : 4, offset = small ? 14 : 28;
  wr(p, s->control);
  wr(p + step, s->status);
  wr(p + 2 * step, s->tags);
  if (!small) {
    wr(p + 2, 65535);
    wr(p + 6, 65535);
    wr(p + 10, 65535);
    wr(p + 26, 65535);
  }
  unsigned top = (s->status >> 11) & 7;
  for (unsigned i = 0; i < 8; ++i) {
    unsigned n = (top + i) & 7;
    for (unsigned j = 0; j < 8; ++j)
      p[offset + 10 * i + j] = (unsigned char)(s->significand[n] >> (j * 8));
    wr(p + offset + 10 * i + 8, s->sign_exponent[n]);
  }
}
static void capture(unsigned char *p) {
  __asm__ volatile("fnsave (%0)" ::"r"(p) : "memory");
}
static void restore(const unsigned char *p) {
  __asm__ volatile("frstor (%0)" ::"r"(p) : "memory");
}
static unsigned cases, queued;
static void next_wait(void) { gamepkg_x86_x87_wait(0x20000, 0x1234); }
static FILE *corpus;
static void word(unsigned v) {
  unsigned char p[4];
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(v >> (8 * i));
  assert(fwrite(p, 1, 4, corpus) == 4);
}
static void one(unsigned form, GamePkgX87State before,
                unsigned char input[108]) {
  unsigned op = form % 5, small = form / 5, env = small ? 14 : 28,
           size = op == 0                ? 2
                  : (op == 2 || op == 4) ? env + 80
                                         : env;
  unsigned char init[108], hw[108], state[108];
  image(&before, init, 0);
  memcpy(hw, input, 108);
  restore(init);
  physical[form](hw);
  capture(state);
  memcpy(ram, input, 108);
  g_esi = 1024;
  *gamepkg_x86_x87_state() = before;
  GamePkgX86Fault f;
  assert(!gamepkg_x86_run(generated[form], &f));
  if (op >= 3) {
    unsigned end = small ? 6 : 12;
    for (unsigned i = 0; i < end; ++i)
      if (ram[i] != hw[i]) {
        fprintf(stderr, "save form%u case%u byte%u got%x expected%x\n", form,
                cases, i, ram[i], hw[i]);
        abort();
      }
    if (op == 4)
      assert(!memcmp(ram + env, hw + env, 80));
  } else
    assert(!memcmp(ram, input, 108));
  assert(!memcmp(ram + size, input + size, 108 - size));
  GamePkgX87State after = *gamepkg_x86_x87_state();
  int pending = (after.status & ~after.control & 63u) != 0;
  int delivered = gamepkg_x86_run(next_wait, &f);
  assert(delivered == pending &&
         (!pending || (f.vector == 16 && f.eip == 0x20000)));
  assert(!memcmp(&after, gamepkg_x86_x87_state(), sizeof(after)));
  queued += (unsigned)pending;
  gamepkg_x86_x87_environment(4, 0, 1280, 0x20000, 0x1234);
  /* Modern hardware deprecates legacy selectors/FOP/FDP. Compare numerical
   * state here; explicit legacy-layout tests below check those fields. */
  for (unsigned i = 0; i < 12; ++i)
    if (ram[256 + i] != state[i]) {
      fprintf(stderr, "state form%u case%u byte%u got%x expected%x CW%x SW%x\n",
              form, cases, i, ram[256 + i], state[i], before.control,
              before.status);
      abort();
    }
  if (memcmp(ram + 284, state + 28, 80)) {
    fprintf(stderr, "registers form%u case%u\n", form, cases);
    abort();
  }
  if (corpus) {
    word(form);
    assert(fwrite(init, 1, 108, corpus) == 108);
    assert(fwrite(input, 1, 108, corpus) == 108);
    assert(fwrite(hw, 1, 108, corpus) == 108);
    assert(fwrite(state, 1, 108, corpus) == 108);
  }
  ++cases;
}
static void hardware(void) {
  GamePkgX87State s = {0};
  s.tags = 0xffff;
  s.control = 0x37f;
  unsigned char p[108];
  memset(p, 0xa5, sizeof(p));
  for (unsigned cw = 0; cw < 65536; ++cw) {
    wr(p, cw);
    one(0, s, p);
    one(5, s, p);
  }
  for (unsigned k = 0; k < 32768; ++k) {
    s.control = (unsigned short)((random64() & 0x1f00) | 0x7f);
    s.status = (unsigned short)(random64() & 0x7f3f);
    s.tags = 0;
    for (unsigned i = 0; i < 8; ++i) {
      s.significand[i] = random64();
      s.sign_exponent[i] = (unsigned short)random64();
      unsigned tag = (random64() & 3) == 3
                         ? 3
                         : classify(s.significand[i], s.sign_exponent[i]);
      s.tags |= (unsigned short)(tag << (2 * i));
    }
    unsigned small = k & 1;
    image(&s, p, small);
    if (k & 2) {
      wr(p, (unsigned)random64());
      wr(p + (small ? 2 : 4), (unsigned)random64());
      wr(p + (small ? 4 : 8), (unsigned)random64());
    }
    for (unsigned op = 1; op < 5; ++op)
      one(5 * small + op, s, p);
  }
}
static void pending_controls(void) {
  GamePkgX87State s = {0};
  s.tags = 0xffff;
  unsigned char p[108];
  memset(p, 0, sizeof(p));
  for (unsigned small = 0; small < 2; ++small)
    for (unsigned flags = 0; flags < 64; ++flags)
      for (unsigned masks = 0; masks < 64; ++masks) {
        s.control = 0x37f;
        s.status = (unsigned short)flags;
        wr(p, 0x340 | masks);
        one(5 * small, s, p);
        s.control = (unsigned short)(0x340 | masks);
        s.status = (unsigned short)(flags | ((flags & ~masks) ? 0x8080 : 0));
        one(5 * small + 3, s, p);
        one(5 * small + 4, s, p);
      }
}
static void current(void) {
  gamepkg_x86_x87_environment(g_eax, g_ecx, g_esi, 0x10000, 0xabcdef12);
}
static void faults(void) {
  GamePkgX87State before = {0};
  before.control = 0x37f;
  before.tags = 0xffff;
  before.significand[7] = 0x12345678;
  GamePkgX86Fault f;
  for (unsigned op = 0; op < 5; ++op)
    for (unsigned small = 0; small < 2; ++small) {
      unsigned n =
          op == 0 ? 2 : (small ? 14 : 28) + ((op == 2 || op == 4) ? 80 : 0);
      g_eax = op;
      g_ecx = small;
      for (unsigned left = 0; left < n; ++left) {
        g_esi = 1536 - left;
        memset(ram, 0x5a, sizeof(ram));
        unsigned char old[512];
        memcpy(old, ram, 512);
        *gamepkg_x86_x87_state() = before;
        assert(gamepkg_x86_run(current, &f) == 1 && f.vector == 14 &&
               f.fault_address == 1536);
        assert(!memcmp(old, ram, 512));
        assert(!memcmp(&before, gamepkg_x86_x87_state(), sizeof(before)));
      }
      for (unsigned pending = 0; pending < 2; ++pending) {
        before.control = (unsigned short)(pending ? 0x37e : 0x37f);
        before.status = (unsigned short)(pending ? 0x8081 : 0);
        *gamepkg_x86_x87_state() = before;
        g_esi = UINT32_MAX - n + 2;
        assert(gamepkg_x86_run(generated[5 * small + op], &f) == 1 &&
               f.vector == ((pending && op <= 2) ? 16u : 13u) &&
               f.eip == 0x10000 && f.ebp == 0xabcdef12);
        assert(!memcmp(&before, gamepkg_x86_x87_state(), sizeof(before)));
      }
      before.control = 0x37e;
      before.status = 0x8081;
      *gamepkg_x86_x87_state() = before;
      g_esi = 1024;
      int r = gamepkg_x86_run(current, &f);
      if (op <= 2) {
        assert(r == 1 && f.vector == 16);
        assert(!memcmp(&before, gamepkg_x86_x87_state(), sizeof(before)));
      } else
        assert(!r);
      before.control = 0x37f;
      before.status = 0;
    }
}
static void legacy(void) {
  GamePkgX87State *s = gamepkg_x86_x87_state();
  gamepkg_x86_x87_init();
  gamepkg_x86_x87_bind_segments(0x1b, 0x23, 0x2b, 0x3b);
  for (unsigned kind = 0; kind < 3; ++kind) {
    gamepkg_x86_x87_data_segment(kind, kind == 2 ? 0xfffffff0u : 0);
    gamepkg_x86_x87_record(s, 0x12345678, 0x7ab);
    gamepkg_x86_x87_record_data(s, 1025);
    gamepkg_x86_x87_environment(3, 0, 1024, 0, 0);
    assert(rd(ram + 16) == 0x1b && rd(ram + 18) == 0x7ab &&
           rd(ram + 24) == (kind == 0   ? 0x23
                            : kind == 1 ? 0x2b
                                        : 0x3b));
    assert(rd(ram + 20) == (kind == 2 ? 1041 : 1025));
    gamepkg_x86_x87_environment(3, 1, 1024, 0, 0);
    assert(rd(ram + 6) == 0x5678 && rd(ram + 8) == 0x1b &&
           rd(ram + 12) == (kind == 0   ? 0x23
                            : kind == 1 ? 0x2b
                                        : 0x3b));
  }
  gamepkg_x86_x87_bind_segments(0, 0, 0, 0);
}

#ifdef _WIN32
static DWORD WINAPI worker(void *raw) {
#else
static void *worker(void *raw) {
#endif
  unsigned index = (unsigned)(uintptr_t)raw;
  GamePkgX87State *s = gamepkg_x86_x87_state();
  assert(s->control == 0x37f && s->tags == 0xffff && !s->instruction_selector &&
         !s->data_selector);
  GamePkgX86Region r = {1024, sizeof(ram), 3, ram};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&r, 1, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  gamepkg_x86_x87_bind_segments(0x10 + index, 0x20 + index, 0x30 + index,
                                0x40 + index);
  for (unsigned n = 0; n < 1000; ++n) {
    gamepkg_x86_x87_init();
    unsigned small = n & 1;
    gamepkg_x86_x87_data_segment(2, 0xfffffff0u);
    gamepkg_x86_x87_record(s, 0x12340000 + n, 0x321 + index);
    gamepkg_x86_x87_record_data(s, 1025);
    gamepkg_x86_x87_environment(3, small, 1024, 0, 0);
    assert(rd(ram + (small ? 8 : 16)) == 0x10 + index &&
           rd(ram + (small ? 12 : 24)) == 0x40 + index &&
           rd(ram + (small ? 10 : 20)) == 1041);
    if (!small)
      assert(rd(ram + 18) == 0x321 + index);
  }
  gamepkg_x86_memory_bind(0);
  gamepkg_x86_memory_release(m);
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
  GamePkgX86Region r = {1024, sizeof(ram), 3, ram};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&r, 1, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  const char *path = getenv("GAMEPKG_TAKEDAWN_X87_ENVIRONMENT_CORPUS");
  if (path) {
    corpus = fopen(path, "wb");
    assert(corpus);
    assert(fwrite("GPKXENV1", 1, 8, corpus) == 8);
    word(0);
  }
  hardware();
  pending_controls();
  if (corpus) {
    assert(fseek(corpus, 8, SEEK_SET) == 0);
    word(cases);
    assert(fclose(corpus) == 0);
  }
  faults();
  legacy();
  threads();
  gamepkg_x86_memory_bind(0);
  gamepkg_x86_memory_release(m);
  printf("x87 environment: %u hardware cases; bounded fault rollback, pending "
         "gates, legacy selectors and8000 thread bindings passed (%u deferred "
         "next-wait faults)\n",
         cases, queued);
  return 0;
}
