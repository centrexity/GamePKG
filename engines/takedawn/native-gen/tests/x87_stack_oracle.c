/* Synthetic stack/rounding oracle; raw80 values, no host FP casts or assets. */
#define _GNU_SOURCE 1
#include "gamepkg_x86_ext80.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_x87.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <ucontext.h>
#include <unistd.h>
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
static _Thread_local unsigned completed;
static void (*const generated[88])(void);
static uint32_t (*const physical[88])(uint32_t);
static const unsigned opcodes[88], operations[88], indices[88];
static void next_wait(void) {
  gamepkg_x86_x87_wait(0x20000, 0x9876);
  ++completed;
}
static uint64_t get64(const unsigned char *p) {
  uint64_t v = 0;
  for (unsigned i = 0; i < 8; ++i)
    v |= (uint64_t)p[i] << (8 * i);
  return v;
}
static unsigned get16(const unsigned char *p) {
  return p[0] | ((unsigned)p[1] << 8);
}
static void put64(unsigned char *p, uint64_t v) {
  for (unsigned i = 0; i < 8; ++i)
    p[i] = (unsigned char)(v >> (8 * i));
}
static uint64_t random64(uint64_t *v) {
  *v ^= *v << 13;
  *v ^= *v >> 7;
  *v ^= *v << 17;
  return *v;
}
static unsigned classify(GamePkgExt80 v) {
  unsigned e = v.sign_exponent & 0x7fff;
  return !e && !v.significand                        ? 1
         : e && e != 0x7fff && (v.significand >> 63) ? 0
                                                     : 2;
}
static const GamePkgExt80 edges[] = {{0, 0},
                                     {0, 0x8000},
                                     {UINT64_C(0x8000000000000000), 0x3fff},
                                     {UINT64_C(0x8000000000000000), 0xbfff},
                                     {UINT64_C(0xc000000000000000), 0x3fff},
                                     {UINT64_C(0xc000000000000000), 0xbfff},
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
                                     {1, 0xffff}};
static unsigned flags_pattern(unsigned pattern) {
  return 0x202u | (pattern & 1u) | ((pattern & 2u) << 1) |
         ((pattern & 4u) << 2) | ((pattern & 8u) << 3) |
         ((pattern & 16u) << 3) | ((pattern & 32u) << 6);
}
static void prepare(Image *image, GamePkgX87State *s, unsigned op, unsigned cw,
                    unsigned top, unsigned empties, GamePkgExt80 a,
                    GamePkgExt80 b, unsigned flags) {
  memset(s, 0, sizeof(*s));
  s->control = (uint16_t)cw;
  s->status =
      (uint16_t)((top << 11) | ((flags & 1) << 8) | ((flags & 4) << 8) |
                 ((flags & 0x40) << 8) | ((flags & 0x10u) << 5) |
                 ((cw & 63) & (op + top + empties)) | ((flags & 0x800u) >> 5));
  s->instruction_pointer = 0x40000;
  s->data_pointer = 0x50000;
  s->opcode = 0x123;
  image->b[0] = (unsigned char)cw;
  image->b[1] = (unsigned char)(cw >> 8);
  image->b[2] = (unsigned char)s->status;
  image->b[3] = (unsigned char)(s->status >> 8);
  image->b[4] = 0;
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    GamePkgExt80 v = i == 0             ? a
                     : i == indices[op] ? b
                                        : edges[(op + top + i) % 24];
    unsigned tag = (empties & (1u << i)) ? 3 : classify(v);
    s->significand[p] = v.significand;
    s->sign_exponent[p] = v.sign_exponent;
    s->tags |= (uint16_t)(tag << (2 * p));
    if (tag != 3)
      image->b[4] |= (unsigned char)(1u << p);
    put64(image->b + 32 + 16 * i, v.significand);
    image->b[40 + 16 * i] = (unsigned char)v.sign_exponent;
    image->b[41 + 16 * i] = (unsigned char)(v.sign_exponent >> 8);
  }
  g_eax = 0x1234;
  g_ecx = 0x2345;
  g_edx = 0x3456;
  g_ebx = 0x4567;
  g_esp = 0x800;
  g_ebp = 0x777;
  g_esi = 0x5678;
  g_edi = 0x6789;
  g_eflags = flags;
  g_fs_base = 0x789a;
}
static void compare(const Image *after, unsigned op, unsigned cw,
                    GamePkgExt80 a, unsigned empties) {
  GamePkgX87State *s = gamepkg_x86_x87_state();
  unsigned sw = get16(after->b + 2), top = (sw >> 11) & 7, valid = 0,
           undefined = operations[op] == 3 || operations[op] == 7 ? 0x4700
                       : operations[op] == 6                      ? 0
                                                                  : 0x4500;
  if ((s->status & ~undefined) != (sw & ~undefined)) {
    fprintf(stderr,
            "stack form%u op%u idx%u cw%04x empty%02x input%04x:%016llx status "
            "actual%04x expected%04x\n",
            op, operations[op], indices[op], cw, empties, a.sign_exponent,
            (unsigned long long)a.significand, s->status, sw);
    abort();
  }
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    uint64_t sig = get64(after->b + 32 + 16 * i);
    unsigned se = get16(after->b + 40 + 16 * i);
    if (s->significand[p] != sig || s->sign_exponent[p] != se) {
      fprintf(stderr,
              "stack form%u op%u cw%04x empty%02x p%u actual%04x:%016llx "
              "expected%04x:%016llx\n",
              op, operations[op], cw, empties, p, s->sign_exponent[p],
              (unsigned long long)s->significand[p], se,
              (unsigned long long)sig);
      abort();
    }
    unsigned tag = (s->tags >> (2 * p)) & 3;
    if (tag != 3) {
      valid |= 1u << p;
      assert(tag == classify((GamePkgExt80){sig, (uint16_t)se}));
    }
  }
  assert(valid == after->b[4] && s->control == cw &&
         s->instruction_pointer == 0x10000 && s->opcode == opcodes[op] &&
         s->data_pointer == 0x50000);
}
static FILE *corpus;
static uint32_t corpus_rows;
static _Thread_local unsigned export_case;
static void word(uint32_t v) {
  unsigned char b[4];
  for (unsigned i = 0; i < 4; ++i)
    b[i] = (unsigned char)(v >> (8 * i));
  assert(fwrite(b, 1, 4, corpus) == 4);
}
static void state_words(const GamePkgX87State *state) {
  for (unsigned i = 0; i < 8; ++i) {
    word((uint32_t)state->significand[i]);
    word((uint32_t)(state->significand[i] >> 32));
    word(state->sign_exponent[i]);
  }
  word(state->control);
  word(state->status);
  word(state->tags);
  word(state->instruction_pointer);
  word(state->data_pointer);
  word(state->opcode);
}
static void record(unsigned form, unsigned flags,
                   const GamePkgX87State *initial) {
  if (!corpus || !export_case)
    return;
  word(operations[form]);
  word(indices[form]);
  word(flags);
  state_words(initial);
  state_words(gamepkg_x86_x87_state());
  ++corpus_rows;
}
static unsigned check(const Image *original, unsigned op, unsigned cw,
                      unsigned top, unsigned empties, GamePkgExt80 a,
                      GamePkgExt80 b, unsigned flags) {
  Image input = *original, after;
  GamePkgX87State initial;
  prepare(&input, &initial, op, cw, top, empties, a, b, flags);
  uint32_t before[] = {g_eax, g_ecx, g_edx, g_ebx,    g_esp,
                       g_ebp, g_esi, g_edi, g_eflags, g_fs_base};
  __asm__ volatile("fxrstor %0" ::"m"(input) : FX_CLOBBERS);
  uint32_t physical_flags = physical[op](flags);
  __asm__ volatile("fxsave %0" : "=m"(after)::"memory");
  __asm__ volatile("fxrstor %0" ::"m"(*original) : FX_CLOBBERS);
  assert((physical_flags & 0x8d5u) == (flags & 0x8d5u));
  *gamepkg_x86_x87_state() = initial;
  completed = 0;
  GamePkgX86Fault fault;
  assert(gamepkg_x86_run(generated[op], &fault) == 0 && completed == 1);
  uint32_t actual[] = {g_eax, g_ecx, g_edx, g_ebx,    g_esp,
                       g_ebp, g_esi, g_edi, g_eflags, g_fs_base};
  assert(memcmp(before, actual, sizeof(before)) == 0);
  compare(&after, op, cw, a, empties);
  record(op, flags, &initial);
  if (get16(after.b + 2) & ~cw & 63u) {
    GamePkgX87State before_state = *gamepkg_x86_x87_state();
    completed = 0;
    assert(gamepkg_x86_run(next_wait, &fault) == 1 && fault.vector == 16 &&
           fault.eip == 0x20000 && fault.ebp == 0x9876 && completed == 0 &&
           memcmp(&before_state, gamepkg_x86_x87_state(),
                  sizeof(before_state)) == 0);
    return 1;
  }
  return 0;
}
static void pending(const Image *original) {
  for (unsigned op = 0; op < 88; ++op) {
    Image input = *original;
    GamePkgX87State s;
    prepare(&input, &s, op, 0x37e, 3, 0, edges[2], edges[3], 0x8d7);
    s.status |= 0x8081;
    *gamepkg_x86_x87_state() = s;
    completed = 0;
    GamePkgX86Fault f;
    assert(gamepkg_x86_run(generated[op], &f) == 1 && f.vector == 16 &&
           f.eip == 0x10000 && f.ebp == 0xabcdef12 && completed == 0 &&
           memcmp(&s, gamepkg_x86_x87_state(), sizeof(s)) == 0 &&
           g_eflags == 0x8d7);
  }
}
#ifndef _WIN32
static Image fault_before;
static unsigned expected_vector;
static void fault_handler(int signal, siginfo_t *info, void *raw) {
  (void)info;
  ucontext_t *u = raw;
  const struct _libc_fpstate *fp = u->uc_mcontext.fpregs;
  if (!fp)
    _exit(90);
#ifdef __x86_64__
  unsigned sw = fp->swd, cw = fp->cwd, valid = fp->ftw & 255u;
  uint64_t ip = fp->rip;
#else
  unsigned sw = (unsigned)fp->sw & 65535u, cw = (unsigned)fp->cw & 65535u,
           valid = 0;
  for (unsigned p = 0; p < 8; ++p)
    if (((fp->tag >> (2 * p)) & 3) != 3)
      valid |= 1u << p;
  uint64_t ip = fp->ipoff;
#endif
  if ((unsigned)u->uc_mcontext.gregs[REG_TRAPNO] != expected_vector ||
      signal != (expected_vector == 14 ? SIGSEGV : SIGFPE))
    _exit(91);
  if (cw != get16(fault_before.b)) {
    _exit(94);
  }
  if (sw != get16(fault_before.b + 2)) {
    _exit(95);
  }
  if (valid != fault_before.b[4]) {
    _exit(96);
  }
  uint64_t before_ip = get64(fault_before.b + 8);
#ifndef __x86_64__
  before_ip = (uint32_t)before_ip;
#endif
  if (ip != before_ip) {
    char message[] = "load context FIP=0000000000000000\n";
    static const char digits[] = "0123456789abcdef";
    for (unsigned j = 0; j < 16; ++j)
      message[17 + j] = digits[(ip >> (60 - 4 * j)) & 15];
    if (write(2, message, sizeof(message) - 1) !=
        (ssize_t)sizeof(message) - 1) {
      _exit(98);
    }
    _exit(97);
  }
  for (unsigned i = 0; i < 8; ++i) {
    uint64_t sig = 0;
    for (unsigned j = 0; j < 4; ++j)
      sig |= (uint64_t)fp->_st[i].significand[j] << (16 * j);
    if (sig != get64(fault_before.b + 32 + 16 * i) ||
        fp->_st[i].exponent != get16(fault_before.b + 40 + 16 * i))
      _exit(92);
  }
  _exit(0);
}
static unsigned physical_contexts;
static void physical_faults(const Image *original) {
  for (unsigned form = 0; form < 88; ++form)
    for (unsigned scenario = 0; scenario < 8; ++scenario) {
      unsigned op = operations[form];
      if (scenario == 1 &&
          !(op == 0 || op == 1 || op == 2 || op == 8 || op == 9 || op >= 10))
        continue;
      if ((scenario == 2 || scenario == 3) && op != 8 && op != 9)
        continue;
      if (scenario == 4 && op != 9)
        continue;
      if (scenario == 5 && op != 8)
        continue;
      if (scenario == 6 && op != 9)
        continue;
      if (scenario == 7 && op != 2 && op < 10)
        continue;
      pid_t child = fork();
      assert(child >= 0);
      if (child == 0) {
        struct rlimit core = {0, 0};
        assert(setrlimit(RLIMIT_CORE, &core) == 0);
        fault_before = *original;
        GamePkgX87State initial;
        unsigned cw = scenario == 3   ? 0x37d
                      : scenario == 4 ? 0x37b
                      : scenario == 5 ? 0x35f
                                      : 0x37e;
        unsigned empties = scenario == 1              ? 1
                           : scenario == 7            ? 1u << indices[form]
                           : op == 9 && scenario != 6 ? 128
                                                      : 0;
        GamePkgExt80 a = scenario == 2   ? edges[17]
                         : scenario == 3 ? edges[8]
                         : scenario == 4 ? edges[0]
                         : scenario == 5 ? edges[4]
                                         : edges[2];
        prepare(&fault_before, &initial, form, cw, 3, empties, a, edges[3],
                flags_pattern(0));
        if (scenario == 0) {
          fault_before.b[2] |= 0x81;
          fault_before.b[3] |= 0x80;
        }
        expected_vector = 16;
        struct sigaction action = {0};
        action.sa_sigaction = fault_handler;
        action.sa_flags = SA_SIGINFO;
        sigemptyset(&action.sa_mask);
        assert(sigaction(SIGFPE, &action, 0) == 0);
        __asm__ volatile("fxrstor %0" ::"m"(fault_before) : FX_CLOBBERS);
        if (scenario) {
          physical[form](flags_pattern(0));
        }
#ifdef __x86_64__
        __asm__ volatile("fxsave64 %0" : "=m"(fault_before)::"memory");
#else
        __asm__ volatile("fxsave %0" : "=m"(fault_before)::"memory");
#endif
        if (!(get16(fault_before.b + 2) & ~cw & 63u))
          _exit(98);
        if (scenario)
          __asm__ volatile("fwait" ::: "memory");
        else
          physical[form](flags_pattern(0));
        _exit(99);
      }
      int status;
      assert(waitpid(child, &status, 0) == child);
      if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        fprintf(stderr, "stack physical MF form%u scenario%u status%x\n", form,
                scenario, status);
        abort();
      }
      ++physical_contexts;
    }
}
#else
static unsigned physical_contexts;
static void physical_faults(const Image *original) { (void)original; }
#endif
#ifdef _WIN32
static DWORD WINAPI worker(void *raw) {
#else
static void *worker(void *raw) {
#endif
  unsigned n = (unsigned)(uintptr_t)raw;
  GamePkgX87State *s = gamepkg_x86_x87_state();
  assert(s->control == 0x37f && s->tags == 0xffff && s->status == 0);
  Image original;
  __asm__ volatile("fxsave %0" : "=m"(original)::"memory");
  uint64_t rng = 0x12345678 + n;
  for (unsigned i = 0; i < 1000; ++i) {
    GamePkgExt80 a = {random64(&rng), (uint16_t)random64(&rng)},
                 b = {random64(&rng), (uint16_t)random64(&rng)};
    check(&original, (i + n) % 88,
          0x40 | ((i + n) & 63) | ((i & 3) << 8) | ((i & 3) << 10), i & 7,
          (unsigned)random64(&rng) & 255, a, b, flags_pattern(i & 63));
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
    assert(pthread_create(&h[i], 0, worker, (void *)(uintptr_t)i) == 0);
  for (unsigned i = 0; i < 8; ++i)
    assert(pthread_join(h[i], 0) == 0);
#endif
}
int main(void) {
  Image original;
  __asm__ volatile("fxsave %0" : "=m"(original)::"memory");
  const char *corpus_path = getenv("GAMEPKG_TAKEDAWN_X87_STACK_CORPUS");
  if (corpus_path) {
    corpus = fopen(corpus_path, "wb");
    assert(corpus && fwrite("GPKXSTK1", 1, 8, corpus) == 8);
    word(0);
  }
  uint64_t count = 0, queued = 0, rng = UINT64_C(0x123456789abcdef);
  for (unsigned op = 0; op < 88; ++op)
    for (unsigned masks = 0; masks < 64; ++masks)
      for (unsigned pc = 0; pc < 4; ++pc)
        for (unsigned rc = 0; rc < 4; ++rc)
          for (unsigned top = 0; top < 8; ++top)
            for (unsigned scenario = 0; scenario < 8; ++scenario) {
              export_case =
                  top == (masks & 7u) && scenario == ((masks >> 3) & 7u);
              unsigned src = indices[op],
                       empties = scenario == 0   ? 0
                                 : scenario == 1 ? 1
                                 : scenario == 2 ? 1u << src
                                 : scenario == 3 ? (1u << src) | 1
                                 : scenario == 4 ? 128
                                 : scenario == 5 ? 129
                                 : scenario == 6
                                     ? 255
                                     : (unsigned)random64(&rng) & 255;
              GamePkgExt80 a = edges[(op + masks + pc + rc + top) % 24],
                           b = edges[(op + masks + rc + scenario) % 24];
              if ((op + top + masks) % 7 == 0)
                a = (GamePkgExt80){random64(&rng), (uint16_t)random64(&rng)};
              queued += check(
                  &original, op, 0x40 | masks | (pc << 8) | (rc << 10), top,
                  empties, a, b,
                  flags_pattern((op + masks + pc + rc + top + scenario) & 63));
              ++count;
            }
  for (unsigned op = 6; op <= 7; ++op)
    for (unsigned e = 0; e < 67; ++e)
      for (unsigned n = 0; n < 8; ++n)
        for (unsigned sign = 0; sign < 2; ++sign)
          for (unsigned masks = 0; masks < 64; ++masks)
            for (unsigned rc = 0; rc < 4; ++rc) {
              export_case = ((e * 8 + n) % 7) == 0 || e < 4 || e >= 64;
              static const uint64_t sig[] = {UINT64_C(0x8000000000000000),
                                             UINT64_C(0x8000000000000001),
                                             UINT64_C(0xbfffffffffffffff),
                                             UINT64_C(0xc000000000000000),
                                             UINT64_C(0xc000000000000001),
                                             UINT64_C(0xfffffffffffffffe),
                                             UINT64_MAX,
                                             UINT64_C(0x7fffffffffffffff)};
              GamePkgExt80 a = {sig[n],
                                (uint16_t)((sign << 15) | (16383 - 2 + e))};
              queued +=
                  check(&original, op, 0x340 | masks | (rc << 10), e & 7,
                        op == 7 ? 128 : 0, a, edges[2], flags_pattern(e & 63));
              ++count;
            }
  for (unsigned form = 24; form < 88; ++form)
    for (unsigned pattern = 0; pattern < 64; ++pattern)
      for (unsigned masks = 0; masks < 4; ++masks)
        for (unsigned scenario = 0; scenario < 8; ++scenario) {
          unsigned src = indices[form],
                   empties = scenario == 0   ? 0
                             : scenario == 1 ? 1
                             : scenario == 2 ? 1u << src
                             : scenario == 3 ? (1u << src) | 1
                             : scenario == 4 ? 128
                             : scenario == 5 ? 129
                             : scenario == 6 ? 255
                                             : (pattern * 13u) & 255u;
          export_case = 1;
          queued += check(&original, form, 0x37c | masks, (pattern >> 3) & 7,
                          empties, edges[pattern % 24],
                          edges[(pattern + 17) % 24], flags_pattern(pattern));
          ++count;
        }
  export_case = 0;
  if (corpus) {
    assert(fseek(corpus, 8, SEEK_SET) == 0);
    word(corpus_rows);
    assert(fclose(corpus) == 0);
    corpus = 0;
  }
  pending(&original);
  physical_faults(&original);
  threads();
  printf("x87 stack: %llu hardware/generated cases, %llu next-wait faults, 88 "
         "pending gates, %u physical FPU contexts and 8000 worker cases\n",
         (unsigned long long)count, (unsigned long long)queued,
         physical_contexts);
  return 0;
}
