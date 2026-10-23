/* Synthetic store/rounding oracle. No game code, disc, assets or host FP casts.
 */
#define _GNU_SOURCE 1
#include "gamepkg_x86_ext80.h"
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
/* FXRSTOR overwrites host XMM registers too. Cached byte snapshots may be
 * vectorized, so every affected compiler register must be declared clobbered.
 */
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
static _Thread_local unsigned char memory[16];
static void (*const generated[76])(void);
static void (*const physical[76])(unsigned char *);
static const unsigned opcodes[76], widths[76], modes[76], kinds[76],
    destinations[76];
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
static void prepare(Image *image, GamePkgX87State *s, unsigned op, unsigned cw,
                    unsigned top, unsigned empty, GamePkgExt80 value) {
  memset(s, 0, sizeof(*s));
  s->control = (uint16_t)cw;
  s->status =
      (uint16_t)((top << 11) | 0x4700 | ((cw & 63) & (op + top + empty)));
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
    GamePkgExt80 v = i == 0 ? value : edges[(i + op + top) % 24];
    unsigned tag = (((empty & 1) && i == 0) ||
                    ((empty & 2) && kinds[op] == 6 && i == destinations[op]))
                       ? 3
                       : classify(v.significand, v.sign_exponent);
    s->significand[p] = v.significand;
    s->sign_exponent[p] = v.sign_exponent;
    s->tags |= (uint16_t)(tag << (2 * p));
    if (tag != 3)
      image->b[4] |= (unsigned char)(1u << p);
    put64(image->b + 32 + 16 * i, v.significand);
    image->b[40 + 16 * i] = (unsigned char)v.sign_exponent;
    image->b[41 + 16 * i] = (unsigned char)(v.sign_exponent >> 8);
  }
  for (unsigned i = 0; i < 16; ++i)
    memory[i] = (unsigned char)(0xa5 ^ i);
  g_eax = 0x1234;
  g_ecx = 0x2345;
  g_edx = 0x3456;
  g_esi = 1025;
  g_edi = 0;
  g_ebx = 0;
  g_fs_base = 0;
  g_ebp = 0x777;
  g_esp = 0x800;
  g_eflags = 0x8d7;
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
                    GamePkgExt80 input) {
  GamePkgX87State *s = gamepkg_x86_x87_state();
  unsigned sw = get16(after->b + 2), top = (sw >> 11) & 7, valid = 0;
  if ((s->status & ~0x4500u) != (sw & ~0x4500u)) {
    fprintf(stderr,
            "store op%u kind%u cw%04x input%04x:%016llx status actual%04x "
            "expected%04x\n",
            op, kinds[op], cw, input.sign_exponent,
            (unsigned long long)input.significand, s->status, sw);
    abort();
  }
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    uint64_t v = get64(after->b + 32 + 16 * i);
    unsigned e = get16(after->b + 40 + 16 * i);
    if (s->significand[p] != v || s->sign_exponent[p] != e) {
      fprintf(stderr,
              "store op%u cw%04x p%u value actual%04x:%016llx "
              "expected%04x:%016llx\n",
              op, cw, p, s->sign_exponent[p],
              (unsigned long long)s->significand[p], e, (unsigned long long)v);
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
         s->data_pointer == (widths[op] ? (modes[op]==1?1041u:1025u) : 0x50000u));
}
static FILE *corpus;
static uint32_t corpus_rows;
static _Thread_local unsigned export_edge;
static void word(uint32_t value) {
  unsigned char b[4];
  for (unsigned i = 0; i < 4; ++i)
    b[i] = (unsigned char)(value >> (8 * i));
  assert(fwrite(b, 1, 4, corpus) == 4);
}
static void value_words(const GamePkgX87State *state, unsigned p) {
  word((uint32_t)state->significand[p]);
  word((uint32_t)(state->significand[p] >> 32));
  word(state->sign_exponent[p]);
}
static void record(unsigned op, unsigned empty, const GamePkgX87State *initial,
                   const unsigned char expected[16]) {
  unsigned top = (initial->status >> 11) & 7,
           dst = (top + destinations[op]) & 7;
  if (!corpus || (!export_edge && (top != (initial->control & 7u) ||
                                   empty != ((initial->control >> 3) & 3u))))
    return;
  unsigned pop =
      kinds[op] == 6 ? (opcodes[op] & 8) != 0 : ((opcodes[op] >> 3) & 7) != 2;
  GamePkgX87State *after = gamepkg_x86_x87_state();
  word(kinds[op]);
  word(destinations[op]);
  word(pop);
  word(initial->control);
  word(initial->status);
  word(initial->tags);
  value_words(initial, top);
  value_words(initial, dst);
  value_words(after, top);
  value_words(after, dst);
  word(after->status);
  word(after->tags);
  for (unsigned i = 0; i < 4; ++i) {
    uint32_t value = 0;
    for (unsigned j = 0; j < 4; ++j)
      value |= (uint32_t)expected[i * 4 + j] << (8 * j);
    word(value);
  }
  ++corpus_rows;
}
static unsigned check(const Image *original, unsigned op, unsigned cw,
                      unsigned top, unsigned empty, GamePkgExt80 value) {
  Image input = *original, after;
  GamePkgX87State initial;
  prepare(&input, &initial, op, cw, top, empty, value);
  uint32_t before_registers[] = {g_eax, g_ecx, g_edx, g_ebx,    g_esp,
                                 g_ebp, g_esi, g_edi, g_eflags, g_fs_base};
  unsigned char before[16], expected[16];
  memcpy(before, memory, 16);
  __asm__ volatile("fxrstor %0" ::"m"(input) : FX_CLOBBERS);
  physical[op](memory);
  __asm__ volatile("fxsave %0" : "=m"(after)::"memory");
  __asm__ volatile("fxrstor %0" ::"m"(*original) : FX_CLOBBERS);
  memcpy(expected, memory, 16);
  memcpy(memory, before, 16);
  *gamepkg_x86_x87_state() = initial;
  completed = 0;
  GamePkgX86Fault fault;
  assert(gamepkg_x86_run(generated[op], &fault) == 0 && completed == 1 &&
         g_eax == 0x1234 && g_ecx == 0x2345 && g_edx == 0x3456 &&
         g_eflags == 0x8d7 && g_esp == 0x800 && g_ebp == 0x777);
  compare(&after, op, cw, value);
  uint32_t after_registers[] = {g_eax, g_ecx, g_edx, g_ebx,    g_esp,
                                g_ebp, g_esi, g_edi, g_eflags, g_fs_base};
  assert(memcmp(before_registers, after_registers, sizeof(before_registers)) ==
         0);
  if (memcmp(memory, expected, 16)) {
    fprintf(stderr,
            "store op%u kind%u cw%04x input%04x:%016llx bytes actual%016llx "
            "expected%016llx sw%04x\n",
            op, kinds[op], cw, value.sign_exponent,
            (unsigned long long)value.significand,
            (unsigned long long)get64(memory),
            (unsigned long long)get64(expected), get16(after.b + 2));
    abort();
  }
  record(op, empty, &initial, expected);
  if (get16(after.b + 2) & ~cw & 63u) {
    GamePkgX87State before_state = *gamepkg_x86_x87_state();
    completed = 0;
    assert(gamepkg_x86_run(wait_next, &fault) == 1 && fault.vector == 16 &&
           fault.eip == 0x20000 && fault.ebp == 0x9876 && completed == 0 &&
           memcmp(&before_state, gamepkg_x86_x87_state(),
                  sizeof(before_state)) == 0);
    return 1;
  }
  return 0;
}
static GamePkgX86Memory *bind_memory(unsigned length, unsigned permissions) {
  GamePkgX86Region region = {1025, length, permissions, memory};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&region, 1, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  return m;
}
static unsigned faults(const Image *original) {
  unsigned count = 0;
  for (unsigned op = 0; op < 60; ++op)
    for (unsigned scenario = 0; scenario < 8; ++scenario) {
      GamePkgX86Memory *m =
          bind_memory(scenario & 1 ? 16 : widths[op] - 1, scenario & 1 ? 1 : 3);
      Image input = *original;
      GamePkgX87State s;
      prepare(&input, &s, op, 0x40, 3, (scenario >> 1) & 1, edges[6]);
      if (scenario >= 4)
        s.status |= 0x8081;
      *gamepkg_x86_x87_state() = s;
      unsigned char before[16];
      memcpy(before, memory, 16);
      completed = 0;
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(generated[op], &fault) == 1 && completed == 0 &&
             memcmp(&s, gamepkg_x86_x87_state(), sizeof(s)) == 0 &&
             memcmp(before, memory, 16) == 0 && fault.eip == 0x10000 &&
             fault.ebp == (modes[op] == 4 ? 1025u : 0xabcdef12u));
      assert(fault.vector == (scenario >= 4 ? 16u : 14u));
      if (scenario < 4)
        assert(fault.fault_address ==
                   (scenario & 1 ? 1025u : 1025u + widths[op] - 1) &&
               fault.error_code == (scenario & 1 ? 7u : 6u));
      gamepkg_x86_memory_bind(0);
      gamepkg_x86_memory_release(m);
      ++count;
    }
  return count;
}
static unsigned pending_and_bounds(const Image *original) {
  unsigned count = 0;
  for (unsigned op = 0; op < 76; ++op) {
    Image input = *original;
    GamePkgX87State initial;
    prepare(&input, &initial, op, 0x37e, 3, 0, edges[2]);
    initial.status |= 0x8081;
    *gamepkg_x86_x87_state() = initial;
    completed = 0;
    GamePkgX86Fault fault;
    assert(gamepkg_x86_run(generated[op], &fault) == 1 && fault.vector == 16 &&
           fault.eip == 0x10000 &&
           fault.ebp == (modes[op] == 4 ? 1025u : 0xabcdef12u) &&
           completed == 0 &&
           memcmp(&initial, gamepkg_x86_x87_state(), sizeof(initial)) == 0);
    ++count;
  }
  for (unsigned op = 0; op < 60; op += 6)
    for (unsigned pending = 0; pending < 2; ++pending) {
      Image input = *original;
      GamePkgX87State initial;
      prepare(&input, &initial, op, 0x40, 3, 1, edges[6]);
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
#ifndef _WIN32
static Image fault_before;
static unsigned expected_vector, expected_error, fault_bytes;
static uintptr_t expected_address;
static unsigned char *fault_target;
static void fault_handler(int signal, siginfo_t *info, void *raw) {
  if (expected_vector == 14 &&
      ((uintptr_t)info->si_addr != expected_address ||
       ((unsigned)((ucontext_t *)raw)->uc_mcontext.gregs[REG_ERR] & 7u) !=
           expected_error))
    _exit(89);
  for (unsigned i = 0; i < fault_bytes; ++i)
    if (fault_target[i] != (unsigned char)(0xa5 ^ i))
      _exit(88);
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
static unsigned physical_faults;
static void fault_priority(const Image *original) {
  for (unsigned op = 0; op < 60; op += 6)
    for (unsigned scenario = 0; scenario < 6; ++scenario)
      for (unsigned readonly = 0; readonly < 2; ++readonly) {
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
          struct rlimit core = {0, 0};
          assert(setrlimit(RLIMIT_CORE, &core) == 0);
          long page = sysconf(_SC_PAGESIZE);
          assert(page > 32);
          unsigned char *map = mmap(0, (size_t)page * 2, PROT_READ | PROT_WRITE,
                                    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
          assert(map != MAP_FAILED);
          unsigned width = widths[op], part = readonly ? width : width / 2;
          fault_target = readonly ? map + 64 : map + page - part;
          fault_bytes = part;
          for (unsigned j = 0; j < part; ++j)
            fault_target[j] = (unsigned char)(0xa5 ^ j);
          if (readonly) {
            assert(mprotect(map, (size_t)page, PROT_READ) == 0);
            volatile unsigned char prefault = fault_target[0];
            (void)prefault;
          } else
            assert(mprotect(map + page, (size_t)page, PROT_NONE) == 0);
          fault_before = *original;
          GamePkgX87State initial;
          GamePkgExt80 v =
              scenario == 3   ? edges[6]
              : scenario == 4 ? edges[8]
              : scenario == 5
                  ? (GamePkgExt80){UINT64_C(0x8000000000000001), 0x3fff}
                  : edges[2];
          unsigned cw = scenario == 0 ? 0x37f : scenario == 5 ? 0x35f : 0x40;
          prepare(&fault_before, &initial, op, cw, 2, scenario == 1, v);
          if (scenario == 2) {
            fault_before.b[2] = 0x81;
            fault_before.b[3] |= 0x80;
          }
          expected_vector = scenario == 2 ? 16 : 14;
          expected_address = (uintptr_t)(readonly ? fault_target : map + page);
          expected_error = readonly ? 7 : 6;
          struct sigaction action = {0};
          action.sa_sigaction = fault_handler;
          action.sa_flags = SA_SIGINFO;
          sigemptyset(&action.sa_mask);
          assert(sigaction(SIGSEGV, &action, 0) == 0 &&
                 sigaction(SIGFPE, &action, 0) == 0);
          __asm__ volatile("fxrstor %0" ::"m"(fault_before) : FX_CLOBBERS);
          if (scenario != 2)
            __asm__ volatile("fnop" ::: "memory");
#ifdef __x86_64__
          __asm__ volatile("fxsave64 %0" : "=m"(fault_before)::"memory");
#else
          __asm__ volatile("fxsave %0" : "=m"(fault_before)::"memory");
#endif
          physical[op](fault_target);
          _exit(99);
        }
        int status;
        assert(waitpid(child, &status, 0) == child);
        if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
          fprintf(stderr, "store PF op%u scenario%u readonly%u status%x\n", op,
                  scenario, readonly, status);
          abort();
        }
        ++physical_faults;
      }
}
static unsigned deferred_contexts;
static void deferred_faults(const Image *original) {
  for (unsigned op = 0; op < 76; ++op)
    for (unsigned scenario = 0; scenario < 5; ++scenario) {
      if (scenario && (kinds[op] == 2 || kinds[op] == 6))
        continue;
      pid_t child = fork();
      assert(child >= 0);
      if (child == 0) {
        struct rlimit core = {0, 0};
        assert(setrlimit(RLIMIT_CORE, &core) == 0);
        fault_before = *original;
        GamePkgX87State initial;
        GamePkgExt80 v =
            scenario == 1   ? edges[17]
            : scenario == 2 ? edges[6]
            : scenario == 3
                ? edges[8]
                : (GamePkgExt80){UINT64_C(0x8000000000000001), 0x3fff};
        unsigned cw = scenario == 4 ? 0x35f : 0x40;
        prepare(&fault_before, &initial, op, cw, 3, scenario == 0, v);
        fault_bytes = 0;
        expected_vector = 16;
        struct sigaction action = {0};
        action.sa_sigaction = fault_handler;
        action.sa_flags = SA_SIGINFO;
        sigemptyset(&action.sa_mask);
        assert(sigaction(SIGFPE, &action, 0) == 0);
        __asm__ volatile("fxrstor %0" ::"m"(fault_before) : FX_CLOBBERS);
        physical[op](memory);
#ifdef __x86_64__
        __asm__ volatile("fxsave64 %0" : "=m"(fault_before)::"memory");
#else
        __asm__ volatile("fxsave %0" : "=m"(fault_before)::"memory");
#endif
        if (!(get16(fault_before.b + 2) & ~cw & 63u))
          _exit(98);
        __asm__ volatile("fwait" ::: "memory");
        _exit(99);
      }
      int status;
      assert(waitpid(child, &status, 0) == child);
      if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        fprintf(stderr, "store MF op%u scenario%u status%x\n", op, scenario,
                status);
        abort();
      }
      ++deferred_contexts;
    }
}
#else
static unsigned deferred_contexts;
static void deferred_faults(const Image *original) { (void)original; }
static unsigned physical_faults;
static void fault_priority(const Image *original) { (void)original; }
#endif
#ifdef _WIN32
static DWORD WINAPI worker(void *raw) {
#else
static void *worker(void *raw) {
#endif
  unsigned n = (unsigned)(uintptr_t)raw;
  GamePkgX87State *s = gamepkg_x86_x87_state();
  assert(s->control == 0x37f && s->tags == 0xffff && s->status == 0);
  GamePkgX86Memory *m = bind_memory(16, 3);
  Image original;
  __asm__ volatile("fxsave %0" : "=m"(original)::"memory");
  uint64_t rng = 0x12345678 + n;
  for (unsigned i = 0; i < 1000; ++i) {
    unsigned op = (i + n) % 76;
    GamePkgExt80 v = {random64(&rng), (uint16_t)random64(&rng)};
    check(&original, op,
          0x40 | ((i + n) & 63) | ((i & 3) << 8) | ((i & 3) << 10), i & 7,
          i & 3, v);
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
  GamePkgX86Memory *m = bind_memory(16, 3);
  const char *corpus_path = getenv("GAMEPKG_TAKEDAWN_X87_STORE_CORPUS");
  if (corpus_path) {
    corpus = fopen(corpus_path, "wb");
    assert(corpus && fwrite("GPKXSTO1", 1, 8, corpus) == 8);
    word(0);
  }
  uint64_t count = 0, queued = 0, rng = UINT64_C(0x123456789abcdef);
  for (unsigned op = 0; op < 76; ++op)
    for (unsigned masks = 0; masks < 64; ++masks)
      for (unsigned pc = 0; pc < 4; ++pc)
        for (unsigned rc = 0; rc < 4; ++rc)
          for (unsigned top = 0; top < 8; ++top)
            for (unsigned empty = 0; empty < 4; ++empty) {
              GamePkgExt80 v = edges[(op + masks + pc + rc + top) % 24];
              if ((op + top + masks) % 7 == 0)
                v = (GamePkgExt80){random64(&rng), (uint16_t)random64(&rng)};
              queued +=
                  check(&original, op, 0x40 | masks | (pc << 8) | (rc << 10),
                        top, empty, v);
              ++count;
            }
  static const int exponents[] = {
      -1075, -1074, -1073, -1023, -1022, -1021, -150, -149, -148, -127,
      -126,  -125,  -1,    0,     1,     14,    15,   16,   30,   31,
      32,    62,    63,    64,    126,   127,   128,  1022, 1023, 1024};
  static const uint64_t sigs[] = {UINT64_C(0x8000000000000000),
                                  UINT64_C(0x8000000000000001),
                                  UINT64_C(0x8000000000000400),
                                  UINT64_C(0x8000000000000800),
                                  UINT64_C(0x8000008000000000),
                                  UINT64_C(0x8000010000000000),
                                  UINT64_MAX,
                                  UINT64_C(0xfffffffffffffc00),
                                  UINT64_C(0xfffffffffffff800),
                                  UINT64_C(0xffffff8000000000),
                                  UINT64_C(0xffffff0000000000),
                                  UINT64_C(0xc000000000000000),
                                  UINT64_C(0xbfffffffffffffff)};
  for (unsigned op = 0; op < 60; op += 6)
    for (unsigned e = 0; e < sizeof(exponents) / sizeof(*exponents); ++e)
      for (unsigned n = 0; n < sizeof(sigs) / sizeof(*sigs); ++n)
        for (unsigned sign = 0; sign < 2; ++sign)
          for (unsigned masks = 0; masks < 64; ++masks)
            for (unsigned rc = 0; rc < 4; ++rc) {
              export_edge = ((e * 13 + n) % 8) == 0;
              GamePkgExt80 v = {
                  sigs[n], (uint16_t)((sign << 15) | (16383 + exponents[e]))};
              queued +=
                  check(&original, op, 0x340 | masks | (rc << 10), e & 7, 0, v);
              ++count;
            }
  gamepkg_x86_memory_bind(0);
  gamepkg_x86_memory_release(m);
  unsigned failure_cases = faults(&original) + pending_and_bounds(&original);
  export_edge = 0;
  if (corpus) {
    assert(fseek(corpus, 8, SEEK_SET) == 0);
    word(corpus_rows);
    assert(fclose(corpus) == 0);
    corpus = 0;
  }
  fault_priority(&original);
  deferred_faults(&original);
  threads();
  printf("x87 stores: %llu hardware/generated cases, %llu queued faults, %u "
         "memory-priority cases, %u physical fault contexts and 8000 worker "
         "cases\n",
         (unsigned long long)count, (unsigned long long)queued, failure_cases,
         physical_faults + deferred_contexts);
  return 0;
}
