/* Independent P3 protected-mode prefix-state oracle. TF traps select exact
 * completed REP iterations; no production runtime or game data is linked. */
#include "../string_inputs.h"
volatile uint32_t prefix_seen, prefix_goal, prefix_start, prefix_finish,
    prefix_stopped;
static unsigned char initial_memory[STRING_RAM_SIZE];
__attribute__((naked)) void prefix_debug(void) {
  __asm__ volatile("pushal\n incl prefix_seen\n"
                   "movl 32(%esp),%eax\n cmpl prefix_start,%eax\n jne 2f\n"
                   "movl prefix_seen,%eax\n cmpl prefix_goal,%eax\n jb 3f\n"
                   "movl prefix_finish,%eax\n movl %eax,32(%esp)\n"
                   "movl $1,prefix_stopped\n"
                   "2:andl $0xfffffeff,40(%esp)\n"
                   "3:popal\n iret");
}
struct __attribute__((packed)) Gate {
  uint16_t low, selector;
  uint8_t zero, type;
  uint16_t high;
};
struct __attribute__((packed)) Idtr {
  uint16_t limit;
  uint32_t base;
};
static struct Gate idt[256];
static void byte(uint8_t v) {
  __asm__ volatile("outb %0,%1" ::"a"(v), "Nd"((uint16_t)0xe9));
}
static void word(uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    byte((uint8_t)(v >> (8 * i)));
}
static void end(uint32_t v) {
  __asm__ volatile("outl %0,%1" ::"a"(v), "Nd"((uint16_t)0xf4));
  for (;;)
    __asm__ volatile("hlt");
}
static void store(uint32_t p, uint8_t v) {
  __asm__ volatile("movb %b0,(%1)" ::"q"(v), "r"(p) : "memory");
}
static uint8_t load(uint32_t p) {
  uint8_t v;
  __asm__ volatile("movb (%1),%b0" : "=q"(v) : "r"(p) : "memory");
  return v;
}
static struct StringRegs observe(const struct StringInput *in) {
  struct StringRegs r = in->regs;
  uint32_t flags, initial = in->regs.flags | 0x100;
  unsigned key = ((((in->op * 3 + (in->bytes == 1   ? 0
                                   : in->bytes == 2 ? 1
                                                    : 2)) *
                        2 +
                    in->a16) *
                       3 +
                   in->repeat) *
                      2 +
                  in->fs);
#define EXECUTE(code)                                                          \
  __asm__ volatile("movl $1f,prefix_start;movl $2f,prefix_finish;push "        \
                   "%[initial];popfl;1:" code                                  \
                   ";2:pushfl;pop %[after];push $2;popfl;cld"                  \
                   : "+a"(r.eax), "+c"(r.ecx), "+S"(r.esi),                    \
                     "+D"(r.edi), [after] "=m"(flags)                          \
                   : [initial] "m"(initial)                                    \
                   : "cc", "memory")
  switch (key) {
#include "variants.inc"
  default:
    end(2);
  }
#undef EXECUTE
  r.flags = flags;
  return r;
}
void string_ring0_main(void) {
  __asm__ volatile("outb %0,%1" ::"a"((uint8_t)255), "Nd"((uint16_t)0x21));
  __asm__ volatile("outb %0,%1" ::"a"((uint8_t)255), "Nd"((uint16_t)0xa1));
  uint32_t handler = (uint32_t)(uintptr_t)prefix_debug;
  idt[1] =
      (struct Gate){(uint16_t)handler, 8, 0, 0x8e, (uint16_t)(handler >> 16)};
  struct Idtr d = {sizeof(idt) - 1, (uint32_t)(uintptr_t)idt};
  __asm__ volatile("lidt %0" ::"m"(d) : "memory");
  const char magic[] = "GPKSIP01";
  for (unsigned i = 0; i < 8; ++i)
    byte(magic[i]);
  word(17556);
  unsigned rows = 0;
  for (unsigned op = 0; op < 5; ++op)
    for (unsigned w = 0; w < 3; ++w)
      for (unsigned a16 = 0; a16 < 2; ++a16)
        for (unsigned rep = 1; rep < 3; ++rep)
          for (unsigned fs = 0; fs < 2; ++fs) {
            if (op < 3 && rep == 2)
              continue;
            for (unsigned scenario = 0; scenario < 16; ++scenario) {
              struct StringInput in;
              string_input(&in, op, 1u << w, a16, rep, fs, scenario);
              for (unsigned cut = 1; cut < string_steps(&in); ++cut) {
                if (!string_initial_memory(initial_memory, &in))
                  end(3);
                for (uint32_t i = 0; i < STRING_RAM_SIZE; ++i)
                  store(i, initial_memory[i]);
                prefix_seen = prefix_stopped = 0;
                prefix_goal = cut;
                struct StringRegs after = observe(&in);
                if (!prefix_seen || prefix_seen > cut ||
                    (op < 3 && (!prefix_stopped || prefix_seen != cut)))
                  end(4);
                word(op);
                word(in.bytes);
                word(a16);
                word(rep);
                word(fs);
                word(scenario);
                word(cut);
                word(in.regs.eax);
                word(in.regs.ecx);
                word(in.regs.esi);
                word(in.regs.edi);
                word(in.regs.flags);
                word(after.eax);
                word(after.ecx);
                word(after.esi);
                word(after.edi);
                word(after.flags);
                word(prefix_stopped);
                word(prefix_seen);
                unsigned size = op < 2 ? prefix_seen * in.bytes : 0;
                word(size);
                for (unsigned i = 0; i < 68; ++i)
                  byte(i < size ? load(string_position(in.regs.edi,
                                                       i / in.bytes, &in) +
                                       i % in.bytes)
                                : 0);
                ++rows;
              }
            }
          }
  end(rows == 17556 ? 0 : 5);
}
