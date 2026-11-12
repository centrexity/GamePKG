/* Actual ELF32 near transfers, simultaneous faults and LOOP counter state.
 * Only synthetic code/data are used. Output is an explicit LE CPU corpus. */
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
uint32_t near_physical_stack, near_physical_return, near_physical_cs,
    near_physical_ss, near_physical_flags;
extern void near_physical_case(const void *, uint32_t, uint32_t, uint32_t,
                               uint32_t, uint32_t);
static volatile sig_atomic_t observed;
static uint32_t vector, error, ip, cs, sp, cx, ax, flags, address;
static void handler(int sig, siginfo_t *info, void *raw) {
  ucontext_t *c = raw;
  assert(!observed && (sig == SIGSEGV || sig == SIGBUS || sig == SIGILL));
  observed = 1;
  vector = (uint32_t)c->uc_mcontext.gregs[REG_TRAPNO];
  error = (uint32_t)c->uc_mcontext.gregs[REG_ERR];
  ip = (uint32_t)c->uc_mcontext.gregs[REG_EIP];
  cs = (uint32_t)c->uc_mcontext.gregs[REG_CS] & 65535;
  sp = (uint32_t)c->uc_mcontext.gregs[REG_ESP];
  cx = (uint32_t)c->uc_mcontext.gregs[REG_ECX];
  ax = (uint32_t)c->uc_mcontext.gregs[REG_EAX];
  flags = (uint32_t)c->uc_mcontext.gregs[REG_EFL];
  address = vector == 14 ? (uint32_t)(uintptr_t)info->si_addr : 0;
  c->uc_mcontext.gregs[REG_CS] = (greg_t)near_physical_cs;
  c->uc_mcontext.gregs[REG_SS] = (greg_t)near_physical_ss;
  c->uc_mcontext.gregs[REG_EIP] = (greg_t)near_physical_return;
  c->uc_mcontext.gregs[REG_ESP] = (greg_t)near_physical_stack;
}
static void *mapped(uint32_t base, size_t bytes, int protection) {
  void *p = mmap((void *)(uintptr_t)base, bytes, protection,
                 MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0);
  assert(p != (void *)-1 && (uintptr_t)p == base);
  return p;
}
static void put(unsigned char *p, uint32_t n) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(n >> (i * 8));
}
static void word(FILE *f, uint32_t n) {
  unsigned char p[4];
  put(p, n);
  assert(fwrite(p, 1, 4, f) == 4);
}
static unsigned encode(unsigned kind, uint32_t target, unsigned char b[8]) {
  memset(b, 0, 8);
  switch (kind) {
  case 0:
    b[0] = 0xe8;
    put(b + 1, target - 5);
    return 5;
  case 1:
    b[0] = 0xff;
    b[1] = 0xd7;
    return 2;
  case 2:
    b[0] = 0xff;
    b[1] = 0x16;
    return 2;
  case 3:
    b[0] = 0xff;
    b[1] = 0x14;
    b[2] = 0x24;
    return 3;
  case 4:
    b[0] = 0xff;
    b[1] = 0x54;
    b[2] = 0x24;
    b[3] = 0xfc;
    return 4;
  case 5:
    b[0] = 0xff;
    b[1] = 0xd4;
    return 2;
  case 6:
    b[0] = 0xe9;
    put(b + 1, target - 5);
    return 5;
  case 7:
    b[0] = 0xff;
    b[1] = 0xe7;
    return 2;
  case 8:
    b[0] = 0xff;
    b[1] = 0x26;
    return 2;
  case 9:
    b[0] = 0xeb;
    b[1] = (unsigned char)(target - 2);
    return 2;
  case 10:
    b[0] = 0x74;
    b[1] = (unsigned char)(target - 2);
    return 2;
  case 11:
    b[0] = 0x75;
    b[1] = (unsigned char)(target - 2);
    return 2;
  case 12:
    b[0] = 0x0f;
    b[1] = 0x84;
    put(b + 2, target - 6);
    return 6;
  case 13:
    b[0] = 0xe3;
    b[1] = (unsigned char)(target - 2);
    return 2;
  case 14:
    b[0] = 0x67;
    b[1] = 0xe3;
    b[2] = (unsigned char)(target - 3);
    return 3;
  case 15:
  case 16:
  case 17:
    b[0] = (unsigned char)(kind == 15 ? 0xe2 : kind == 16 ? 0xe1 : 0xe0);
    b[1] = (unsigned char)(target - 2);
    return 2;
  case 18:
    b[0] = 0x67;
    b[1] = 0xe2;
    b[2] = (unsigned char)(target - 3);
    return 3;
  case 19:
    b[0] = 0xc3;
    return 1;
  case 20:
    b[0] = 0xc2;
    b[1] = 8;
    return 3;
  case 21:
    b[0] = 0x66;
    b[1] = 0xc3;
    return 2;
  default:
    assert(kind == 22);
    b[0] = 0x66;
    b[1] = 0xc2;
    b[2] = 8;
    return 4;
  }
}
int main(int argc, char **argv) {
  assert(argc == 2);
  _Static_assert(sizeof(void *) == 4, "ELF32 required");
  void *alternate = mmap(NULL, 65536, PROT_READ | PROT_WRITE,
                         MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  assert(alternate != MAP_FAILED);
  stack_t alt = {.ss_sp = alternate, .ss_size = 65536};
  assert(!sigaltstack(&alt, NULL));
  struct sigaction action;
  memset(&action, 0, sizeof(action));
  action.sa_sigaction = handler;
  action.sa_flags = SA_SIGINFO | SA_ONSTACK;
  assert(!sigemptyset(&action.sa_mask) && !sigaction(SIGSEGV, &action, NULL) &&
         !sigaction(SIGBUS, &action, NULL) &&
         !sigaction(SIGILL, &action, NULL));
  unsigned char *code = mapped(0x10000000, 4096, PROT_READ | PROT_WRITE);
  unsigned char *arena = mapped(0x11000000, 12288, PROT_NONE),
                *stack = arena + 4096;
  unsigned char *operand = mapped(0x12000000, 4096, PROT_READ | PROT_WRITE);
  struct {
    uint32_t index, base, limit, flags;
  } desc = {0, 0x10000000, 127, 0x45};
  assert(syscall(SYS_modify_ldt, 1, &desc, sizeof(desc)) == 0);
  const unsigned char far_pointer[6] = {0, 0, 0, 0, 7, 0};
  const uint32_t counts[] = {0, 1, 2, 0x10000, 0x10001, 0x80000000, UINT32_MAX};
  const uint32_t total = 23 * 3 * 6 * 7 * 2;
  FILE *out = fopen(argv[1], "wb");
  assert(out && fwrite("GPKNEAR1", 1, 8, out) == 8);
  word(out, total);
  unsigned seen[15] = {0}, gp_loop_changed = 0, gp_loop_unchanged = 0;
  for (unsigned kind = 0; kind < 23; ++kind)
    for (unsigned bad = 0; bad < 3; ++bad)
      for (unsigned mode = 0; mode < 6; ++mode)
        for (unsigned count = 0; count < 7; ++count)
          for (unsigned zf = 0; zf < 2; ++zf) {
            uint32_t target = bad == 0   ? 64
                              : bad == 1 ? 128
                                         : 127,
                     initial_sp = mode == 2   ? 0x11001000
                                  : mode == 3 ? 1
                                  : mode == 5 ? 0x11000ffc
                                              : 0x11001040;
            assert(!mprotect(code, 4096, PROT_READ | PROT_WRITE));
            memset(code, 0xcc, 4096);
            unsigned char bytes[8];
            unsigned size = encode(kind, target, bytes);
            memcpy(code, bytes, size);
            code[size] = 0x0f;
            code[size + 1] = 0x0b;
            code[64] = 0x0f;
            code[65] = 0x0b;
            code[127] = 0x0f;
            code[128] = 0x0b;
            assert(!mprotect(code, 4096, PROT_READ | PROT_EXEC));
            assert(!mprotect(stack, 4096, PROT_READ | PROT_WRITE));
            memset(stack, 0xa5, 4096);
            if (initial_sp >= 0x11001000 && initial_sp <= 0x11001ffc)
              put(stack + (initial_sp - 0x11001000), target);
            if (initial_sp >= 0x11001004 && initial_sp <= 0x11002000)
              put(stack + (initial_sp - 4 - 0x11001000), target);
            if (mode == 1)
              assert(!mprotect(stack, 4096, PROT_READ));
            assert(!mprotect(operand, 4096, PROT_READ | PROT_WRITE));
            put(operand, target);
            if (mode == 4)
              assert(!mprotect(operand, 4096, PROT_NONE));
            observed = 0;
            near_physical_case(far_pointer, initial_sp, counts[count],
                               0x202 | (zf ? 0x40 : 0), 0x12000000, target);
            assert(
                observed && cs == 7 && ax == 0x11223344 &&
                (vector == 6 || vector == 12 || vector == 13 || vector == 14) &&
                (flags & 0x10000));
            assert(vector == 6 ? (ip == 64 || ip == size)
                               : (ip == 0 || (vector == 13 && ip == 127)));
            if (vector != 6 && ip == 0)
              assert(sp == initial_sp && cx == counts[count]);
            ++seen[vector];
            if (vector == 13 && ip == 0 && kind >= 15 && kind <= 18) {
              if (cx == counts[count])
                ++gp_loop_unchanged;
              else
                ++gp_loop_changed;
            }
            uint32_t window = initial_sp == 0x11001040 ? 56 : 0;
            uint32_t fields[] = {kind,
                                 size,
                                 target,
                                 mode,
                                 initial_sp,
                                 counts[count],
                                 near_physical_flags,
                                 window,
                                 vector,
                                 error,
                                 ip,
                                 sp,
                                 cx,
                                 ax,
                                 flags,
                                 address};
            for (unsigned i = 0; i < 16; ++i)
              word(out, fields[i]);
            assert(fwrite(bytes, 1, 8, out) == 8 &&
                   fwrite(stack + window, 1, 32, out) == 32);
          }
  assert(!fclose(out));
  printf("Native near corpus:%u records; UD completion=%u SS=%u GP=%u PF=%u; "
         "faulting LOOP counters changed=%u unchanged=%u\n",
         total, seen[6], seen[12], seen[13], seen[14], gp_loop_changed,
         gp_loop_unchanged);
  desc.flags = 0x28;
  desc.base = desc.limit = 0;
  assert(syscall(SYS_modify_ldt, 1, &desc, sizeof(desc)) == 0);
  assert(!munmap(code, 4096) && !munmap(arena, 12288) &&
         !munmap(operand, 4096));
  stack_t disabled = {.ss_flags = SS_DISABLE};
  assert(!sigaltstack(&disabled, NULL) && !munmap(alternate, 65536));
  return 0;
}
