/* Independent protected32 CPU instruction results. No production runtime. */
#include "../string_inputs.h"
static unsigned char initial_memory[STRING_RAM_SIZE];
static void out_byte(uint8_t v) {
  __asm__ volatile("outb %0,%1" ::"a"(v), "Nd"((uint16_t)0xe9));
}
static void out_word(uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    out_byte((uint8_t)(v >> (8 * i)));
}
static void exit_test(uint32_t v) {
  __asm__ volatile("outl %0,%1" ::"a"(v), "Nd"((uint16_t)0xf4));
  for (;;)
    __asm__ volatile("hlt");
}
static void physical_write(uint32_t a, uint8_t v) {
  __asm__ volatile("movb %b0,(%1)" ::"q"(v), "r"(a) : "memory");
}
static uint8_t physical_read(uint32_t a) {
  uint8_t v;
  __asm__ volatile("movb (%1),%b0" : "=q"(v) : "r"(a) : "memory");
  return v;
}
static struct StringRegs observe(const struct StringInput *in) {
  struct StringRegs r = in->regs;
  uint32_t flags;
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
  __asm__ volatile("push %[initial];popfl;" code                               \
                   ";pushfl;pop %[after];push $2;popfl;cld"                    \
                   : "+a"(r.eax), "+c"(r.ecx), "+S"(r.esi),                    \
                     "+D"(r.edi), [after] "=m"(flags)                          \
                   : [initial] "m"(in->regs.flags)                             \
                   : "cc", "memory")
  switch (key) {
#include "variants.inc"
  default:
    exit_test(2);
  }
#undef EXECUTE
  r.flags = flags;
  return r;
}
void string_ring0_main(void) {
  __asm__ volatile("outb %0,%1" ::"a"((uint8_t)255), "Nd"((uint16_t)0x21));
  __asm__ volatile("outb %0,%1" ::"a"((uint8_t)255), "Nd"((uint16_t)0xa1));
  const char magic[] = "GPKSTRv1";
  for (unsigned i = 0; i < 8; ++i)
    out_byte((uint8_t)magic[i]);
  out_word(2304);
  for (unsigned op = 0; op < 5; ++op)
    for (unsigned w = 0; w < 3; ++w)
      for (unsigned a16 = 0; a16 < 2; ++a16)
        for (unsigned repeat = 0; repeat < 3; ++repeat)
          for (unsigned fs = 0; fs < 2; ++fs) {
            if (op < 3 && repeat == 2)
              continue;
            for (unsigned scenario = 0; scenario < 16; ++scenario) {
              struct StringInput in;
              string_input(&in, op, 1u << w, a16, repeat, fs, scenario);
              if (!string_initial_memory(initial_memory, &in))
                exit_test(3);
              for (uint32_t i = 0; i < STRING_RAM_SIZE; ++i)
                physical_write(i, initial_memory[i]);
              struct StringRegs after = observe(&in);
              out_word(op);
              out_word(in.bytes);
              out_word(a16);
              out_word(repeat);
              out_word(fs);
              out_word(scenario);
              out_word(in.regs.eax);
              out_word(in.regs.ecx);
              out_word(in.regs.esi);
              out_word(in.regs.edi);
              out_word(in.regs.flags);
              out_word(after.eax);
              out_word(after.ecx);
              out_word(after.esi);
              out_word(after.edi);
              out_word(after.flags);
              unsigned size = op < 2 ? string_steps(&in) * in.bytes : 0;
              out_word(size);
              for (unsigned i = 0; i < 68; ++i) {
                uint8_t v = 0;
                if (i < size) {
                  uint32_t a = string_position(in.regs.edi, i / in.bytes, &in) +
                               i % in.bytes;
                  v = physical_read(a);
                }
                out_byte(v);
              }
            }
          }
  exit_test(0);
}
