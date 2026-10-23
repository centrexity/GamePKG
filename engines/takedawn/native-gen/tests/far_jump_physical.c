/* Actual ELF32 code-segment fetch boundary, no game media. A process-local
 * LDT code segment admits EIP0, but its first MOV imm32 crosses limit0. */
#define _GNU_SOURCE 1
#include <assert.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <ucontext.h>
#include <unistd.h>
#if !defined(__i386__)
#error actual i386 process required
#endif
uint32_t far_physical_stack, far_physical_return, far_physical_cs;
extern void far_physical_case(const void *pointer);
extern void far_physical_jump_site(void);
static volatile sig_atomic_t observed;
static uint32_t vector, error, cs, ip, eax, flags;
static void handler(int signal, siginfo_t *info, void *opaque) {
  (void)info;
  ucontext_t *c = opaque;
  assert(signal == SIGSEGV && !observed);
  observed = 1;
  vector = (uint32_t)c->uc_mcontext.gregs[REG_TRAPNO];
  error = (uint32_t)c->uc_mcontext.gregs[REG_ERR];
  cs = (uint32_t)c->uc_mcontext.gregs[REG_CS] & 65535;
  ip = (uint32_t)c->uc_mcontext.gregs[REG_EIP];
  eax = (uint32_t)c->uc_mcontext.gregs[REG_EAX];
  flags = (uint32_t)c->uc_mcontext.gregs[REG_EFL];
  c->uc_mcontext.gregs[REG_CS] = (greg_t)far_physical_cs;
  c->uc_mcontext.gregs[REG_EIP] = (greg_t)far_physical_return;
  c->uc_mcontext.gregs[REG_ESP] = (greg_t)far_physical_stack;
}
int main(void) {
  _Static_assert(sizeof(void *) == 4, "ELF32 required");
  struct sigaction action;
  memset(&action, 0, sizeof(action));
  action.sa_sigaction = handler;
  action.sa_flags = SA_SIGINFO;
  assert(!sigemptyset(&action.sa_mask) && !sigaction(SIGSEGV, &action, NULL));
  unsigned char *code = mmap(NULL, 4096, PROT_READ | PROT_WRITE,
                             MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  assert(code != MAP_FAILED);
  static const unsigned char bytes[] = {0xb8, 0x78, 0x56, 0x34,
                                        0x12, 0x0f, 0x0b};
  memcpy(code, bytes, sizeof(bytes));
  assert(!mprotect(code, 4096, PROT_READ | PROT_EXEC));
  struct {
    uint32_t index, base, limit, flags;
  } descriptor = {0, (uint32_t)(uintptr_t)code, 0, 0x45};
  assert(syscall(SYS_modify_ldt, 1, &descriptor, sizeof(descriptor)) == 0);
  uint32_t before_descriptor[2], after_descriptor[2];
  assert(syscall(SYS_modify_ldt, 0, before_descriptor,
                 sizeof(before_descriptor)) == 8);
  unsigned char pointer[6] = {0, 0, 0, 0, 7, 0};
  far_physical_case(pointer);
  assert(observed && vector == 13 && error == 0 && cs == 7 && ip == 0 &&
         eax == 0x11223344);
  assert((flags & 0x10000u) != 0);
  assert(syscall(SYS_modify_ldt, 0, after_descriptor,
                 sizeof(after_descriptor)) == 8);
  assert(before_descriptor[0] == after_descriptor[0] &&
         after_descriptor[1] == (before_descriptor[1] | 0x100u));
  printf("Native LDT accessed observation: before=%08x after=%08x\n",
         before_descriptor[1], after_descriptor[1]);
  observed = 0;
  assert(!mprotect(code, 4096, PROT_READ | PROT_WRITE));
  code[0] = 0x40;
  assert(!mprotect(code, 4096, PROT_READ | PROT_EXEC));
  far_physical_case(pointer);
  assert(observed && vector == 13 && error == 0 && cs == 7 && ip == 1 &&
         eax == 0x11223345 && (flags & 0x10000u));
  observed = 0;
  pointer[0] = 1;
  far_physical_case(pointer);
  assert(observed && vector == 13 && error == 0 && cs == far_physical_cs &&
         ip == (uint32_t)(uintptr_t)far_physical_jump_site &&
         eax == 0x11223344 && (flags & 0x10000u));
  descriptor.flags = 0x28;
  descriptor.base = descriptor.limit = 0;
  assert(syscall(SYS_modify_ldt, 1, &descriptor, sizeof(descriptor)) == 0);
  assert(!munmap(code, 4096));
  puts("Physical ELF32 far jumps: transfer-limit failure retains CS; valid "
       "transfer commits CS; crossing fetch faults before MOV; one-byte INC "
       "retires before next fetch fault; saved RF1 passed");
  return 0;
}
