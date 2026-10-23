/* Portable instruction-state tests, no game bytes or native privileged ops. */
#include "gamepkg_x86_interrupt.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static unsigned checked;
static void table(void) {
  /* Rows independently describe the SDM's four protected privilege classes:
   * each bit names an IOPL permitted to update physical IF at that CPL. */
  const unsigned physical_iopl[4] = {15, 14, 12, 8};
  for (unsigned pe = 0; pe < 2; ++pe)
    for (unsigned vm = 0; vm < 2; ++vm)
      for (unsigned cpl = 0; cpl < 4; ++cpl) {
        if ((!pe && cpl) || (pe && vm && cpl != 3))
          continue;
        for (unsigned pvi = 0; pvi < 2; ++pvi)
          for (unsigned vme = 0; vme < 2; ++vme)
            for (unsigned iopl = 0; iopl < 4; ++iopl)
              for (unsigned bits = 0; bits < 8; ++bits)
                for (unsigned shadow = 0; shadow < 2; ++shadow)
                  for (unsigned op = 0; op < 2; ++op) {
                    uint32_t flags = 0xa0c40cd7u | (iopl << 12) | (vm << 17) |
                                     ((bits & 1) << 9) | ((bits & 2) << 18) |
                                     ((bits & 4) << 18);
                    int physical =
                        !pe || (vm ? iopl == 3
                                   : !!(physical_iopl[cpl] & (1u << iopl)));
                    int virtualized =
                        !physical && pe && (vm ? vme : cpl == 3 && pvi);
                    int fault =
                        !physical && (!virtualized || (op && (bits & 4)));
                    GamePkgX86InterruptState s = {flags, pe,  cpl,
                                                  pvi,   vme, shadow},
                                             old = s;
                    int result = gamepkg_x86_interrupt_step(&s, op);
                    assert(result == fault);
                    if (fault) {
                      assert(!memcmp(&s, &old, sizeof(s)));
                    } else {
                      uint32_t mask = physical ? 0x200u : 0x80000u;
                      assert(s.flags == (op ? flags | mask : flags & ~mask));
                      assert(s.sti_shadow ==
                             (op && physical && !(flags & 0x200)));
                      assert(s.protected_mode == pe && s.cpl == cpl &&
                             s.pvi == pvi && s.vme == vme);
                    }
                    assert(gamepkg_x86_interrupt_eligible(&s) ==
                           (!!(s.flags & 0x200) && !s.sti_shadow));
                    assert(!gamepkg_x86_interrupt_retire_other(&s));
                    assert(!s.sti_shadow);
                    ++checked;
                  }
      }
}
static void sequences(void) {
  GamePkgX86InterruptState s = {2, 1, 0, 0, 0, 0};
  assert(!gamepkg_x86_interrupt_step(&s, 1));
  assert(s.flags == 0x202 && s.sti_shadow &&
         !gamepkg_x86_interrupt_eligible(&s));
  assert(!gamepkg_x86_interrupt_step(&s, 1));
  assert(!s.sti_shadow && gamepkg_x86_interrupt_eligible(&s));
  assert(!gamepkg_x86_interrupt_step(&s, 0) && s.flags == 2);
  assert(!gamepkg_x86_interrupt_step(&s, 1) && s.sti_shadow);
  assert(!gamepkg_x86_interrupt_step(&s, 0));
  assert(!s.sti_shadow && !gamepkg_x86_interrupt_eligible(&s));
  assert(!gamepkg_x86_interrupt_step(&s, 1));
  assert(!gamepkg_x86_interrupt_retire_other(
      &s)); /* Other event/fault delivered. */
  assert(gamepkg_x86_interrupt_eligible(&s));
  /* Saved state round trips without changing another CPU/thread image. */
  s.flags = 2;
  assert(!gamepkg_x86_interrupt_step(&s, 1));
  GamePkgX86InterruptState saved = s, other = {0x202, 1, 0, 0, 0, 0};
  assert(gamepkg_x86_interrupt_eligible(&other));
  assert(!memcmp(&saved, &s, sizeof(s)) &&
         !gamepkg_x86_interrupt_eligible(&saved));
}
static void invalid(void) {
  GamePkgX86InterruptState good = {0x202, 1, 0, 0, 0, 0};
  assert(gamepkg_x86_interrupt_step(NULL, 0) == -1);
  assert(gamepkg_x86_interrupt_retire_other(NULL) == -1);
  assert(gamepkg_x86_interrupt_eligible(NULL) == -1);
  GamePkgX86InterruptState *wrong[] = {
      (GamePkgX86InterruptState *)((uintptr_t)&good + 1),
      (GamePkgX86InterruptState *)(UINTPTR_MAX & ~(uintptr_t)3)};
  for (unsigned i = 0; i < 2; ++i) {
    assert(gamepkg_x86_interrupt_step(wrong[i], 0) == -1);
    assert(gamepkg_x86_interrupt_retire_other(wrong[i]) == -1);
    assert(gamepkg_x86_interrupt_eligible(wrong[i]) == -1);
  }
  for (unsigned field = 0; field < 7; ++field) {
    GamePkgX86InterruptState bad = good;
    if (field == 0)
      bad.protected_mode = 2;
    if (field == 1)
      bad.cpl = 4;
    if (field == 2)
      bad.pvi = 2;
    if (field == 3)
      bad.vme = 2;
    if (field == 4)
      bad.sti_shadow = 2;
    if (field == 5) {
      bad.protected_mode = 0;
      bad.cpl = 1;
    }
    if (field == 6)
      bad.flags |= 0x20000;
    GamePkgX86InterruptState old = bad;
    assert(gamepkg_x86_interrupt_step(&bad, 0) == -1);
    assert(gamepkg_x86_interrupt_retire_other(&bad) == -1);
    assert(gamepkg_x86_interrupt_eligible(&bad) == -1);
    assert(!memcmp(&old, &bad, sizeof(bad)));
  }
  GamePkgX86InterruptState old = good;
  assert(gamepkg_x86_interrupt_step(&good, 2) == -1);
  assert(!memcmp(&old, &good, sizeof(good)));
}
static uint32_t read_word(FILE *f) {
  unsigned char b[4];
  assert(fread(b, 1, 4, f) == 4);
  return (uint32_t)b[0] | ((uint32_t)b[1] << 8) | ((uint32_t)b[2] << 16) |
         ((uint32_t)b[3] << 24);
}
static void corpus(const char *path) {
  FILE *f = fopen(path, "rb");
  assert(f);
  char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && !memcmp(magic, "GPKIRQv1", 8));
  assert(read_word(f) == 2051);
  unsigned ordinary = 0, shadows = 0;
  for (unsigned i = 0; i < 2051; ++i) {
    uint32_t row[8];
    for (unsigned j = 0; j < 8; ++j)
      row[j] = read_word(f);
    if (!row[0]) {
      GamePkgX86InterruptState s = {row[1], 1, 0, 0, 0, 0};
      assert(!gamepkg_x86_interrupt_step(&s, row[2]) && s.flags == row[3]);
      ++ordinary;
    } else {
      assert(row[0] == 1 && row[1] < 3 && row[2] == 1);
      unsigned sequence = row[1];
      GamePkgX86InterruptState s = {2, 1, 0, 0, 0, 0};
      assert(!gamepkg_x86_interrupt_step(&s, 1) &&
             !gamepkg_x86_interrupt_eligible(&s));
      if (sequence == 0)
        assert(!gamepkg_x86_interrupt_retire_other(&s));
      else
        assert(!gamepkg_x86_interrupt_step(&s, sequence == 2));
      int delivered = gamepkg_x86_interrupt_eligible(&s);
      assert(row[3] == (unsigned)delivered);
      if (delivered) {
        assert(row[4] == (sequence == 0) && row[5] == 0 && row[6] == 0x202);
      }
      assert(row[7] == 2);
      ++shadows;
    }
  }
  assert(ordinary == 2048 && shadows == 3 && fgetc(f) == EOF);
  fclose(f);
  puts("Pentium3 QEMU:2048 privileged flag cases and3 pending-PIT IRQ shadow "
       "sequences agree");
}
int main(int argc, char **argv) {
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
  uint32_t probe = 1;
  assert(sizeof(void *) == 4 && *(unsigned char *)&probe == 0);
#endif
  table();
  sequences();
  invalid();
  printf("Interrupt state:%u mode/privilege/IF/VIF/VIP/shadow cases passed\n",
         checked);
  if (argc == 2)
    corpus(argv[1]);
  else
    assert(argc == 1);
  return 0;
}
