#define _GNU_SOURCE
#include <assert.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <ucontext.h>
#include <unistd.h>
#if !defined(__i386__)
#error This native page-fault oracle requires an actual i386 process
#endif
static unsigned char alt[65536], *pages;
static uint32_t expected_ip, address, accumulator, expected_error;
static unsigned readonly;
static void handler(int sig, siginfo_t *info, void *raw) {
  __asm__ volatile("cld" ::: "cc");
  ucontext_t *u = raw;
  assert(sig == SIGSEGV && (uint32_t)u->uc_mcontext.gregs[REG_TRAPNO] == 14);
  assert((uint32_t)u->uc_mcontext.gregs[REG_ERR] == expected_error);
  assert((uint32_t)(uintptr_t)info->si_addr ==
         (uint32_t)(uintptr_t)(pages + 4096));
  assert((uint32_t)u->uc_mcontext.gregs[REG_EIP] == expected_ip);
  assert((uint32_t)u->uc_mcontext.gregs[REG_EAX] == accumulator);
  assert((uint32_t)u->uc_mcontext.gregs[REG_ECX] == 0x87654321);
  assert((uint32_t)u->uc_mcontext.gregs[REG_EDI] == address);
  assert(((uint32_t)u->uc_mcontext.gregs[REG_EFL] & 0x8d5) == (0xcd7 & 0x8d5));
  for (unsigned i = 4080; i < (readonly ? 4112u : 4096u); ++i)
    assert(pages[i] == 0xa5);
  _exit(0);
}
static void execute(unsigned kind, unsigned bytes, unsigned locked) {
  uint32_t a = accumulator, c = 0x87654321, d = address, flags = 0xcd7;
#define RUN(prefix, code)                                                      \
  __asm__ volatile("movl $1f,%[ip];push %[flags];popfl;1:" prefix code ";cld"  \
                   : "+a"(a), "+c"(c), "+D"(d), [ip] "=m"(expected_ip)         \
                   : [flags] "m"(flags)                                        \
                   : "memory", "cc")
#define WIDTH(prefix)                                                          \
  if (kind == 0) {                                                             \
    if (bytes == 1) {                                                          \
      RUN(prefix, ".byte 0x86,0x0f");                                          \
    } else if (bytes == 2) {                                                   \
      RUN(prefix, ".byte 0x66,0x87,0x0f");                                     \
    } else {                                                                   \
      RUN(prefix, ".byte 0x87,0x0f");                                          \
    }                                                                          \
  } else if (kind == 1) {                                                      \
    if (bytes == 1) {                                                          \
      RUN(prefix, ".byte 0x0f,0xc0,0x0f");                                     \
    } else if (bytes == 2) {                                                   \
      RUN(prefix, ".byte 0x66,0x0f,0xc1,0x0f");                                \
    } else {                                                                   \
      RUN(prefix, ".byte 0x0f,0xc1,0x0f");                                     \
    }                                                                          \
  } else {                                                                     \
    if (bytes == 1) {                                                          \
      RUN(prefix, ".byte 0x0f,0xb0,0x0f");                                     \
    } else if (bytes == 2) {                                                   \
      RUN(prefix, ".byte 0x66,0x0f,0xb1,0x0f");                                \
    } else {                                                                   \
      RUN(prefix, ".byte 0x0f,0xb1,0x0f");                                     \
    }                                                                          \
  }
  if (locked) {
    WIDTH(".byte 0xf0;")
  } else {
    WIDTH("")
  }
#undef WIDTH
#undef RUN
  _exit(9);
}
int main(void) {
  struct rlimit limit = {0, 0};
  assert(!setrlimit(RLIMIT_CORE, &limit));
  pages =
      mmap(0, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  assert(pages != MAP_FAILED);
  unsigned count = 0;
  for (unsigned kind = 0; kind < 3; ++kind)
    for (unsigned width = 0; width < 3; ++width)
      for (unsigned locked = 0; locked < 2; ++locked)
        for (unsigned equal = 0; equal < 2; ++equal)
          for (unsigned ro = 0; ro < 2; ++ro) {
            unsigned bytes = 1u << width;
            for (unsigned before = 0; before < bytes; ++before) {
              pid_t child = fork();
              assert(child >= 0);
              if (!child) {
                alarm(5);
                for (unsigned i = 0; i < 8192; ++i)
                  pages[i] = 0xa5;
                readonly = ro;
                expected_error = ro ? 7 : 6;
                accumulator = equal ? 0xa5a5a5a5 : 0x11223344;
                address = (uint32_t)(uintptr_t)(pages + 4096 - before);
                stack_t stack = {.ss_sp = alt, .ss_size = sizeof(alt)};
                assert(!sigaltstack(&stack, 0));
                struct sigaction action = {0};
                action.sa_sigaction = handler;
                action.sa_flags = SA_SIGINFO | SA_ONSTACK;
                sigemptyset(&action.sa_mask);
                assert(!sigaction(SIGSEGV, &action, 0));
                assert(
                    !mprotect(pages + 4096, 4096, ro ? PROT_READ : PROT_NONE));
                execute(kind, bytes, locked);
              }
              int status;
              assert(waitpid(child, &status, 0) == child);
              if (!WIFEXITED(status) || WEXITSTATUS(status)) {
                fprintf(stderr,
                        "physical atomic fault failed kind%u bytes%u lock%u "
                        "equal%u readonly%u before%u status%d\n",
                        kind, bytes, locked, equal, ro, before, status);
                return 1;
              }
              ++count;
            }
          }
  assert(!munmap(pages, 8192));
  printf("Actual i386 atomic faults: %u protected-page contexts, exact "
         "flags/registers/write-fault metadata and no partial store passed\n",
         count);
  return 0;
}
