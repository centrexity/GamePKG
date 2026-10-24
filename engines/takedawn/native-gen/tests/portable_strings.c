/* Complete CPU-record replay, no native x86 instructions or source aliases. */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_string.h"
#include "string_inputs.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error This fixture requires a real big-endian target
#endif
_Static_assert(sizeof(void *) == 4, "This fixture requires a 32bit target");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static unsigned char memory[STRING_RAM_SIZE], expected[STRING_RAM_SIZE];
static struct StringInput input;
static uint32_t le32(const unsigned char *p) {
  uint32_t v = 0;
  for (unsigned i = 0; i < 4; ++i)
    v |= (uint32_t)p[i] << (8 * i);
  return v;
}
static void invoke(void) {
  gamepkg_x86_string(input.op, input.bytes, input.a16, input.repeat, input.fs,
                     0x10000, 0xabcdef12);
}
int main(int argc, char **argv) {
  assert(argc == 2);
  FILE *file = fopen(argv[1], "rb");
  assert(file);
  unsigned char header[12];
  assert(fread(header, 1, 12, file) == 12 && !memcmp(header, "GPKSTRv1", 8) &&
         le32(header + 8) == 2304);
  GamePkgX86Region region = {0, sizeof(memory), 3, memory};
  GamePkgX86Memory *map = gamepkg_x86_memory_create(&region, 1, 0);
  assert(map);
  gamepkg_x86_memory_bind(map);
  for (unsigned row = 0; row < 2304; ++row) {
    unsigned char raw[136];
    uint32_t words[17];
    assert(fread(raw, 1, sizeof(raw), file) == sizeof(raw));
    for (unsigned i = 0; i < 17; ++i)
      words[i] = le32(raw + 4 * i);
    assert(words[0] <= 4 && (words[1] == 1 || words[1] == 2 || words[1] == 4) &&
           words[2] <= 1 && words[3] <= 2 && words[4] <= 1 && words[5] < 16);
    string_input(&input, words[0], words[1], words[2], words[3], words[4],
                 words[5]);
    assert(input.regs.eax == words[6] && input.regs.ecx == words[7] &&
           input.regs.esi == words[8] && input.regs.edi == words[9] &&
           input.regs.flags == words[10]);
    assert(string_initial_memory(memory, &input));
    memcpy(expected, memory, sizeof(memory));
    unsigned size = input.op < 2 ? string_steps(&input) * input.bytes : 0;
    assert(size == words[16] && size <= 68);
    for (unsigned i = 0; i < size; ++i) {
      uint32_t at = string_position(input.regs.edi, i / input.bytes, &input) +
                    i % input.bytes;
      assert(at < sizeof(memory));
      expected[at] = raw[68 + i];
    }
    g_eax = input.regs.eax;
    g_ecx = input.regs.ecx;
    g_esi = input.regs.esi;
    g_edi = input.regs.edi;
    g_eflags = input.regs.flags;
    g_df = (int)((g_eflags >> 10) & 1);
    g_fs_base = 0x2000;
    g_ebx = 0x11112222;
    g_edx = 0x33334444;
    g_esp = 0x55556666;
    g_ebp = 0xabcdef12;
    GamePkgX86Fault fault;
    int error = gamepkg_x86_run(invoke, &fault);
    if (error || g_eax != words[11] || g_ecx != words[12] ||
        g_esi != words[13] || g_edi != words[14] || g_eflags != words[15] ||
        memcmp(memory, expected, sizeof(memory))) {
      fprintf(stderr,
              "String replay row%u op%u width%u a16%u rep%u fs%u scenario%u "
              "error%d eax%08x/%08x cx%08x/%08x si%08x/%08x di%08x/%08x "
              "flags%08x/%08x\n",
              row, input.op, input.bytes, input.a16, input.repeat, input.fs,
              input.scenario, error, g_eax, words[11], g_ecx, words[12], g_esi,
              words[13], g_edi, words[14], g_eflags, words[15]);
      for (unsigned i = 0; i < sizeof(memory); ++i)
        if (memory[i] != expected[i]) {
          fprintf(stderr, "memory at%x actual%x expected%x\n", i, memory[i],
                  expected[i]);
          break;
        }
      return 1;
    }
    assert(g_ebx == 0x11112222 && g_edx == 0x33334444 && g_esp == 0x55556666 &&
           g_ebp == 0xabcdef12 && g_df == (int)((words[15] >> 10) & 1) &&
           gamepkg_x86_get_mxcsr() == 0x1f80);
  }
  assert(fgetc(file) == EOF && !ferror(file) && !fclose(file));
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(map);
  printf("String CPU replay: 2304 full register/arena states,144 "
         "width/address/repeat/FS variants; pointer_bits=%u\n",
         (unsigned)(sizeof(void *) * 8));
  return 0;
}
