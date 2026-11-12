/* Independent legacy protected32 descriptor-table store oracle. Only this
 * disposable ring0 test uses native LGDT/LIDT; player code must never do so. */
#include <stdint.h>
struct __attribute__((packed)) Table {
  uint16_t limit;
  uint32_t base;
};
static void byte(uint8_t value) {
  __asm__ volatile("outb %0,$0xe9" ::"a"(value));
}
static void word(uint32_t value) {
  for (unsigned i = 0; i < 4; ++i)
    byte((uint8_t)(value >> (i * 8)));
}
void string_ring0_main(void) {
  struct Table original_gdt, original_idt;
  __asm__ volatile("sgdt %0; sidt %1" : "=m"(original_gdt), "=m"(original_idt));
  const char magic[] = "GPKTBL01";
  for (unsigned i = 0; i < 8; ++i)
    byte((uint8_t)magic[i]);
  word(2448);
  for (unsigned op = 0; op < 2; ++op)
    for (unsigned bits16 = 0; bits16 < 2; ++bits16)
      for (unsigned b = 0; b < 34; ++b)
        for (unsigned l = 0; l < 18; ++l) {
          uint32_t base = b == 0 ? 0 : b == 1 ? UINT32_MAX : 1u << (b - 2);
          uint16_t limit = l == 0   ? 0
                           : l == 1 ? 65535
                                    : (uint16_t)(1u << (l - 2));
          struct Table input = {limit, base};
          struct {
            uint8_t before, data[6], after;
          } output;
          output.before = output.after = 0x5a;
          for (unsigned i = 0; i < 6; ++i)
            output.data[i] = 0xa5;
          if (op)
            __asm__ volatile("lidt %0" ::"m"(input) : "memory");
          else
            __asm__ volatile("lgdt %0" ::"m"(input) : "memory");
          if (op) {
            if (bits16)
              __asm__ volatile("data16 sidt %0" : "=m"(output.data)::"memory");
            else
              __asm__ volatile("sidt %0" : "=m"(output.data)::"memory");
          } else {
            if (bits16)
              __asm__ volatile("data16 sgdt %0" : "=m"(output.data)::"memory");
            else
              __asm__ volatile("sgdt %0" : "=m"(output.data)::"memory");
          }
          __asm__ volatile("lgdt %0; lidt %1" ::"m"(original_gdt),
                           "m"(original_idt)
                           : "memory");
          word(op);
          word(bits16);
          word(base);
          word(limit);
          for (unsigned i = 0; i < 6; ++i)
            byte(output.data[i]);
          byte(output.before);
          byte(output.after);
        }
  __asm__ volatile("outl %0,$0xf4" ::"a"(0));
  for (;;)
    __asm__ volatile("hlt");
}
