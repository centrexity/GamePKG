/* Independent protected32 instruction oracle. No production runtime. */
#include "../atomic_inputs.h"
static void out_byte(uint8_t v) {
  __asm__ volatile("outb %0,%1" ::"a"(v), "Nd"((uint16_t)0xe9));
}
static void out_word(uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    out_byte((uint8_t)(v >> (i * 8)));
}
static void finish(uint32_t v) {
  __asm__ volatile("outl %0,%1" ::"a"(v), "Nd"((uint16_t)0xf4));
  for (;;)
    __asm__ volatile("hlt");
}
static void put(uint32_t at, uint8_t v) {
  __asm__ volatile("movb %b0,(%1)" ::"q"(v), "r"(at) : "memory");
}
static uint32_t get(uint32_t at) {
  uint32_t v;
  __asm__ volatile("movl (%1),%0" : "=r"(v) : "r"(at) : "memory");
  return v;
}
static uint32_t initial_flags, after_flags;
static struct AtomicRegs observe(unsigned f, struct AtomicRegs r) {
  initial_flags = r.flags;
#define EXECUTE(code)                                                          \
  __asm__ volatile("push %[initial];popfl;" code                               \
                   ";pushfl;pop %[after];push $2;popfl;cld"                    \
                   : "+a"(r.eax), "+b"(r.ebx), "+c"(r.ecx), "+S"(r.esi),       \
                     "+D"(r.edi), [after] "=m"(after_flags)                    \
                   : [initial] "m"(initial_flags)                              \
                   : "cc", "memory")
  switch (f) {
#include "variants.inc"
  default:
    finish(2);
  }
#undef EXECUTE
  r.flags = after_flags;
  return r;
}
void atomic_ring0_main(void) {
  __asm__ volatile("outb %0,%1" ::"a"((uint8_t)255), "Nd"((uint16_t)0x21));
  __asm__ volatile("outb %0,%1" ::"a"((uint8_t)255), "Nd"((uint16_t)0xa1));
  const char magic[] = "GPKATMv1";
  for (unsigned i = 0; i < 8; ++i)
    out_byte((uint8_t)magic[i]);
  out_word(59 * 1024);
  for (unsigned f = 0; f < 59; ++f)
    for (unsigned s = 0; s < 1024; ++s) {
      struct AtomicInput in = atomic_input(f, s);
      uint32_t at = in.base + 129;
      for (unsigned i = 0; i < 4; ++i)
        put(at + i, (uint8_t)(in.initial_memory >> (8 * i)));
      struct AtomicRegs r = observe(f, in.r);
      out_word(f);
      out_word(s);
      out_word(r.eax);
      out_word(r.ecx);
      out_word(r.ebx);
      out_word(r.esi);
      out_word(r.edi);
      out_word(r.flags);
      out_word(get(at));
    }
  finish(0);
}
