/* Maintainer-only actual i386 kernel fault-context oracle. Runs in bounded
 * reaped child processes with a private alternate signal stack and no cores. */
#define _GNU_SOURCE 1
#if !defined(__linux__) || !defined(__i386__)
#error Compile this oracle as an actual Linux i386 process
#endif
#include <assert.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <ucontext.h>
#include <unistd.h>
static int pipe_output;
static void handler(int signal, siginfo_t *info, void *raw) {
  ucontext_t *u = raw;
  uint32_t report[] = {(uint32_t)signal,
                       (uint32_t)u->uc_mcontext.gregs[REG_TRAPNO],
                       (uint32_t)u->uc_mcontext.gregs[REG_ESP],
                       (uint32_t)u->uc_mcontext.gregs[REG_EBP],
                       (uint32_t)u->uc_mcontext.gregs[REG_ERR],
                       (uint32_t)(uintptr_t)info->si_addr};
  if (write(pipe_output, report, sizeof(report)) != (ssize_t)sizeof(report))
    _exit(90);
  _exit(0);
}
static void *mapping(uint32_t address, size_t size, int prot) {
  void *p = mmap((void *)(uintptr_t)address, size, prot,
                 MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0);
  assert(p == (void *)(uintptr_t)address);
  return p;
}
int main(void) {
  struct rlimit parent_no_core = {0, 0};
  assert(!setrlimit(RLIMIT_CORE, &parent_no_core));
  for (unsigned op = 0; op < 12; ++op) {
    int fds[2];
    assert(!pipe(fds));
    pid_t child = fork();
    assert(child >= 0);
    if (!child) {
      alarm(10);
      close(fds[0]);
      pipe_output = fds[1];
      struct rlimit no_core = {0, 0};
      assert(!setrlimit(RLIMIT_CORE, &no_core));
      void *alt = mmap(NULL, 65536, PROT_READ | PROT_WRITE,
                       MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
      assert(alt != MAP_FAILED);
      stack_t stack = {.ss_sp = alt, .ss_size = 65536};
      assert(!sigaltstack(&stack, NULL));
      struct sigaction action = {0};
      action.sa_sigaction = handler;
      action.sa_flags = SA_SIGINFO | SA_ONSTACK;
      sigemptyset(&action.sa_mask);
      assert(!sigaction(SIGSEGV, &action, NULL));
      mapping(0x20000000, 4096, PROT_NONE);
      mapping(0x30000000, 4096, PROT_READ | PROT_WRITE);
      mapping(0x30001000, 4096, PROT_NONE);
      /* The assembly always faults or executes UD2. No path can return to C
       * with the compiler's native SP/BP changed. Handler uses its alt stack.
       */
      switch (op) {
      case 0:
        __asm__ volatile("mov $0x20000004,%%esp;pushl $7;ud2" ::: "memory");
        __builtin_unreachable();
      case 1:
        __asm__ volatile("mov $0x20000002,%%esp;pushw $7;ud2" ::: "memory");
        __builtin_unreachable();
      case 2:
        __asm__ volatile(
            "mov $0x30000000,%%ebp;mov $0x20000000,%%esp;popl %%eax;ud2" ::
                : "memory");
        __builtin_unreachable();
      case 3:
        __asm__ volatile(
            "mov $0x30000000,%%ebp;mov $0x20000000,%%esp;popw %%ax;ud2" ::
                : "memory");
        __builtin_unreachable();
      case 4:
        __asm__ volatile(
            "mov $0x30000000,%%esp;mov $0x20000000,%%ebp;leave;ud2" ::
                : "memory");
        __builtin_unreachable();
      case 5:
        __asm__ volatile(
            "mov $0x30000000,%%esp;mov $0x20000000,%%ebp;.byte 0x66,0xc9;ud2" ::
                : "memory");
        __builtin_unreachable();
      case 6:
        __asm__ volatile("mov $0x30000000,%%esp;popl 0x20000000;ud2" ::
                             : "memory");
        __builtin_unreachable();
      case 7:
        __asm__ volatile("mov $0x30000000,%%esp;popw 0x20000000;ud2" ::
                             : "memory");
        __builtin_unreachable();
      case 8:
        __asm__ volatile("mov $0x30000ffc,%%esp;popl (%%esp);ud2" ::: "memory");
        __builtin_unreachable();
      case 9:
        __asm__ volatile("mov $0x30000ffe,%%esp;popw (%%esp);ud2" ::: "memory");
        __builtin_unreachable();
      case 10:
        __asm__ volatile("mov $0x20000000,%%esp;pushl (%%esp);ud2" ::
                             : "memory");
        __builtin_unreachable();
      default:
        __asm__ volatile("mov $0x20000000,%%esp;pushw (%%esp);ud2" ::
                             : "memory");
        __builtin_unreachable();
      }
    }
    close(fds[1]);
    uint32_t report[6];
    ssize_t n = read(fds[0], report, sizeof(report));
    close(fds[0]);
    int status;
    assert(waitpid(child, &status, 0) == child && WIFEXITED(status) &&
           WEXITSTATUS(status) == 0);
    assert(n == (ssize_t)sizeof(report) && report[0] == SIGSEGV &&
           report[1] == 14);
    uint32_t esp = op == 0                            ? 0x20000004
                   : op == 1                          ? 0x20000002
                   : op == 8                          ? 0x30000ffc
                   : op == 9                          ? 0x30000ffe
                   : (op == 2 || op == 3 || op >= 10) ? 0x20000000
                                                      : 0x30000000;
    assert(report[2] == esp);
    if (op == 2 || op == 3)
      assert(report[3] == 0x30000000);
    if (op == 4 || op == 5)
      assert(report[3] == 0x20000000);
    assert(report[5] == (op == 8 || op == 9 ? 0x30001000u : 0x20000000u));
    assert(report[4] == (op < 2 || (op >= 6 && op <= 9) ? 6u : 4u));
  }
  puts("Actual i386: 12 protected-page PUSH/POP/LEAVE source/store contexts "
       "preserve original ESP and EBP; POP [ESP] destination uses "
       "post-increment address");
}
