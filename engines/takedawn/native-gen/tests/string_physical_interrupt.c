/* Actual i386 asynchronous REP contexts. Uses shared RAM and a binary pipe;
 * handler calls only async-signal-safe write/_exit. No game/runtime code.
 * RF is checked separately from logical pre-interrupt checkpoint flags. */
#define _GNU_SOURCE
#include <assert.h>
#include <errno.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <ucontext.h>
#include <unistd.h>
#if !defined(__i386__)
#error Actual i386 Linux required
#endif
static uint32_t instruction_pc, instruction_count;
static int output;
struct Observation {
  uint32_t signal, pc, count, source, destination, accumulator, flags;
};
static void alarm_handler(int signal, siginfo_t *info, void *context) {
  __asm__ volatile("cld" ::: "cc");
  (void)info;
  ucontext_t *u = context;
  if ((uint32_t)u->uc_mcontext.gregs[REG_EIP] != instruction_pc ||
      !(uint32_t)u->uc_mcontext.gregs[REG_ECX] ||
      (uint32_t)u->uc_mcontext.gregs[REG_ECX] >= instruction_count)
    return;
  struct Observation o = {(uint32_t)signal,
                          (uint32_t)u->uc_mcontext.gregs[REG_EIP],
                          (uint32_t)u->uc_mcontext.gregs[REG_ECX],
                          (uint32_t)u->uc_mcontext.gregs[REG_ESI],
                          (uint32_t)u->uc_mcontext.gregs[REG_EDI],
                          (uint32_t)u->uc_mcontext.gregs[REG_EAX],
                          (uint32_t)u->uc_mcontext.gregs[REG_EFL]};
  ssize_t n;
  do {
    n = write(output, &o, sizeof(o));
  } while (n < 0 && errno == EINTR);
  _exit(n == (ssize_t)sizeof(o) ? 0 : 11);
}
static void execute(unsigned op, unsigned width, unsigned backward,
                    uint32_t source, uint32_t destination, uint32_t count) {
  uint32_t flags = 0x203 | (backward ? 0x400 : 0), accumulator = 0x55555555;
  instruction_count = count;
#define RUN(code)                                                              \
  __asm__ volatile("movl $1f,%[pc];push %[flags];popfl;1:" code ";cld"         \
                   : "+a"(accumulator), "+c"(count), "+S"(source),             \
                     "+D"(destination), [pc] "=m"(instruction_pc)              \
                   : [flags] "m"(flags)                                        \
                   : "memory", "cc")
#define WIDTH(a, b, c)                                                         \
  do {                                                                         \
    if (width == 1) {                                                          \
      RUN(a);                                                                  \
    } else if (width == 2) {                                                   \
      RUN(b);                                                                  \
    } else {                                                                   \
      RUN(c);                                                                  \
    }                                                                          \
  } while (0)
  switch (op) {
  case 0:
    WIDTH("rep movsb", "rep movsw", "rep movsl");
    break;
  case 1:
    WIDTH("rep stosb", "rep stosw", "rep stosl");
    break;
  case 2:
    WIDTH("rep lodsb", "rep lodsw", "rep lodsl");
    break;
  case 3:
    WIDTH("repe cmpsb", "repe cmpsw", "repe cmpsl");
    break;
  case 4:
    WIDTH("repne cmpsb", "repne cmpsw", "repne cmpsl");
    break;
  case 5:
    WIDTH("repe scasb", "repe scasw", "repe scasl");
    break;
  case 6:
    WIDTH("repne scasb", "repne scasw", "repne scasl");
    break;
  default:
    _exit(12);
  }
#undef WIDTH
#undef RUN
  _exit(13); /* No interior signal observation is a failure, never a skip. */
}
static uint32_t comparison_flags(unsigned mismatch, unsigned width,
                                 unsigned backward) {
  uint32_t a = 0x55555555, b = mismatch ? 0xaaaaaaaa : a, flags;
#define CMP(code)                                                              \
  __asm__ volatile("push %[before];popfl;" code ";pushfl;pop %[after];cld"     \
                   : [after] "=r"(flags)                                       \
                   : [before] "r"(0x203u | (backward ? 0x400u : 0)), "a"(a),   \
                     "d"(b)                                                    \
                   : "cc")
  if (width == 1) {
    CMP("cmpb %%dl,%%al");
  } else if (width == 2) {
    CMP("cmpw %%dx,%%ax");
  } else {
    CMP("cmpl %%edx,%%eax");
  }
#undef CMP
  return flags;
}
int main(void) {
  const size_t size = 128u * 1024u * 1024u;
  unsigned char *source = mmap(NULL, size, PROT_READ | PROT_WRITE,
                               MAP_SHARED | MAP_ANONYMOUS, -1, 0);
  unsigned char *destination = mmap(NULL, size, PROT_READ | PROT_WRITE,
                                    MAP_SHARED | MAP_ANONYMOUS, -1, 0);
  assert(source != MAP_FAILED && destination != MAP_FAILED);
  for (unsigned op = 0; op < 7; ++op)
    for (unsigned width = 1; width <= 4; width *= 2)
      for (unsigned backward = 0; backward < 2; ++backward) {
        memset(source, 0x55, size);
        unsigned char initial = (op < 2 || op == 4 || op == 6) ? 0xaa : 0x55;
        memset(destination, initial, size);
        uint32_t initial_source =
            (uint32_t)(uintptr_t)source + (backward ? size - width : 0);
        uint32_t initial_destination =
            (uint32_t)(uintptr_t)destination + (backward ? size - width : 0);
        uint32_t count = (uint32_t)(size / width);
        int pipefd[2];
        assert(!pipe(pipefd));
        pid_t child = fork();
        assert(child >= 0);
        if (!child) {
          close(pipefd[0]);
          output = pipefd[1];
          struct sigaction action = {0};
          action.sa_flags = SA_SIGINFO;
          action.sa_sigaction = alarm_handler;
          assert(!sigemptyset(&action.sa_mask) &&
                 !sigaction(SIGALRM, &action, NULL));
          struct itimerval timer = {.it_value = {.tv_usec = 50},
                                    .it_interval = {.tv_usec = 50}};
          assert(!setitimer(ITIMER_REAL, &timer, NULL));
          execute(op, width, backward, initial_source, initial_destination,
                  count);
        }
        assert(!close(pipefd[1]));
        struct Observation o;
        size_t read_bytes = 0;
        while (read_bytes < sizeof(o)) {
          ssize_t n = read(pipefd[0], (unsigned char *)&o + read_bytes,
                           sizeof(o) - read_bytes);
          if (n < 0 && errno == EINTR)
            continue;
          assert(n > 0);
          read_bytes += (size_t)n;
        }
        unsigned char extra;
        assert(read(pipefd[0], &extra, 1) == 0 && !close(pipefd[0]));
        int status;
        assert(waitpid(child, &status, 0) == child && WIFEXITED(status) &&
               WEXITSTATUS(status) == 0);
        assert(o.signal == SIGALRM && o.count > 0 && o.count < count);
        uint32_t completed = count - o.count, delta = completed * width,
                 displacement = backward ? 0u - delta : delta;
        assert(o.source ==
               initial_source + ((op == 0 || op == 2 || op == 3 || op == 4)
                                     ? displacement
                                     : 0));
        assert(o.destination ==
               initial_destination + (op == 2 ? 0 : displacement));
        assert(o.accumulator == 0x55555555);
        uint32_t expected_flags =
            op < 3 ? (0x203u | (backward ? 0x400u : 0))
                   : comparison_flags(op == 4 || op == 6, width, backward);
        if ((o.flags & ~0x10000u) != expected_flags)
          fprintf(stderr, "op%u width%u df%u flags%x expected%x\n", op, width,
                  backward, o.flags, expected_flags);
        assert(o.flags == (expected_flags | 0x10000u));
        for (size_t i = 0; i < size; ++i) {
          assert(source[i] == 0x55);
          unsigned char want = initial;
          if (op < 2 && (backward ? i >= size - delta : i < delta))
            want = 0x55;
          assert(destination[i] == want);
        }
        printf("Actual i386 REP signal: op%u width%u DF%u remaining%u, "
               "flags%04x; exact indices and full RAM passed\n",
               op, width, backward, o.count, o.flags);
        fflush(stdout);
      }
  assert(!munmap(source, size) && !munmap(destination, size));
  puts("42 physical asynchronous REP checkpoints passed; RF=1 checked in every "
       "saved context; Linux signal ABI evidence, not original P3 IDT frame "
       "qualification");
  return 0;
}
