/* Independent maintainer-only 32bit protected-mode CPL0 instruction oracle.
 * No production runtime, numerical library, ROM, OS or player dependency. */
#include <stdint.h>
volatile uint32_t debug_count;
extern void debug_handler(void);
struct __attribute__((packed)) Gate {
  uint16_t low, selector;
  uint8_t zero, type;
  uint16_t high;
};
static struct Gate idt[256];
struct __attribute__((packed)) Idtr {
  uint16_t limit;
  uint32_t base;
};
static void out_byte(uint16_t port, uint8_t value) {
  __asm__ volatile("outb %0,%1" ::"a"(value), "Nd"(port));
}
static void word(uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    out_byte(0xe9, (uint8_t)(v >> (8 * i)));
}
static uint32_t observe(uint32_t initial, uint32_t value, unsigned bytes) {
  uint32_t result;
  if (bytes == 4)
    __asm__ volatile("push %[initial];popfl;push %[value];popfl;pushfl;pop "
                     "%%edx;push $2;popfl;cld"
                     : "=&d"(result)
                     : [initial] "r"(initial), [value] "r"(value)
                     : "cc", "memory");
  else
    __asm__ volatile("push %[initial];popfl;pushw %w[value];popfw;pushfl;pop "
                     "%%edx;push $2;popfl;cld"
                     : "=&d"(result)
                     : [initial] "r"(initial), [value] "r"(value)
                     : "cc", "memory");
  return result;
}
void flags_ring0_main(void) {
  out_byte(0x21, 255);
  out_byte(0xa1, 255);
  uint32_t handler = (uint32_t)(uintptr_t)debug_handler;
  idt[1] =
      (struct Gate){(uint16_t)handler, 8, 0, 0x8e, (uint16_t)(handler >> 16)};
  struct Idtr descriptor = {sizeof(idt) - 1, (uint32_t)(uintptr_t)idt};
  __asm__ volatile("lidt %0" ::"m"(descriptor) : "memory");
  const char magic[] = "GPKFLGv1";
  for (unsigned i = 0; i < 8; ++i)
    out_byte(0xe9, (uint8_t)magic[i]);
  word(524288);
  for (unsigned bytes = 2; bytes <= 4; bytes += 2)
    for (unsigned old = 0; old < 4; ++old)
      for (uint32_t low = 0; low < 65536; ++low) {
        uint32_t initial = 2 | ((old & 1) << 18) | ((old >> 1) << 21);
        uint32_t value = low | (((low * 40503u + old * 37u) ^ 0xa5a5u) << 16);
        uint32_t actual = observe(initial, value, bytes);
        word(bytes);
        word(initial);
        word(value);
        word(actual);
      }
  uint32_t result = debug_count == 262144 ? 0 : 1;
  __asm__ volatile("outl %0,%1" ::"a"(result), "Nd"((uint16_t)0xf4));
  for (;;)
    __asm__ volatile("hlt");
}
