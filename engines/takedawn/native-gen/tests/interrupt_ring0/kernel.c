/* Independent privileged/PIT oracle. Shares flags_ring0 start/link harness;
 * no production CPU service, ROM, OS or player-side dependency. */
#include <stdint.h>
volatile uint32_t debug_count;
volatile uint32_t interrupt_marker, interrupt_count, interrupt_observed_marker,
    interrupt_observed_ip, interrupt_observed_flags, interrupt_expected_ip;
__attribute__((naked)) void interrupt_handler(void) {
  __asm__ volatile("pushal\n"
                   "incl interrupt_count\n"
                   "movl interrupt_marker,%eax\n"
                   "movl %eax,interrupt_observed_marker\n"
                   "movl 32(%esp),%eax\n"
                   "movl %eax,interrupt_observed_ip\n"
                   "movl 40(%esp),%eax\n"
                   "movl %eax,interrupt_observed_flags\n"
                   "movb $255,%al\n outb %al,$0x21\n"
                   "movb $32,%al\n outb %al,$0x20\n"
                   "popal\n iret");
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
static uint32_t flags(void) {
  uint32_t value;
  __asm__ volatile("pushfl;pop %0" : "=r"(value)::"memory");
  return value;
}
static uint32_t observe(uint32_t initial, unsigned op) {
  uint32_t value;
  if (op)
    __asm__ volatile(
        "push %[initial];popfl;sti;pushfl;pop %%edx;push $2;popfl;cld"
        : "=&d"(value)
        : [initial] "r"(initial)
        : "cc", "memory");
  else
    __asm__ volatile(
        "push %[initial];popfl;cli;pushfl;pop %%edx;push $2;popfl;cld"
        : "=&d"(value)
        : [initial] "r"(initial)
        : "cc", "memory");
  return value;
}
static void pending_pit(void) {
  __asm__ volatile("cli" ::: "memory");
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
  out(0x43, 0x30); /* Channel0, binary one-shot, low/high divisor. */
  out(0x40, 100);
  out(0x40, 0);
  out(0x20, 0x0a); /* Read pending requests, not in-service IRQs. */
  while (!(in(0x20) & 1)) {
  }
}
void flags_ring0_main(void) {
  out(0x21, 255);
  out(0xa1, 255);
  uint32_t handler = (uint32_t)(uintptr_t)interrupt_handler;
  idt[32] =
      (struct Gate){(uint16_t)handler, 8, 0, 0x8e, (uint16_t)(handler >> 16)};
  struct Idtr descriptor = {sizeof(idt) - 1, (uint32_t)(uintptr_t)idt};
  __asm__ volatile("lidt %0" ::"m"(descriptor) : "memory");
  const char magic[] = "GPKIRQv1";
  for (unsigned i = 0; i < 8; ++i)
    out(0xe9, magic[i]);
  word(2051);
  const uint32_t status_bits[7] = {1, 4, 16, 64, 128, 1024, 2048};
  for (unsigned iopl = 0; iopl < 4; ++iopl)
    for (unsigned enabled = 0; enabled < 2; ++enabled)
      for (unsigned status = 0; status < 128; ++status)
        for (unsigned op = 0; op < 2; ++op) {
          uint32_t initial = 2 | (iopl << 12) | (enabled << 9);
          for (unsigned bit = 0; bit < 7; ++bit)
            if (status & (1u << bit))
              initial |= status_bits[bit];
          uint32_t result = observe(initial, op);
          word(0);
          word(initial);
          word(op);
          word(result);
          for (unsigned j = 0; j < 4; ++j)
            word(0);
        }
  for (unsigned sequence = 0; sequence < 3; ++sequence) {
    pending_pit();
    interrupt_marker = interrupt_count = interrupt_observed_marker = 0;
    interrupt_observed_ip = interrupt_observed_flags = interrupt_expected_ip =
        0;
    uint32_t final_flags;
    if (sequence == 0)
      __asm__ volatile("push $2;popfl;movl $1f,interrupt_expected_ip;sti;"
                       "incl interrupt_marker;1:cli;pushfl;pop %0"
                       : "=r"(final_flags)::"cc", "memory");
    else if (sequence == 1)
      __asm__ volatile(
          "push $2;popfl;sti;cli;incl interrupt_marker;pushfl;pop %0"
          : "=r"(final_flags)::"cc", "memory");
    else
      __asm__ volatile("push $2;popfl;movl $1f,interrupt_expected_ip;sti;sti;"
                       "1:incl interrupt_marker;cli;pushfl;pop %0"
                       : "=r"(final_flags)::"cc", "memory");
    out(0x21, 255);
    word(1);
    word(sequence);
    word(interrupt_marker);
    word(interrupt_count);
    word(interrupt_observed_marker);
    word(interrupt_observed_ip - interrupt_expected_ip);
    word(interrupt_observed_flags);
    word(final_flags);
  }
  /* Keep this reference helper checked even though sequence capture is inline.
   */
  uint32_t result = !!(flags() & 0x200);
  __asm__ volatile("outl %0,%1" ::"a"(result), "Nd"((uint16_t)0xf4));
  for (;;)
    __asm__ volatile("hlt");
}
