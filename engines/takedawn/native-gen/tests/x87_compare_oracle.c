/* Synthetic instruction oracle: no game code or assets. */
#define _GNU_SOURCE 1
/* FXRSTOR overwrites compiler-held XMM temporaries as well as x87 state. */
#ifndef GAMEPKG_TEST_FX_CLOBBERS
#ifdef __x86_64__
#define GAMEPKG_TEST_FX_CLOBBERS "memory","xmm0","xmm1","xmm2","xmm3","xmm4","xmm5","xmm6","xmm7","xmm8","xmm9","xmm10","xmm11","xmm12","xmm13","xmm14","xmm15"
#else
#define GAMEPKG_TEST_FX_CLOBBERS "memory","xmm0","xmm1","xmm2","xmm3","xmm4","xmm5","xmm6","xmm7"
#endif
#endif
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
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
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <ucontext.h>
#include <unistd.h>
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
_Thread_local uint_fast8_t gamepkg_softfloat_unbounded_inexact;
typedef struct {
  _Alignas(16) unsigned char b[512];
} Image;
static _Thread_local unsigned completed;
static _Thread_local unsigned char memory[16];
static void (*const generated[115])(void);
static uint32_t (*const physical[115])(const unsigned char *, uint32_t);
static const unsigned opcodes[115], widths[115], modes[115], kinds[115],
    sources[115], integer_flags[115];
static void wait_next(void) {
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
static unsigned classify(uint64_t v, unsigned e) {
  e &= 0x7fff;
  return !e && !v ? 1 : e && e != 0x7fff && (v >> 63) ? 0 : 2;
}
static const uint64_t sigs[] = {0,
                                0,
                                UINT64_C(0x8000000000000000),
                                UINT64_C(0x8000000000000001),
                                UINT64_MAX,
                                UINT64_MAX,
                                1,
                                UINT64_C(0x7fffffffffffffff),
                                UINT64_C(0x8000000000000000),
                                UINT64_C(0x8000000000000000),
                                UINT64_C(0xc000000000000007),
                                UINT64_C(0x8000000000000001),
                                UINT64_C(0xc000000000000017),
                                0,
                                UINT64_C(0x123456789),
                                UINT64_C(0x8000000000000000)};
static const unsigned exps[] = {0,      0x8000, 0x3fff, 0xbfff, 0x7ffe, 0xfffe,
                                0,      0,      1,      0x7fff, 0x7fff, 0x7fff,
                                0xffff, 1,      0xffff, 0};
static const uint64_t bits64[] = {0,
                                  1,
                                  UINT64_C(0x8000000000000000),
                                  UINT64_C(0x3ff0000000000000),
                                  UINT64_C(0xbff0000000000001),
                                  UINT64_C(0x7ff0000000000000),
                                  UINT64_C(0xfff0000000000000),
                                  UINT64_C(0x7ff0000000000001),
                                  UINT64_C(0x7ff8000000000001),
                                  UINT64_C(0xfff8000000000007),
                                  UINT64_C(0x000fffffffffffff),
                                  UINT64_C(0x0010000000000000),
                                  UINT64_MAX,
                                  UINT64_C(0x7fefffffffffffff),
                                  0x8000,
                                  0x7fff,
                                  0x80000000,
                                  0x7fffffff};
static const uint32_t bits32[] = {
    0,          1,          0x80000000, 0x3f800000, 0xbf800001, 0x7f800000,
    0xff800000, 0x7f800001, 0x7fc00001, 0xffc00007, 0x7fffff,   0x800000,
    UINT32_MAX, 0x7f7fffff, 0x8000,     0x7fff,     0x80000000, 0x7fffffff};
static void prepare(Image *image, GamePkgX87State *s, unsigned op, unsigned cw,
                    unsigned top, unsigned empty, unsigned left,
                    uint64_t bits) {
  memset(s, 0, sizeof(*s));
  s->control = (uint16_t)cw;
  s->status = (uint16_t)((top << 11) | ((left & 8) << 6) | ((left & 1) << 8) |
                         ((left & 2) << 9) | ((left & 4) << 12));
  /* Old masked exceptions and SF are sticky; they must not affect the new
   * exception priority or be cleared by an otherwise successful comparison. */
  s->status |=
      (uint16_t)(((cw & 63u) & ((op + left + top + empty * 7u) & 63u)) |
                 ((op & 1u) << 6));
  s->instruction_pointer = 0x40000;
  s->data_pointer = 0x50000;
  s->opcode = 0x123;
  image->b[0] = (unsigned char)cw;
  image->b[1] = (unsigned char)(cw >> 8);
  image->b[2] = (unsigned char)s->status;
  image->b[3] = (unsigned char)(s->status >> 8);
  image->b[4] = 0;
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7, which = (left + i) % 16, e = exps[which];
    if (kinds[op] == 0 && i == sources[op]) {
      which = (unsigned)bits & 15;
      e = exps[which];
    }
    uint64_t v = sigs[which];
    s->significand[p] = v;
    s->sign_exponent[p] = (uint16_t)e;
    unsigned tag = (((empty & 1) && i == 0) ||
                    ((empty & 2) && kinds[op] == 0 && i == sources[op]))
                       ? 3
                       : classify(v, e);
    s->tags |= (uint16_t)(tag << (2 * p));
    if (tag != 3)
      image->b[4] |= (unsigned char)(1u << p);
    put64(image->b + 32 + 16 * i, v);
    image->b[40 + 16 * i] = (unsigned char)e;
    image->b[41 + 16 * i] = (unsigned char)(e >> 8);
  }
  put64(memory, bits);
  g_eax = 0x1234;
  g_ecx = 0x2345;
  g_edx = 0x3456;
  g_esi = 1025;
  g_edi = 0;
  g_ebx = 0;
  g_fs_base = 0;
  g_ebp = 0x777;
  g_esp = 0x800;
  g_eflags = 0xad7;
  if (modes[op] == 1) {
    g_fs_base = 0xfffffff0;
    g_esi += 16;
  }
  if (modes[op] == 2) {
    g_ebx = 0xffff;
    g_esi = 0x10402;
  }
  if (modes[op] == 3) {
    g_edi = 0x20000001;
    g_esi -= 40;
  }
}
static void compare(const Image *after, unsigned op, unsigned cw,
                    unsigned left) {
  GamePkgX87State *s = gamepkg_x86_x87_state();
  unsigned sw = get16(after->b + 2), top = (sw >> 11) & 7, valid = 0;
  if (s->status != sw) {
    fprintf(stderr,
            "memory op%u kind%u cw%04x left%u bits%016llx status actual%04x "
            "expected%04x\n",
            op, kinds[op], cw, left, (unsigned long long)get64(memory),
            s->status, sw);
    abort();
  }
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    uint64_t v = get64(after->b + 32 + 16 * i);
    unsigned e = get16(after->b + 40 + 16 * i);
    if (s->significand[p] != v || s->sign_exponent[p] != e) {
      fprintf(stderr,
              "memory op%u cw%04x left%u p%u bits%016llx value "
              "actual%04x:%016llx expected%04x:%016llx\n",
              op, cw, left, p, (unsigned long long)get64(memory),
              s->sign_exponent[p], (unsigned long long)s->significand[p], e,
              (unsigned long long)v);
      abort();
    }
    unsigned tag = (s->tags >> (2 * p)) & 3;
    if (tag != 3) {
      valid |= 1u << p;
      assert(tag == classify(v, e));
    }
  }
  assert(valid == after->b[4] && s->control == cw &&
         s->instruction_pointer == 0x10000 && s->opcode == opcodes[op] &&
         s->data_pointer == (kinds[op] >= 2 ? (modes[op]==1?1041u:1025u) : 0x50000u));
}
static _Thread_local uint32_t forced_flags;
static FILE *corpus;
static uint32_t corpus_rows;
static void word(uint32_t value) {
  unsigned char bytes[4];
  for (unsigned i = 0; i < 4; ++i)
    bytes[i] = (unsigned char)(value >> (8 * i));
  assert(fwrite(bytes, 1, 4, corpus) == 4);
}
static void record(unsigned op, unsigned top, unsigned empty, uint64_t bits,
                   uint32_t flags, const GamePkgX87State *initial) {
  if (!corpus || !((top == (initial->control & 7u) &&
                    empty == ((initial->control >> 3) & 3u)) ||
                   (forced_flags && top == 3u)))
    return;
  unsigned src = (top + sources[op]) & 7, first = opcodes[op] >> 8,
           second = opcodes[op] & 255, pop;
  if (kinds[op] >= 2)
    pop = ((second >> 3) & 7) == 3;
  else if (op == 112 || op == 113)
    pop = 2;
  else
    pop = (first == 0 && (second & 8)) || (first == 5 && (second & 8)) ||
          first == 7;
  unsigned quiet =
      (first == 5 && second >= 0xe0) || (first == 2 && second == 0xe9) ||
      ((first == 3 || first == 7) && second >= 0xe8 && second < 0xf0);
  GamePkgX87State *after = gamepkg_x86_x87_state();
  word(kinds[op]);
  word(sources[op]);
  word(pop);
  word(quiet);
  word(integer_flags[op]);
  word(flags);
  word((uint32_t)bits);
  word((uint32_t)(bits >> 32));
  word(initial->control);
  word(initial->status);
  word(initial->tags);
  word((uint32_t)initial->significand[top]);
  word((uint32_t)(initial->significand[top] >> 32));
  word(initial->sign_exponent[top]);
  word((uint32_t)initial->significand[src]);
  word((uint32_t)(initial->significand[src] >> 32));
  word(initial->sign_exponent[src]);
  word(after->status);
  word(after->tags);
  word(g_eflags);
  ++corpus_rows;
}
static unsigned check(const Image *original, unsigned op, unsigned cw,
                      unsigned top, unsigned empty, unsigned left,
                      uint64_t bits) {
  Image input = *original, after;
  GamePkgX87State initial;
  prepare(&input, &initial, op, cw, top, empty, left, bits);
  unsigned pattern = (op + cw + top + empty + left) & 63u;
  uint32_t flags = forced_flags
                       ? forced_flags
                       : 0x202u | (pattern & 1u) | ((pattern & 2u) << 1) |
                             ((pattern & 4u) << 2) | ((pattern & 8u) << 3) |
                             ((pattern & 16u) << 3) | ((pattern & 32u) << 6);
  g_eflags = flags;
  __asm__ volatile("fxrstor %0" ::"m"(input) : GAMEPKG_TEST_FX_CLOBBERS);
  uint32_t physical_flags = physical[op](memory, flags);
  __asm__ volatile("fxsave %0" : "=m"(after)::"memory");
  __asm__ volatile("fxrstor %0" ::"m"(*original) : GAMEPKG_TEST_FX_CLOBBERS);
  *gamepkg_x86_x87_state() = initial;
  completed = 0;
  GamePkgX86Fault fault;
  assert(gamepkg_x86_run(generated[op], &fault) == 0 && completed == 1 &&
         g_eax == 0x1234 && g_ecx == 0x2345 && g_edx == 0x3456 &&
         (g_eflags & ~0x8d5u) == (flags & ~0x8d5u) && g_esp == 0x800 &&
         g_ebp == 0x777);
  compare(&after, op, cw, left);
  if (!integer_flags[op])
    assert(g_eflags == flags);
  if ((g_eflags & 0x8d5u) != (physical_flags & 0x8d5u)) {
    fprintf(stderr,
            "compare op%u kind%u cw%04x left%u flags actual%x expected%x\n", op,
            kinds[op], cw, left, g_eflags, physical_flags);
    abort();
  }
  record(op, top, empty, bits, flags, &initial);
  if (get16(after.b + 2) & ~cw & 63u) {
    GamePkgX87State before = *gamepkg_x86_x87_state();
    completed = 0;
    assert(gamepkg_x86_run(wait_next, &fault) == 1 && fault.vector == 16 &&
           fault.eip == 0x20000 && fault.ebp == 0x9876 && completed == 0 &&
           memcmp(&before, gamepkg_x86_x87_state(), sizeof(before)) == 0);
    return 1;
  }
  return 0;
}
static GamePkgX86Memory *bind_memory(unsigned length, unsigned permission) {
  GamePkgX86Region region = {1025, length, permission, memory};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&region, 1, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  return m;
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
static unsigned compare_fault_contexts;
static void comparison_faults(const Image *original) {
  for (unsigned op = 0; op < 115; ++op)
    for (unsigned scenario = 0; scenario < 3; ++scenario) {
      pid_t child = fork();
      assert(child >= 0);
      if (child == 0) {
        struct rlimit core = {0, 0};
        assert(setrlimit(RLIMIT_CORE, &core) == 0);
        fault_before = *original;
        GamePkgX87State initial;
        unsigned top = 3;
        uint64_t bits = kinds[op] == 3   ? UINT64_C(0x3ff0000000000000)
                        : kinds[op] == 2 ? 0x3f800000
                                         : 1;
        prepare(&fault_before, &initial, op, 0x37c, top, scenario == 0, 2,
                bits);
        if (scenario) {
          put64(fault_before.b + 32,
                scenario == 1 ? UINT64_C(0x8000000000000001) : 1);
          fault_before.b[40] = scenario == 1 ? 0xff : 0;
          fault_before.b[41] = scenario == 1 ? 0x7f : 0;
        }
        struct sigaction action = {0};
        action.sa_sigaction = fault_handler;
        action.sa_flags = SA_SIGINFO;
        sigemptyset(&action.sa_mask);
        assert(sigaction(SIGFPE, &action, 0) == 0);
        expected_vector = 16;
        __asm__ volatile("fxrstor %0" ::"m"(fault_before) : GAMEPKG_TEST_FX_CLOBBERS);
        physical[op](memory, 0x202);
#ifdef __x86_64__
        __asm__ volatile("fxsave64 %0" : "=m"(fault_before)::"memory");
#else
        __asm__ volatile("fxsave %0" : "=m"(fault_before)::"memory");
#endif
        if (!(get16(fault_before.b + 2) & ~0x37cu & 3u))
          _exit(98);
        __asm__ volatile("fwait" ::: "memory");
        _exit(99);
      }
      int status;
      assert(waitpid(child, &status, 0) == child);
      if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        fprintf(stderr, "comparison MF op%u scenario%u status%x\n", op,
                scenario, status);
        abort();
      }
      ++compare_fault_contexts;
    }
}
static unsigned physical_faults;
static void fault_priority(const Image *original) {
  for (unsigned op = 0; op < 48; op += 6)
    for (unsigned scenario = 0; scenario < 3; ++scenario) {
      pid_t child = fork();
      assert(child >= 0);
      if (child == 0) {
        struct rlimit core = {0, 0};
        assert(setrlimit(RLIMIT_CORE, &core) == 0);
        long page = sysconf(_SC_PAGESIZE);
        assert(page > 16);
        unsigned char *map = mmap(0, (size_t)page * 2, PROT_READ | PROT_WRITE,
                                  MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        assert(map != MAP_FAILED &&
               mprotect(map + page, (size_t)page, PROT_NONE) == 0);
        unsigned part = widths[op] / 2;
        unsigned char *ptr = map + page - part;
        memset(ptr, 0xff, part);
        fault_before = *original;
        GamePkgX87State initial;
        prepare(&fault_before, &initial, op, scenario ? 0x40 : 0x37f, 2,
                scenario == 1, 11, 1);
        if (scenario == 2) {
          fault_before.b[2] = 0x81;
          fault_before.b[3] |= 0x80;
        }
        expected_vector = scenario == 2 ? 16 : 14;
        struct sigaction action = {0};
        action.sa_sigaction = fault_handler;
        action.sa_flags = SA_SIGINFO;
        sigemptyset(&action.sa_mask);
        assert(sigaction(SIGSEGV, &action, 0) == 0 &&
               sigaction(SIGFPE, &action, 0) == 0);
        __asm__ volatile("fxrstor %0" ::"m"(fault_before) : GAMEPKG_TEST_FX_CLOBBERS);
        if (scenario < 2)
          __asm__ volatile("fnop" ::: "memory");
#ifdef __x86_64__
        __asm__ volatile("fxsave64 %0" : "=m"(fault_before)::"memory");
#else
        __asm__ volatile("fxsave %0" : "=m"(fault_before)::"memory");
#endif
        physical[op](ptr, 0xad7);
        _exit(99);
      }
      int status;
      assert(waitpid(child, &status, 0) == child);
      if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        fprintf(stderr, "compare physical fault op%u scenario%u status%x\n", op,
                scenario, status);
        abort();
      }
      ++physical_faults;
    }
}
#else
static unsigned compare_fault_contexts;
static void comparison_faults(const Image *original) { (void)original; }
static unsigned physical_faults;
static void fault_priority(const Image *original) { (void)original; }
#endif
static unsigned faults(const Image *original) {
  unsigned count = 0;
  for (unsigned op = 0; op < 48; ++op)
    for (unsigned scenario = 0; scenario < 4; ++scenario) {
      GamePkgX86Memory *m = bind_memory(widths[op] - 1, 1);
      Image input = *original;
      GamePkgX87State s;
      prepare(&input, &s, op, 0x40, 3, scenario & 1, 11, 1);
      if (scenario >= 2)
        s.status |= 0x8081;
      *gamepkg_x86_x87_state() = s;
      completed = 0;
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(generated[op], &fault) == 1 && completed == 0 &&
             memcmp(&s, gamepkg_x86_x87_state(), sizeof(s)) == 0 &&
             fault.eip == 0x10000 &&
             fault.ebp == (modes[op] == 4 ? 1025 : 0xabcdef12));
      assert(fault.vector == (scenario >= 2 ? 16u : 14u));
      if (scenario < 2)
        assert(fault.fault_address == 1025 + widths[op] - 1 &&
               fault.error_code == 4);
      gamepkg_x86_memory_bind(0);
      gamepkg_x86_memory_release(m);
      ++count;
    }
  return count;
}
static unsigned pending_cases(const Image *original) {
  for (unsigned op = 0; op < 115; ++op) {
    Image input = *original;
    GamePkgX87State initial;
    prepare(&input, &initial, op, 0x37e, 3, 0, 2, 0);
    initial.status |= 0x8081;
    *gamepkg_x86_x87_state() = initial;
    completed = 0;
    GamePkgX86Fault fault;
    assert(gamepkg_x86_run(generated[op], &fault) == 1 && fault.vector == 16 &&
           fault.eip == 0x10000 &&
           fault.ebp == (modes[op] == 4 ? 1025u : 0xabcdef12u) &&
           completed == 0 && g_eflags == 0xad7 &&
           memcmp(&initial, gamepkg_x86_x87_state(), sizeof(initial)) == 0);
  }
  return 115;
}
static unsigned address_faults(const Image *original) {
  unsigned count = 0;
  for (unsigned op = 0; op < 48; op += 6)
    for (unsigned pending = 0; pending < 2; ++pending) {
      Image input = *original;
      GamePkgX87State initial;
      prepare(&input, &initial, op, 0x40, 2, 1, 11, 1);
      if (pending)
        initial.status |= 0x8081;
      *gamepkg_x86_x87_state() = initial;
      g_esi = UINT32_MAX - widths[op] + 2;
      completed = 0;
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(generated[op], &fault) == 1 &&
             fault.vector == (pending ? 16u : 13u) && fault.eip == 0x10000 &&
             fault.ebp == 0xabcdef12 && completed == 0 &&
             memcmp(&initial, gamepkg_x86_x87_state(), sizeof(initial)) == 0);
      ++count;
    }
  return count;
}
#ifdef _WIN32
static DWORD WINAPI worker(void *raw) {
#else
static void *worker(void *raw) {
#endif
  unsigned n = (unsigned)(uintptr_t)raw;
  GamePkgX87State *s = gamepkg_x86_x87_state();
  assert(s->control == 0x37f && s->tags == 0xffff && s->status == 0);
  GamePkgX86Memory *m = bind_memory(16, 1);
  Image original;
  __asm__ volatile("fxsave %0" : "=m"(original)::"memory");
  uint64_t rng = 0x12345678 + n;
  static const unsigned pc[] = {0, 2, 3};
  for (unsigned i = 0; i < 1000; ++i) {
    unsigned op = (i + n) % 115;
    check(&original, op,
          0x40 | ((i + n) & 63) | (pc[i % 3] << 8) | ((i & 3) << 10), i & 7,
          i & 1, i % 16, random64(&rng));
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
    assert(pthread_create(&h[i], 0, worker, (void *)(uintptr_t)i) == 0);
  for (unsigned i = 0; i < 8; ++i)
    assert(pthread_join(h[i], 0) == 0);
#endif
}
int main(void) {
  Image original;
  __asm__ volatile("fxsave %0" : "=m"(original)::"memory");
  GamePkgX86Memory *m = bind_memory(16, 1);
  const char *corpus_path = getenv("GAMEPKG_TAKEDAWN_X87_COMPARE_CORPUS");
  if (corpus_path) {
    corpus = fopen(corpus_path, "wb");
    assert(corpus && fwrite("GPKXCMP1", 1, 8, corpus) == 8);
    word(0);
  }
  uint64_t count = 0, queued = 0, rng = UINT64_C(0x123456789abcdef);
  for (unsigned op = 0; op < 115; ++op)
    for (unsigned masks = 0; masks < 64; ++masks)
      for (unsigned pc = 0; pc < 4; ++pc) {
        for (unsigned rc = 0; rc < 4; ++rc)
          for (unsigned top = 0; top < 8; ++top)
            for (unsigned empty = 0; empty < 4; ++empty) {
              unsigned cw = 0x40 | masks | (pc << 8) | (rc << 10),
                       left = (op + masks + top + rc) % 16,
                       idx = (op + masks + pc + rc + top) % 18;
              uint64_t bits = kinds[op] == 3 ? bits64[idx] : bits32[idx];
              if ((op + top + masks) % 7 == 0)
                bits = random64(&rng);
              queued += check(&original, op, cw, top, empty, left, bits);
              ++count;
            }
      }
  for (unsigned op = 0; op < 115; ++op) {
    if (op < 48 && op % 6)
      continue;
    for (unsigned left = 0; left < 16; ++left)
      for (unsigned right = 0; right < 18; ++right)
        for (unsigned masks = 0; masks < 64; ++masks) {
          uint64_t bits = kinds[op] == 0   ? right
                          : kinds[op] == 3 ? bits64[right]
                                           : bits32[right];
          queued +=
              check(&original, op, 0x340 | masks, left & 7, 0, left, bits);
          ++count;
        }
  }
  for (unsigned op = 80; op < 112; ++op)
    for (unsigned pattern = 0; pattern < 64; ++pattern) {
      forced_flags = 0x202u | (pattern & 1u) | ((pattern & 2u) << 1) |
                     ((pattern & 4u) << 2) | ((pattern & 8u) << 3) |
                     ((pattern & 16u) << 3) | ((pattern & 32u) << 6);
      for (unsigned left = 0; left < 16; ++left)
        for (unsigned right = 0; right < 16; ++right)
          for (unsigned mask = 0; mask < 4; ++mask) {
            queued +=
                check(&original, op, 0x37c | mask, left & 7, 0, left, right);
            ++count;
          }
    }
  forced_flags = 0;
  gamepkg_x86_memory_bind(0);
  gamepkg_x86_memory_release(m);
  unsigned failure_cases =
      faults(&original) + address_faults(&original) + pending_cases(&original);
  if (corpus) {
    assert(fseek(corpus, 8, SEEK_SET) == 0);
    word(corpus_rows);
    assert(fclose(corpus) == 0);
    corpus = 0;
  }
  fault_priority(&original);
  comparison_faults(&original);
  threads();
  printf("x87 compare: %llu hardware/generated cases, %llu queued faults, %u "
         "admission/priority cases, 8000 threaded cases, %u physical fault "
         "contexts\n",
         (unsigned long long)count, (unsigned long long)queued, failure_cases,
         physical_faults + compare_fault_contexts);
  return 0;
}
