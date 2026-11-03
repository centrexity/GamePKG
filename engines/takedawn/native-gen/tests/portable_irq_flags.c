/* Consume independent protected-mode gate frames. No native x86 in replay. */
#include "gamepkg_x86_interrupt.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static uint32_t le32(const unsigned char *p) {
  return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) |
         ((uint32_t)p[3] << 24);
}
static void guards(void) {
  GamePkgX86InterruptState state = {0x202, 1, 0, 0, 0, 0}, original = state;
  GamePkgX86IrqFlags output = {0xaabbccdd, 0x12345678}, before = output;
  assert(gamepkg_x86_interrupt_plan_irq(NULL, 0, 0, &output) == -1);
  assert(gamepkg_x86_interrupt_plan_irq(&state, 2, 0, &output) == -1);
  assert(gamepkg_x86_interrupt_plan_irq(&state, 0, 2, &output) == -1);
  assert(gamepkg_x86_interrupt_plan_irq(&state, 0, 0, NULL) == -1);
  assert(gamepkg_x86_interrupt_plan_irq(
             &state, 0, 0, (GamePkgX86IrqFlags *)(uintptr_t)UINTPTR_MAX) == -1);
  assert(gamepkg_x86_interrupt_plan_irq(&state, 0, 0,
                                        (GamePkgX86IrqFlags *)&state) == -1);
  assert(!memcmp(&state, &original, sizeof(state)) &&
         !memcmp(&output, &before, sizeof(output)));
  for (unsigned inhibition = 0; inhibition < 3; ++inhibition) {
    state = original;
    if (inhibition != 1)
      state.flags &= ~0x200u;
    if (inhibition != 0)
      state.sti_shadow = 1;
    GamePkgX86InterruptState copy = state;
    for (unsigned rep = 0; rep < 2; ++rep)
      for (unsigned trap = 0; trap < 2; ++trap) {
        assert(!gamepkg_x86_interrupt_plan_irq(&state, rep, trap, &output) &&
               !memcmp(&output, &before, sizeof(output)) &&
               !memcmp(&state, &copy, sizeof(state)));
      }
  }
  for (unsigned invalid = 0; invalid < 7; ++invalid) {
    state = original;
    switch (invalid) {
    case 0:
      state.protected_mode = 0;
      break;
    case 1:
      state.cpl = 1;
      break;
    case 2:
      state.cpl = 2;
      break;
    case 3:
      state.cpl = 3;
      break;
    case 4:
      state.flags |= 0x20000;
      break;
    case 5:
      state.sti_shadow = 2;
      break;
    default:
      state.pvi = 2;
      break;
    }
    GamePkgX86InterruptState copy = state;
    assert(gamepkg_x86_interrupt_plan_irq(&state, 0, 0, &output) == -1 &&
           !memcmp(&output, &before, sizeof(output)) &&
           !memcmp(&state, &copy, sizeof(state)));
  }
}
int main(int argc, char **argv) {
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
  _Static_assert(sizeof(void *) == 4, "actual32 required");
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Actual big endian required
#endif
#endif
  guards();
  if (argc == 1) {
    puts("IRQ flag planning: invalid, unsupported, overlapping and inhibited "
         "state guards passed; CPU corpus requires explicit path");
    return 0;
  }
  assert(argc == 2);
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  unsigned char bytes[24], header[12], seen[8192] = {0};
  assert(fread(header, 1, 12, f) == 12 && !memcmp(header, "GPKIGF01", 8) &&
         le32(header + 8) == 8192);
  unsigned interior = 0;
  for (unsigned i = 0; i < 8192; ++i) {
    assert(fread(bytes, 1, 24, f) == 24);
    uint32_t initial = le32(bytes), trap = le32(bytes + 4),
             delta = le32(bytes + 8), saved = le32(bytes + 12),
             handler = le32(bytes + 16), count = le32(bytes + 20);
    const uint32_t bits[] = {1, 4, 16, 64, 128, 1024, 2048};
    unsigned status = 0;
    for (unsigned j = 0; j < 7; ++j)
      if (initial & bits[j])
        status |= 1u << j;
    unsigned special =
        ((initial >> 8) & 1) | ((initial >> 13) & 2) | ((initial >> 14) & 4);
    unsigned key =
        (((status * 4 + ((initial >> 12) & 3)) * 8 + special) * 2 + trap);
    assert(trap < 2 && key < 8192 && !seen[key]++ && !delta && count == 1 &&
           saved == initial);
    GamePkgX86InterruptState state = {initial, 1, 0, 0, 0, 0}, original = state;
    GamePkgX86IrqFlags output = {0}, before = {0xaabbccdd, 0x12345678};
    assert(gamepkg_x86_interrupt_plan_irq(&state, 0, trap, &output) == 1 &&
           output.saved_flags == saved && output.handler_flags == handler &&
           !memcmp(&state, &original, sizeof(state)));
    /* Saved RF for a REP interior is additionally qualified by actual i386
     * timer-signal contexts and Intel SDM; this corpus is between instructions.
     */
    assert(gamepkg_x86_interrupt_plan_irq(&state, 1, trap, &output) == 1 &&
           output.saved_flags == (saved | 0x10000u) &&
           output.handler_flags == handler);
    ++interior;
    for (unsigned inhibition = 0; inhibition < 3; ++inhibition) {
      state = original;
      if (inhibition != 1)
        state.flags &= ~0x200u;
      if (inhibition != 0)
        state.sti_shadow = 1;
      GamePkgX86InterruptState inhibited = state;
      for (unsigned rep = 0; rep < 2; ++rep) {
        output = before;
        assert(!gamepkg_x86_interrupt_plan_irq(&state, rep, trap, &output) &&
               !memcmp(&output, &before, sizeof(output)) &&
               !memcmp(&state, &inhibited, sizeof(state)));
      }
    }
  }
  assert(fgetc(f) == EOF && !ferror(f) && !fclose(f));
  printf("IRQ flags:8192 independent saved/live gate frames,%u REP saved-RF "
         "plans and49152 inhibited cases; pointer_bits=%u\n",
         interior, (unsigned)(sizeof(void *) * 8));
  return 0;
}
