/* Independent direct-code far jumps in native ring0/ring3 under QEMU only.
 * No production planner/header linkage. Fixed code VMAs and normalized fault
 * sites keep output independent of compiler placement of the C harness. */
#include <stdint.h>
extern void far_case(uint32_t), far_target_stub(void), far_target_stub16(void),
    far_gp(void), far_np(void), far_bad(void), far_success(void);
extern void kernel_jump32_site(void), kernel_jump16_site(void),
    user_jump32_site(void), user_jump16_site(void);
uint32_t resume_sp, resume_ip, seen_vector, seen_error, seen_cs, seen_before,
    seen_after, seen_ip, operand16;
uint8_t far_pointer32[6], far_pointer16[4];
struct __attribute__((packed)) Table {
  uint16_t limit;
  uint32_t base;
};
struct __attribute__((packed)) Gate {
  uint16_t low, selector;
  uint8_t zero, type;
  uint16_t high;
};
static uint32_t gdt[32], ldt[4];
static uint8_t tss[104], kernel_stack[8192];
static struct Gate idt[256];
static void byte(uint8_t v) { __asm__ volatile("outb %0,$0xe9" ::"a"(v)); }
static void word(uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    byte((uint8_t)(v >> (8 * i)));
}
static void put(uint8_t *p, uint32_t v, unsigned n) {
  for (unsigned i = 0; i < n; ++i)
    p[i] = (uint8_t)(v >> (8 * i));
}
static void descriptor(uint32_t *p, uint32_t base, uint32_t limit,
                       uint32_t access, uint32_t flags) {
  p[0] = (limit & 65535) | (base << 16);
  p[1] = ((base >> 16) & 255) | (access << 8) | (limit & 0xf0000) |
         (flags << 20) | (base & 0xff000000);
}
static void gate(unsigned v, void (*handler)(void), unsigned type) {
  uint32_t at = (uint32_t)(uintptr_t)handler;
  idt[v] =
      (struct Gate){(uint16_t)at, 8, 0, (uint8_t)type, (uint16_t)(at >> 16)};
}
static uint32_t stub(unsigned wide) {
  return (uint32_t)(uintptr_t)(wide ? far_target_stub : far_target_stub16);
}
static void observe(unsigned cpl, unsigned selector, unsigned small,
                    uint32_t *candidate, unsigned usable, uint32_t offset) {
  operand16 = small;
  put(far_pointer32, offset, 4);
  put(far_pointer32 + 4, selector, 2);
  put(far_pointer16, offset, 2);
  put(far_pointer16 + 2, selector, 2);
  uint32_t lo = candidate[0], hi = candidate[1];
  seen_vector = 254;
  seen_error = seen_cs = seen_after = seen_before = seen_ip = 0;
  far_case(cpl);
  unsigned phase = 0;
  uint32_t normalized_ip = 0;
  if (seen_vector) {
    uint32_t source = (uint32_t)(uintptr_t)(cpl ? (small ? user_jump16_site
                                                         : user_jump32_site)
                                                : (small ? kernel_jump16_site
                                                         : kernel_jump32_site));
    if (seen_cs == (uint32_t[]){8, 65, 82, 35}[cpl] && seen_ip == source) {
      phase = 1;
      normalized_ip = 0xffff0000u | (cpl << 4) | small;
    } else if (seen_cs == ((selector & 0xfffcu) | cpl) &&
               seen_ip == (small ? (offset & 65535) : offset)) {
      phase = 2;
      normalized_ip = seen_ip;
    } else {
      phase = 255;
      normalized_ip = seen_ip;
    }
  }
  word(cpl);
  word(selector);
  word(small);
  word(small ? 0xa5a50000u | (offset & 65535) : offset);
  word(lo);
  word(hi);
  word(127);
  word(15);
  word(usable);
  word(seen_vector);
  word(seen_error);
  word(seen_cs);
  word(candidate[0]);
  word(candidate[1]);
  word(seen_before);
  word(seen_after);
  word(phase);
  word(normalized_ip);
}
void string_ring0_main(void) {
  __asm__ volatile("outb %%al,$0x21;outb %%al,$0xa1" ::"a"((uint8_t)255));
  descriptor(gdt + 2, 0, 0xfffff, 0x9b, 12);
  descriptor(gdt + 4, 0, 0xfffff, 0x93, 12);
  descriptor(gdt + 8, 0, 0xfffff, 0xfb, 12);
  descriptor(gdt + 10, 0, 0xfffff, 0xf3, 12);
  descriptor(gdt + 16, 0, 0xfffff, 0xbb, 12);
  descriptor(gdt + 18, 0, 0xfffff, 0xb3, 12);
  descriptor(gdt + 20, 0, 0xfffff, 0xdb, 12);
  descriptor(gdt + 22, 0, 0xfffff, 0xd3, 12);
  put(tss + 4, (uint32_t)(uintptr_t)(kernel_stack + sizeof(kernel_stack)), 4);
  put(tss + 8, 16, 4);
  put(tss + 102, sizeof(tss), 2);
  descriptor(gdt + 12, (uint32_t)(uintptr_t)tss, sizeof(tss) - 1, 0x89, 0);
  descriptor(gdt + 14, (uint32_t)(uintptr_t)ldt, sizeof(ldt) - 1, 0x82, 0);
  struct Table tables = {sizeof(gdt) - 1, (uint32_t)(uintptr_t)gdt},
               interrupts = {sizeof(idt) - 1, (uint32_t)(uintptr_t)idt};
  gate(11, far_np, 0x8e);
  gate(13, far_gp, 0x8e);
  gate(12, far_bad, 0x8e);
  gate(128, far_success, 0xee);
  __asm__ volatile(
      "lgdt %0;lidt %1;mov $48,%%ax;ltr %%ax;mov $56,%%ax;lldt %%ax" ::"m"(
          tables),
      "m"(interrupts)
      : "eax", "memory");
  const char magic[] = "GPKFAR02";
  for (unsigned i = 0; i < 8; ++i)
    byte((uint8_t)magic[i]);
  word(132816);
  for (unsigned table = 0; table < 2; ++table)
    for (unsigned ring = 0; ring < 4; ++ring)
      for (unsigned small = 0; small < 2; ++small)
        for (unsigned rpl = 0; rpl < 4; ++rpl)
          for (unsigned type = 0; type < 16; ++type)
            for (unsigned dpl = 0; dpl < 4; ++dpl)
              for (unsigned present = 0; present < 2; ++present)
                for (unsigned large = 0; large < 2; ++large)
                  for (unsigned reserved = 0; reserved < 4; ++reserved)
                    for (unsigned wide = 0; wide < 2; ++wide) {
                      uint32_t *p = table ? ldt + 2 : gdt + 6;
                      descriptor(p, stub(wide) - 0x1000,
                                 large ? 0xfffff : 0xfff,
                                 16 | type | (dpl << 5) | (present << 7),
                                 (wide ? 4 : 0) | reserved | (large ? 8 : 0));
                      observe(ring, (table ? 12 : 24) | rpl, small, p, 1,
                              0x1000);
                    }
  /* All base bits, including wrapping base+offset, use a 32-bit target stream.
   */
  for (unsigned table = 0; table < 2; ++table)
    for (unsigned ring = 0; ring < 4; ++ring)
      for (unsigned b = 0; b < 36; ++b) {
        uint32_t base = b == 0    ? 0
                        : b == 1  ? UINT32_MAX
                        : b == 34 ? 0x89abcdef
                        : b == 35 ? 0xfedcba98
                                  : 1u << (b - 2);
        uint32_t *p = table ? ldt + 2 : gdt + 6;
        descriptor(p, base, 0xfffff, (0x9a | (ring << 5)), 12);
        observe(ring, (table ? 12 : 24) | (ring), 0, p, 1, stub(1) - base);
      }
  /* Isolate all twenty limit bits and target-fetch faults after a valid jump.
   */
  for (unsigned table = 0; table < 2; ++table)
    for (unsigned ring = 0; ring < 4; ++ring)
      for (unsigned small = 0; small < 2; ++small)
        for (unsigned wide = 0; wide < 2; ++wide)
          for (unsigned gran = 0; gran < 2; ++gran)
            for (unsigned bit = 0; bit < 21; ++bit) {
              uint32_t *p = table ? ldt + 2 : gdt + 6;
              descriptor(p, stub(wide) - 0x1000, bit == 20 ? 0 : 1u << bit,
                         (0x9a | (ring << 5)), (wide ? 4 : 0) | (gran ? 8 : 0));
              observe(ring, (table ? 12 : 24) | (ring), small, p, 1, 0x1000);
            }
  static const unsigned invalid[] = {0,    1,      2,      3,    0x80,
                                     0x83, 0xfff8, 0xffff, 0x14, 0x17};
  for (unsigned ring = 0; ring < 4; ++ring)
    for (unsigned small = 0; small < 2; ++small)
      for (unsigned i = 0; i < 10; ++i) {
        uint32_t *p = invalid[i] & 4 ? ldt + 2 : gdt + 6;
        p[0] = p[1] = 0;
        observe(ring, invalid[i], small, p, 1, 0x1000);
      }
  __asm__ volatile("xor %%eax,%%eax;lldt %%ax" ::: "eax", "memory");
  for (unsigned ring = 0; ring < 4; ++ring)
    for (unsigned small = 0; small < 2; ++small)
      for (unsigned rpl = 0; rpl < 4; ++rpl) {
        ldt[2] = ldt[3] = 0;
        observe(ring, 12 | rpl, small, ldt + 2, 0, 0x1000);
      }
  __asm__ volatile("outl %0,$0xf4" ::"a"(0));
  for (;;)
    __asm__ volatile("hlt");
}
