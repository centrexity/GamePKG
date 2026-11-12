/* Deterministic input construction shared by independent CPU producer and
 * consumers. This header never computes an instruction result. */
#ifndef GAMEPKG_TEST_STRING_INPUTS_H
#define GAMEPKG_TEST_STRING_INPUTS_H
#include <stdint.h>
#define STRING_RAM_SIZE 0x14000u
struct StringRegs {
  uint32_t eax, ecx, esi, edi, flags;
};
struct StringInput {
  uint32_t op, bytes, a16, repeat, fs, scenario;
  struct StringRegs regs;
};
static unsigned string_steps(const struct StringInput *in) {
  return in->repeat ? (in->regs.ecx & (in->a16 ? 65535u : UINT32_MAX)) : 1;
}
static uint32_t string_position(uint32_t index, unsigned n,
                                const struct StringInput *in) {
  uint32_t delta = n * in->bytes;
  if (in->regs.flags & 0x400)
    delta = 0u - delta;
  return (index + delta) & (in->a16 ? 65535u : UINT32_MAX);
}
static void string_input(struct StringInput *in, unsigned op, unsigned bytes,
                         unsigned a16, unsigned repeat, unsigned fs,
                         unsigned scenario) {
  *in = (struct StringInput){.op = op,
                             .bytes = bytes,
                             .a16 = a16,
                             .repeat = repeat,
                             .fs = fs,
                             .scenario = scenario};
  unsigned count = scenario == 0   ? 0
                   : scenario == 1 ? 1
                   : scenario == 2 ? 2
                                   : 17;
  unsigned df = (scenario >> 1) & 1;
  in->regs = (struct StringRegs){.eax = 0x89abcdefu ^ scenario,
                                 .ecx = count,
                                 .esi = 0x4000,
                                 .edi = 0x8000,
                                 .flags = 2u | ((scenario & 1) ? 0 : 0x8d5u) |
                                          (df << 10)};
  if (scenario == 5)
    in->regs.edi = in->regs.esi + (fs ? 0x2000 : 0) + 1;
  if (scenario == 6)
    in->regs.edi = in->regs.esi + (fs ? 0x2000 : 0) - 1;
  if (scenario == 7)
    in->regs.edi = in->regs.esi + (fs ? 0x2000 : 0);
  if (a16) {
    if (scenario == 12)
      in->regs.esi = 0xffff;
    if (scenario == 13)
      in->regs.edi = 0xffff;
    if (scenario == 14)
      in->regs.esi = 0;
    in->regs.esi |= 0xabcd0000;
    in->regs.edi |= 0x12340000;
    in->regs.ecx |= 0x76540000;
  } else if (fs && scenario == 15)
    in->regs.esi = 0xfffff000;
  if (repeat && scenario == 0) {
    in->regs.esi = UINT32_MAX;
    in->regs.edi = UINT32_MAX;
  }
}
static int string_initial_memory(unsigned char *data,
                                 const struct StringInput *in) {
  for (uint32_t i = 0; i < STRING_RAM_SIZE; ++i)
    data[i] = (unsigned char)(i * 17u + (i >> 8) * 31u + in->scenario * 73u);
  if (in->scenario >= 8 && in->scenario <= 11 && in->op >= 3) {
    unsigned n = string_steps(in);
    for (unsigned i = 0; i < n; ++i) {
      uint32_t s = string_position(in->regs.esi, i, in) + (in->fs ? 0x2000 : 0),
               d = string_position(in->regs.edi, i, in);
      if (d > STRING_RAM_SIZE - in->bytes ||
          (in->op == 3 && s > STRING_RAM_SIZE - in->bytes))
        return 0;
      uint32_t value = in->op == 3 ? (0x51738ba5u + i * 17) : in->regs.eax;
      unsigned equal = in->scenario == 8    ? i != 0
                       : in->scenario == 9  ? i + 1 != n
                       : in->scenario == 10 ? i == 0
                                            : i + 1 == n;
      for (unsigned j = 0; j < in->bytes; ++j) {
        if (in->op == 3)
          data[s + j] = (unsigned char)(value >> (8 * j));
        data[d + j] = (unsigned char)((value ^ (!equal ? 1u : 0u)) >> (8 * j));
      }
    }
  }
  return 1;
}
#endif
