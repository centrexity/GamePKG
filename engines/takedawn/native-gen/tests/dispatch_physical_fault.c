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
#error Actual i386 process required
#endif
static unsigned char alt[65536], *pages;
static uint32_t initial_sp, expected_ip, flags = 0xcd7;
static unsigned operation;
static void handler(int signal, siginfo_t *info, void *raw) {
  __asm__ volatile("cld" ::: "cc");
  ucontext_t *u = raw;
  if (operation == 7) {
    assert(signal == SIGTRAP && u->uc_mcontext.gregs[REG_TRAPNO] == 3 &&
           u->uc_mcontext.gregs[REG_ERR] == 0);
    assert((uint32_t)u->uc_mcontext.gregs[REG_EIP] == expected_ip + 1 &&
           (uint32_t)u->uc_mcontext.gregs[REG_ESP] == initial_sp);
    assert((uint32_t)u->uc_mcontext.gregs[REG_EAX] == 0x12345678 &&
           (uint32_t)u->uc_mcontext.gregs[REG_ECX] == 0x87654321 &&
           (uint32_t)u->uc_mcontext.gregs[REG_EDX] == 0x456789ab);
    assert(((uint32_t)u->uc_mcontext.gregs[REG_EFL] & 0xcd5) ==
           (flags & 0xcd5));
    _exit(0);
  }
  assert(signal == SIGSEGV && (uint32_t)u->uc_mcontext.gregs[REG_TRAPNO] == 14);
  uint32_t fetch = operation == 6;
  assert((uint32_t)u->uc_mcontext.gregs[REG_ESP] ==
         initial_sp + (fetch ? 4u : 0u));
  assert((uint32_t)u->uc_mcontext.gregs[REG_EIP] ==
         (fetch ? 0x43434000 : expected_ip));
  assert((uint32_t)(uintptr_t)info->si_addr ==
         (fetch ? 0x43434000 : (uint32_t)(uintptr_t)(pages + 4096)));
  assert(((uint32_t)u->uc_mcontext.gregs[REG_ERR] & 7u) ==
         (operation == 4 ? 6u : 4u));
  assert((uint32_t)u->uc_mcontext.gregs[REG_EAX] == 0x12345678 &&
         (uint32_t)u->uc_mcontext.gregs[REG_ECX] == 0x87654321 &&
         (uint32_t)u->uc_mcontext.gregs[REG_EDX] == 0x456789ab);
  assert(((uint32_t)u->uc_mcontext.gregs[REG_EFL] & 0xcd5) == (flags & 0xcd5));
  if (!fetch)
    for (unsigned i = 4080; i < 4096; ++i)
      assert(pages[i] == 0xa5);
  _exit(0);
}
static void execute(void) {
  uint32_t a = 0x12345678, c = 0x87654321, d = 0x456789ab;
#define CPU(code)                                                              \
  __asm__ volatile(                                                            \
      "movl $1f,%[ip];push %[flags];popfl;movl %%edi,%%esp;1:" code            \
      : "+a"(a), "+c"(c), "+d"(d), [ip] "=m"(expected_ip)                      \
      : "D"(initial_sp), [flags] "m"(flags)                                    \
      : "cc", "memory")
  switch (operation) {
  case 0:
    CPU("ret");
    break;
  case 1:
    CPU("ret $32767");
    break;
  case 2:
    CPU(".byte 0x66,0xc3");
    break;
  case 3:
    CPU(".byte 0x66,0xc2,0xff,0x7f");
    break;
  case 4:
    CPU("call 2f;2:ud2");
    break;
  case 5:
    CPU("call *(%%esp)");
    break;
  case 6:
    CPU("ret");
    break;
  case 7:
    CPU("int3");
    break;
  default:
    _exit(8);
  }
#undef CPU
  _exit(9);
}
int main(void) {
  struct rlimit limit = {0, 0};
  assert(!setrlimit(RLIMIT_CORE, &limit));
  pages = mmap(NULL, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS,
               -1, 0);
  assert(pages != MAP_FAILED);
  void *fetch = mmap((void *)0x43434000, 4096, PROT_NONE,
                     MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0);
  assert(fetch == (void *)0x43434000);
  unsigned count = 0;
  for (operation = 0; operation < 8; ++operation) {
    unsigned width = operation == 2 || operation == 3 ? 2 : 4,
             cases = operation < 5 ? width : 1;
    for (unsigned before = 0; before < cases; ++before) {
      pid_t child = fork();
      assert(child >= 0);
      if (!child) {
        alarm(5);
        for (unsigned i = 0; i < 8192; ++i)
          pages[i] = 0xa5;
        initial_sp =
            (uint32_t)(uintptr_t)(pages + 4096) - (operation < 4 ? before : 0);
        if (operation == 4)
          initial_sp += before + 1;
        if (operation == 6) {
          initial_sp -= 64;
          *(uint32_t *)(uintptr_t)initial_sp = 0x43434000;
        }
        stack_t stack = {.ss_sp = alt, .ss_size = sizeof(alt)};
        assert(!sigaltstack(&stack, NULL));
        struct sigaction action = {0};
        action.sa_sigaction = handler;
        action.sa_flags = SA_SIGINFO | SA_ONSTACK;
        sigemptyset(&action.sa_mask);
        assert(!sigaction(SIGSEGV, &action, NULL));
        assert(!sigaction(SIGTRAP, &action, NULL));
        assert(!mprotect(pages + 4096, 4096, PROT_NONE));
        execute();
      }
      int status;
      assert(waitpid(child, &status, 0) == child);
      if (!WIFEXITED(status) || WEXITSTATUS(status)) {
        fprintf(stderr, "native control fault op%u before%u status%d\n",
                operation, before, status);
        return 1;
      }
      ++count;
    }
  }
  assert(!munmap(fetch, 4096) && !munmap(pages, 8192));
  printf("Actual i386 control faults:%u contexts;RET read/cleanup and CALL "
         "write/source restart state,INT3 next-EIP trap,RET target-fetch "
         "commits verified (host "
         "fetch bits not P3 oracle)\n",
         count);
  return 0;
}
