/* Deterministic inputs only. Expected output is recorded from CPU instructions.
 */
#ifndef GAMEPKG_ATOMIC_INPUTS_H
#define GAMEPKG_ATOMIC_INPUTS_H
#include <stdint.h>
struct AtomicRegs {
  uint32_t eax, ecx, ebx, esi, edi, flags;
};
struct AtomicInput {
  struct AtomicRegs r;
  uint32_t initial_memory, base, bytes;
};
static struct AtomicInput atomic_input(unsigned f, unsigned scenario) {
  unsigned bytes = f < 54    ? 1u << ((f % 18) / 6)
                   : f < 57  ? 1
                   : f == 57 ? 4
                             : 2,
           mode = f < 54 ? f % 6 : 0;
  uint32_t a = scenario * 0x9e3779b9u, b = scenario * 0xdeadbeefu + 0x80000000u;
  static const uint32_t edge[] = {0,          1,          0x7f,      0x80,
                                  0xff,       0x7fff,     0x8000,    0xffff,
                                  0x7fffffff, 0x80000000, 0xffffffff};
  if (scenario < 121) {
    a = edge[scenario / 11];
    b = edge[scenario % 11];
  }
  uint32_t acc = (scenario & 1) ? a : a ^ 0x80;
  struct AtomicInput in = {{acc, b, mode == 3 ? 0xabcd0040 : a,
                            mode == 3 ? 0xdef00041 : 0x16161616, 0x4081,
                            2 | ((scenario & 1) ? 0xcd5u : 0x810u)},
                           a,
                           mode == 3   ? 0
                           : mode == 2 ? 0x6000
                                       : 0x4000,
                           bytes};
  return in;
}
#endif
