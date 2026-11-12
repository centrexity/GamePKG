/* Actual i386 repeated-comparison fault flags and index/count progress. */
#define _GNU_SOURCE 1
#if !defined(__linux__) || !defined(__i386__)
#error Compile as an actual Linux i386 process
#endif
#include <assert.h>
#include <cpuid.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <ucontext.h>
#include <unistd.h>
static int output;
static volatile sig_atomic_t initial_flags;
static void trap(int signal, siginfo_t *info, void *raw) {
  ucontext_t *u = raw;
  uint32_t words[] = {(uint32_t)signal,
                      (uint32_t)u->uc_mcontext.gregs[REG_TRAPNO],
                      (uint32_t)u->uc_mcontext.gregs[REG_EAX],
                      (uint32_t)u->uc_mcontext.gregs[REG_ECX],
                      (uint32_t)u->uc_mcontext.gregs[REG_ESI],
                      (uint32_t)u->uc_mcontext.gregs[REG_EDI],
                      (uint32_t)u->uc_mcontext.gregs[REG_EFL],
                      (uint32_t)(uintptr_t)info->si_addr,
                      (uint32_t)u->uc_mcontext.gregs[REG_ERR],
                      (uint32_t)initial_flags};
  if (write(output, words, sizeof(words)) != (ssize_t)sizeof(words))
    _exit(90);
  _exit(0);
}
static unsigned char *mapping(uint32_t address, unsigned bytes) {
  void *p = mmap((void *)(uintptr_t)address, bytes, PROT_READ | PROT_WRITE,
                 MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0);
  assert(p == (void *)(uintptr_t)address);
  return p;
}
/* This is a host observation, not the Pentium III fault-flags oracle. The
 * target service strictly retains Intel's documented pre-instruction flags.
 * Some modern hosts instead expose the last comparison. Keep both observed
 * profiles exact and report them separately; do not relax the target tests. */
static uint32_t last_compare(unsigned scan, unsigned width, unsigned ne) {
  uint32_t mask = UINT32_MAX >> (32 - width * 8), sign = 1u << (width * 8 - 1);
  uint32_t a = (scan ? 0x89abcdefu : 0x51738ba5u + 2 * 17) & mask,
           b = (a ^ (ne ? 1u : 0u)) & mask;
  uint32_t r = (a - b) & mask, parity = 0;
  for (unsigned i = 0; i < 8; ++i)
    parity ^= (r >> i) & 1;
  int64_t sa = (int64_t)(a ^ sign) - sign, sb = (int64_t)(b ^ sign) - sign;
  int64_t difference = sa - sb;
  return (a < b ? 1u : 0u) | (!parity ? 4u : 0u) |
         ((a & 15) < (b & 15) ? 16u : 0u) | (!r ? 64u : 0u) |
         (r >= sign ? 128u : 0u) |
         (difference < -(int64_t)sign || difference > (int64_t)sign - 1 ? 2048u
                                                                        : 0u);
}
int main(void) {
  struct rlimit no_core = {0, 0};
  assert(!setrlimit(RLIMIT_CORE, &no_core));
  unsigned checks = 0, original = 0, last = 0;
  for (unsigned scan = 0; scan < 2; ++scan)
    for (unsigned size = 0; size < 3; ++size)
      for (unsigned df = 0; df < 2; ++df)
        for (unsigned ne = 0; ne < 2; ++ne) {
          unsigned width = 1u << size;
          uint32_t source = 0x20001080,
                   dest = df ? 0x30001000 + 2 * width : 0x30001000 - 3 * width;
          int fds[2];
          assert(!pipe(fds));
          pid_t child = fork();
          assert(child >= 0);
          if (!child) {
            alarm(10);
            close(fds[0]);
            output = fds[1];
            void *alt = mmap(NULL, 65536, PROT_READ | PROT_WRITE,
                             MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
            assert(alt != MAP_FAILED);
            stack_t ss = {.ss_sp = alt, .ss_size = 65536};
            assert(!sigaltstack(&ss, NULL));
            struct sigaction sa = {0};
            sa.sa_sigaction = trap;
            sa.sa_flags = SA_SIGINFO | SA_ONSTACK;
            sigemptyset(&sa.sa_mask);
            assert(!sigaction(SIGSEGV, &sa, NULL));
            unsigned char *src = mapping(0x20000000, 8192),
                          *dst = mapping(0x30000000, 8192);
            for (unsigned i = 0; i < 4; ++i) {
              uint32_t s = source + (df ? 0u - i * width : i * width),
                       d = dest + (df ? 0u - i * width : i * width),
                       v = scan ? 0x89abcdefu : 0x51738ba5u + i * 17;
              for (unsigned j = 0; j < width; ++j) {
                src[s - 0x20000000 + j] = (unsigned char)(v >> (8 * j));
                dst[d - 0x30000000 + j] =
                    (unsigned char)((v ^ (ne ? 1u : 0u)) >> (8 * j));
              }
            }
            assert(!mprotect(dst + (df ? 0 : 4096), 4096, PROT_NONE));
            uint32_t eax = 0x89abcdef, ecx = 8, esi = source, edi = dest,
                     control;
            __asm__ volatile("pushfl;pop %0" : "=r"(control)::"memory");
            control = (control & ~0xcd5u) | 0x8d5u | (df << 10);
#define EXEC(code)                                                             \
  __asm__ volatile("push %[control];popfl;pushfl;pop %[before];" code ";ud2"   \
                   : "+a"(eax), "+c"(ecx), "+S"(esi),                          \
                     "+D"(edi), [before] "=m"(initial_flags)                   \
                   : [control] "m"(control)                                    \
                   : "memory", "cc")
            if (!scan) {
              if (width == 1) {
                if (ne) {
                  EXEC("repne cmpsb");
                } else {
                  EXEC("repe cmpsb");
                }
              } else if (width == 2) {
                if (ne) {
                  EXEC("repne cmpsw");
                } else {
                  EXEC("repe cmpsw");
                }
              } else {
                if (ne) {
                  EXEC("repne cmpsl");
                } else {
                  EXEC("repe cmpsl");
                }
              }
            } else {
              if (width == 1) {
                if (ne) {
                  EXEC("repne scasb");
                } else {
                  EXEC("repe scasb");
                }
              } else if (width == 2) {
                if (ne) {
                  EXEC("repne scasw");
                } else {
                  EXEC("repe scasw");
                }
              } else {
                if (ne) {
                  EXEC("repne scasl");
                } else {
                  EXEC("repe scasl");
                }
              }
            }
#undef EXEC
            __builtin_unreachable();
          }
          close(fds[1]);
          uint32_t report[10];
          ssize_t n = read(fds[0], report, sizeof(report));
          close(fds[0]);
          int status;
          assert(waitpid(child, &status, 0) == child && WIFEXITED(status) &&
                 !WEXITSTATUS(status));
          assert(n == (ssize_t)sizeof(report));
          uint32_t delta = df ? 0u - 3 * width : 3 * width;
          if (report[0] != SIGSEGV || report[1] != 14 ||
              report[2] != 0x89abcdef || report[3] != 5 ||
              report[4] != (scan ? source : source + delta) ||
              report[5] != dest + delta ||
              (report[9] & 0xcd5u) != (0x8d5u | (df << 10)) ||
              ((report[6] >> 10) & 1) != df || report[7] != dest + delta ||
              report[8] != 4) {
            fprintf(stderr,
                    "string physical scan%u width%u df%u ne%u count%u si%08x "
                    "di%08x flags%08x\n",
                    scan, width, df, ne, report[3], report[4], report[5],
                    report[6]);
            assert(0);
          }
          uint32_t arithmetic = report[6] & 0x8d5u;
          if (arithmetic == 0x8d5u)
            ++original;
          else if (arithmetic == last_compare(scan, width, ne))
            ++last;
          else {
            fprintf(stderr, "unclassified host REP fault flags %08x\n",
                    report[6]);
            assert(0);
          }
          ++checks;
        }
  unsigned a, b, c, d;
  char vendor[13] = {0};
  if (__get_cpuid(0, &a, &b, &c, &d)) {
    memcpy(vendor, &b, 4);
    memcpy(vendor + 4, &d, 4);
    memcpy(vendor + 8, &c, 4);
  }
  printf("Actual i386 host %s: %u repeated compare faults with exact "
         "index/count progress; flags original=%u last-compare=%u. Host flags "
         "are not the target-profile oracle.\n",
         vendor, checks, original, last);
  return 0;
}
