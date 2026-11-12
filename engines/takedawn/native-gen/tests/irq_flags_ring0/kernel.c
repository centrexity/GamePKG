/* Independent protected-mode PIT IRQ gate oracle; no runtime or game code.
 * IRETD establishes RF/TF/NT/IOPL without POPFD's RF normalization.
 * The pending one-shot IRQ arrives at the following NOP boundary. */
#include <stdint.h>
volatile uint32_t observed_saved, observed_handler, observed_pc, expected_pc,
    observed_count;
__attribute__((naked)) void irq_handler(void) {
  __asm__ volatile("pushfl\n pushal\n"
                   "movl 32(%esp),%eax\n movl %eax,observed_handler\n"
                   "movl 44(%esp),%eax\n movl %eax,observed_saved\n"
                   "movl 36(%esp),%eax\n movl %eax,observed_pc\n"
                   "incl observed_count\n"
                   /* Do not restore NT/TF into our unpaged test harness. */
                   "andl $0xffffbeff,44(%esp)\n"
                   "movb $255,%al\n outb %al,$0x21\n"
                   "movb $32,%al\n outb %al,$0x20\n"
                   "popal\n addl $4,%esp\n iret");
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
static void out(uint16_t port, uint8_t value) {
  __asm__ volatile("outb %0,%1" ::"a"(value), "Nd"(port));
}
static uint8_t in(uint16_t port) {
  uint8_t value;
  __asm__ volatile("inb %1,%0" : "=a"(value) : "Nd"(port));
  return value;
}
static void word(uint32_t value) {
  for (unsigned i = 0; i < 4; ++i)
    out(0xe9, (uint8_t)(value >> (8 * i)));
}
static void pending_pit(void) {
  out(0x20, 0x11);
  out(0xa0, 0x11);
  out(0x21, 0x20);
  out(0xa1, 0x28);
  out(0x21, 4);
  out(0xa1, 2);
  out(0x21, 1);
  out(0xa1, 1);
  out(0x21, 0xfe);
  out(0xa1, 0xff);
  out(0x43, 0x30);
  out(0x40, 100);
  out(0x40, 0);
  out(0x20, 0x0a);
  while (!(in(0x20) & 1)) {
  }
}
void string_ring0_main(void) {
  out(0x21, 255);
  out(0xa1, 255);
  uint32_t handler = (uint32_t)(uintptr_t)irq_handler;
  struct Idtr descriptor = {sizeof(idt) - 1, (uint32_t)(uintptr_t)idt};
  __asm__ volatile("lidt %0" ::"m"(descriptor) : "memory");
  const char magic[] = "GPKIGF01";
  for (unsigned i = 0; i < 8; ++i)
    out(0xe9, magic[i]);
  word(8192);
  const uint32_t bits[] = {1, 4, 16, 64, 128, 1024, 2048};
  for (unsigned status = 0; status < 128; ++status)
    for (unsigned iopl = 0; iopl < 4; ++iopl)
      for (unsigned special = 0; special < 8; ++special)
        for (unsigned trap = 0; trap < 2; ++trap) {
          uint32_t initial =
              0x202 | (iopl << 12) | ((special & 1) ? 0x100 : 0) |
              ((special & 2) ? 0x4000 : 0) | ((special & 4) ? 0x10000 : 0);
          for (unsigned j = 0; j < 7; ++j)
            if (status & (1u << j))
              initial |= bits[j];
          idt[32] = (struct Gate){(uint16_t)handler, 8, 0,
                                  (uint8_t)(trap ? 0x8f : 0x8e),
                                  (uint16_t)(handler >> 16)};
          observed_count = observed_pc = observed_saved = observed_handler = 0;
          pending_pit();
          __asm__ volatile(
              "movl $1f,expected_pc;push %[initial];push $8;"
              "push $1f;iretl;1:nop;cli;push $2;popfl;cld" ::[initial] "r"(
                  initial)
              : "memory", "cc");
          word(initial);
          word(trap);
          word(observed_pc - expected_pc);
          word(observed_saved);
          word(observed_handler);
          word(observed_count);
        }
  uint32_t result = 0;
  __asm__ volatile("outl %0,%1" ::"a"(result), "Nd"((uint16_t)0xf4));
  for (;;)
    __asm__ volatile("hlt");
}
