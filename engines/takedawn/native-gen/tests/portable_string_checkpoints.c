/* Replay independent P3 debug-trap prefix states, then resume to independent
 * whole-instruction CPU results. No native x86 instructions in this consumer.
 */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_string.h"
#include "string_inputs.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static unsigned char ram[STRING_RAM_SIZE], expected[STRING_RAM_SIZE];
static struct StringInput input;
static unsigned goal, seen;
static int stopped;
struct Full {
  unsigned valid;
  uint32_t after[5], size;
  unsigned char bytes[68];
};
static struct Full full[180][16];
static uint32_t le32(const unsigned char *p) {
  return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) |
         ((uint32_t)p[3] << 24);
}
static unsigned key(unsigned op, unsigned width, unsigned a16, unsigned repeat,
                    unsigned fs) {
  assert(op < 5 && (width == 1 || width == 2 || width == 4) && a16 < 2 &&
         repeat < 3 && fs < 2);
  return ((((op * 3 + (width == 1   ? 0
                       : width == 2 ? 1
                                    : 2)) *
                2 +
            a16) *
               3 +
           repeat) *
              2 +
          fs);
}
static void header(FILE *f, const char *magic, unsigned count) {
  unsigned char h[12];
  assert(fread(h, 1, 12, f) == 12 && !memcmp(h, magic, 8) &&
         le32(h + 8) == count);
}
static int boundary(uint32_t pc, uint32_t bp) {
  assert(pc == 0x10000 && bp == 0xabcdef12);
  ++seen;
  return goal && seen == goal;
}
static void invoke(void) {
  stopped = gamepkg_x86_string_controlled(input.op, input.bytes, input.a16,
                                          input.repeat, input.fs, 0x10000,
                                          0xabcdef12, boundary);
}
static void check_regs(const uint32_t *r) {
  if (g_eax != r[0] || g_ecx != r[1] || g_esi != r[2] || g_edi != r[3] ||
      g_eflags != r[4]) {
    fprintf(stderr,
            "Checkpoint op%u width%u a16%u rep%u fs%u scenario%u cut%u: "
            "eax%08x/%08x cx%08x/%08x si%08x/%08x di%08x/%08x flags%08x/%08x\n",
            input.op, input.bytes, input.a16, input.repeat, input.fs,
            input.scenario, goal, g_eax, r[0], g_ecx, r[1], g_esi, r[2], g_edi,
            r[3], g_eflags, r[4]);
    assert(0);
  }
  assert(g_ebx == 0x11112222 && g_edx == 0x33334444 && g_esp == 0x55556666 &&
         g_ebp == 0xabcdef12 && g_df == (int)((r[4] >> 10) & 1) &&
         gamepkg_x86_get_mxcsr() == 0x1f80);
}
static void image(unsigned size, const unsigned char *bytes) {
  assert(size <= 68 && string_initial_memory(expected, &input));
  for (unsigned i = 0; i < size; ++i) {
    uint32_t at = string_position(input.regs.edi, i / input.bytes, &input) +
                  i % input.bytes;
    assert(at < sizeof(expected));
    expected[at] = bytes[i];
  }
}
int main(int argc, char **argv) {
  assert(argc == 3);
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
  uint32_t endian = 1;
  assert(sizeof(void *) == 4 && *(unsigned char *)&endian == 0);
#endif
  FILE *f = fopen(argv[2], "rb");
  assert(f);
  header(f, "GPKSTRv1", 2304);
  for (unsigned row = 0; row < 2304; ++row) {
    unsigned char raw[136];
    uint32_t w[17];
    assert(fread(raw, 1, sizeof(raw), f) == sizeof(raw));
    for (unsigned i = 0; i < 17; ++i)
      w[i] = le32(raw + 4 * i);
    assert(w[5] < 16 && w[16] <= 68);
    struct Full *p = &full[key(w[0], w[1], w[2], w[3], w[4])][w[5]];
    assert(!p->valid);
    p->valid = 1;
    for (unsigned i = 0; i < 5; ++i)
      p->after[i] = w[11 + i];
    p->size = w[16];
    memcpy(p->bytes, raw + 68, 68);
  }
  assert(fgetc(f) == EOF && !ferror(f) && !fclose(f));
  f = fopen(argv[1], "rb");
  assert(f);
  header(f, "GPKSIP01", 17556);
  GamePkgX86Region region = {0, sizeof(ram), 3, ram};
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(&region, 1, 0);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  unsigned partial = 0, completed = 0;
  for (unsigned row = 0; row < 17556; ++row) {
    unsigned char raw[148];
    uint32_t w[20];
    assert(fread(raw, 1, sizeof(raw), f) == sizeof(raw));
    for (unsigned i = 0; i < 20; ++i)
      w[i] = le32(raw + 4 * i);
    assert(w[5] < 16 && w[17] <= 1 && w[18] > 0 && w[18] <= w[6] &&
           w[19] <= 68);
    unsigned form = key(w[0], w[1], w[2], w[3], w[4]);
    string_input(&input, w[0], w[1], w[2], w[3], w[4], w[5]);
    assert(w[6] > 0 && w[6] < string_steps(&input) && input.repeat &&
           input.regs.eax == w[7] && input.regs.ecx == w[8] &&
           input.regs.esi == w[9] && input.regs.edi == w[10] &&
           input.regs.flags == w[11]);
    assert(string_initial_memory(ram, &input));
    g_eax = w[7];
    g_ecx = w[8];
    g_esi = w[9];
    g_edi = w[10];
    g_eflags = w[11];
    g_df = (int)((g_eflags >> 10) & 1);
    g_fs_base = 0x2000;
    g_ebx = 0x11112222;
    g_edx = 0x33334444;
    g_esp = 0x55556666;
    g_ebp = 0xabcdef12;
    goal = w[6];
    seen = 0;
    stopped = -1;
    GamePkgX86Fault fault;
    assert(!gamepkg_x86_run(invoke, &fault) && stopped == (int)w[17]);
    assert((stopped && seen == goal) || (!stopped && seen + 1 == w[18]));
    check_regs(w + 12);
    image(w[19], raw + 80);
    assert(!memcmp(ram, expected, sizeof(ram)));
    if (stopped) {
      ++partial;
      goal = 0;
      assert(!gamepkg_x86_run(invoke, &fault) && !stopped);
    } else
      ++completed;
    const struct Full *p = &full[form][input.scenario];
    assert(p->valid);
    check_regs(p->after);
    image(p->size, p->bytes);
    assert(!memcmp(ram, expected, sizeof(ram)));
  }
  assert(fgetc(f) == EOF && !ferror(f) && !fclose(f));
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
  printf("REP checkpoint CPU replay:17556 prefix states,%u interior stops,%u "
         "early completions; "
         "all resumed/final register and81920-byte RAM images agree\n",
         partial, completed);
  return 0;
}
