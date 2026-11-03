#define _GNU_SOURCE
/* Independent actual CPL3 x86 instructions: no runtime implementation. */
#include <assert.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <ucontext.h>
#include <unistd.h>
#if !defined(__i386__)
#error Actual i386 process required
#endif
static uint32_t expected_ip, expected_sp, expected_flags, initial_flags;
static unsigned operation;
static void handler(int signal, siginfo_t *info, void *raw) {
  __asm__ volatile("cld" ::: "cc");
  (void)info;
  ucontext_t *u = raw;
  assert(signal == SIGSEGV && u->uc_mcontext.gregs[REG_TRAPNO] == 13 &&
         u->uc_mcontext.gregs[REG_ERR] == 0);
  assert((uint32_t)u->uc_mcontext.gregs[REG_EIP] == expected_ip &&
         (uint32_t)u->uc_mcontext.gregs[REG_ESP] == expected_sp);
  assert((uint32_t)u->uc_mcontext.gregs[REG_EAX] == 0x12345678 &&
         (uint32_t)u->uc_mcontext.gregs[REG_ECX] == 0x87654321 &&
         (uint32_t)u->uc_mcontext.gregs[REG_EDX] == 0x456789ab);
  /* Fault delivery sets RF in the saved image; it was not changed by CLI/STI.
   */
  assert(((uint32_t)u->uc_mcontext.gregs[REG_EFL] & ~0x10000u) ==
         expected_flags);
  assert((expected_flags & 0x3200) == 0x200); /* CPL3, IOPL0, physical IF1. */
  _exit(0);
}
static void execute(void) {
  uint32_t a = 0x12345678, c = 0x87654321, d = 0x456789ab;
#define CPU(opcode)                                                            \
  __asm__ volatile("push %[flags];popfl;pushfl;pop %[actual];"                 \
                   "movl %%esp,%[sp];movl $1f,%[ip];1:" opcode ";ud2"          \
                   : "+a"(a), "+c"(c), "+d"(d), [ip] "=m"(expected_ip),        \
                     [sp] "=m"(expected_sp), [actual] "=m"(expected_flags)     \
                   : [flags] "m"(initial_flags)                                \
                   : "cc", "memory")
  if (operation)
    CPU("sti");
  else
    CPU("cli");
#undef CPU
  _exit(9);
}
int main(void) {
  struct rlimit limit = {0, 0};
  assert(!setrlimit(RLIMIT_CORE, &limit));
  const unsigned bits[] = {1, 4, 16, 64, 128, 1024, 2048};
  unsigned count = 0;
  for (operation = 0; operation < 2; ++operation)
    for (unsigned status = 0; status < 128; ++status) {
      initial_flags = 2;
      for (unsigned i = 0; i < 7; ++i)
        if (status & (1u << i))
          initial_flags |= bits[i];
      pid_t child = fork();
      assert(child >= 0);
      if (!child) {
        alarm(5);
        struct sigaction action = {0};
        action.sa_sigaction = handler;
        action.sa_flags = SA_SIGINFO;
        sigemptyset(&action.sa_mask);
        assert(!sigaction(SIGSEGV, &action, NULL));
        execute();
      }
      int status_code;
      assert(waitpid(child, &status_code, 0) == child);
      assert(WIFEXITED(status_code) && WEXITSTATUS(status_code) == 0);
      ++count;
    }
  printf("Actual i386 CLI/STI:%u privileged-instruction #GP contexts preserve "
         "flags/GPRs/ESP/EIP\n",
         count);
  return 0;
}
