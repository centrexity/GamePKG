/* Replay independently executed protected-mode CPL0 POPF/POPFD/PUSHFD words.
 * Host record format and explicit LE input; no x86 instructions or host flags.
 */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_flags.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error This fixture requires an actual big-endian target
#endif
_Static_assert(sizeof(void *) == 4, "This fixture requires a 32bit target");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static uint32_t le32(const unsigned char *b) {
  uint32_t v = 0;
  for (unsigned i = 0; i < 4; ++i)
    v |= (uint32_t)b[i] << (8 * i);
  return v;
}
int main(int argc, char **argv) {
  assert(argc == 2);
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  unsigned char b[16];
  assert(fread(b, 1, 12, f) == 12 && !memcmp(b, "GPKFLGv1", 8) &&
         le32(b + 8) == 524288);
  for (unsigned i = 0; i < 524288; ++i) {
    assert(fread(b, 1, 16, f) == 16);
    uint32_t width = le32(b), initial = le32(b + 4), input = le32(b + 8),
             expected = le32(b + 12);
    assert(width == 2 || width == 4);
    g_eflags = initial;
    g_df = (int)((initial >> 10) & 1);
    g_eax = 0x12345678;
    g_ecx = 0xabcdef01;
    g_esp = 0x11112222;
    gamepkg_x86_flags_load(input, width);
    if (gamepkg_x86_flags_image() != expected || g_eflags != expected ||
        g_df != (int)((expected >> 10) & 1)) {
      fprintf(stderr,
              "flags corpus row%u width%u old%08x input%08x image%08x "
              "expected%08x\n",
              i, width, initial, input, gamepkg_x86_flags_image(), expected);
      return 1;
    }
    assert(g_eax == 0x12345678 && g_ecx == 0xabcdef01 && g_esp == 0x11112222 &&
           gamepkg_x86_get_mxcsr() == 0x1f80);
  }
  assert(fgetc(f) == EOF && !ferror(f) && !fclose(f));
  printf("Protected-mode CPL0 flag replay: 524288 exact instruction records; "
         "pointer_bits=%u\n",
         (unsigned)(sizeof(void *) * 8));
  return 0;
}
