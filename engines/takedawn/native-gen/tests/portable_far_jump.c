/* Replay independent protected-mode CPU branches against pure plans. */
#include "gamepkg_x86_far_jump.h"
#include "gamepkg_x86_fault.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big-endian compiler required
#endif
_Static_assert(sizeof(void *) == 4, "actual big-endian32 required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static uint32_t pending_vector, pending_error;
static void raise_fault(void) {
  gamepkg_x86_segment_fault(pending_vector, pending_error, 0x123456, 0x789abc);
}
static uint32_t word(FILE *f) {
  uint32_t value = 0;
  for (unsigned i = 0; i < 4; ++i) {
    int c = fgetc(f);
    assert(c != EOF);
    value |= (uint32_t)c << (i * 8);
  }
  return value;
}
static void invalid_and_boundaries(void) {
  GamePkgX86DescriptorLookup lookup, saved_lookup;
  memset(&lookup, 0xa5, sizeof(lookup));
  saved_lookup = lookup;
  assert(gamepkg_x86_far_lookup(65536, 65535, 15, 1, &lookup) == -1 &&
         !memcmp(&lookup, &saved_lookup, sizeof(lookup)));
  assert(gamepkg_x86_far_lookup(8, 65536, 15, 1, &lookup) == -1 &&
         !memcmp(&lookup, &saved_lookup, sizeof(lookup)));
  assert(gamepkg_x86_far_lookup(8, 65535, 15, 2, &lookup) == -1 &&
         !memcmp(&lookup, &saved_lookup, sizeof(lookup)));
  assert(gamepkg_x86_far_lookup(8, 65535, 15, 1, NULL) == -1);
  for (unsigned rpl = 0; rpl < 4; ++rpl) {
    assert(gamepkg_x86_far_lookup(0xfff8 | rpl, 65535, 0, 0, &lookup) == 0 &&
           lookup.offset == 0xfff8 && !lookup.table);
    assert(gamepkg_x86_far_lookup(0xfff8 | rpl, 65534, 0, 0, &lookup) == 1 &&
           lookup.error == 0xfff8);
    assert(gamepkg_x86_far_lookup(4 | rpl, 0, 7, 1, &lookup) == 0 &&
           lookup.table == 1 && lookup.offset == 0);
    assert(gamepkg_x86_far_lookup(4 | rpl, 0, 6, 1, &lookup) == 1 &&
           lookup.error == 4);
  }
  GamePkgX86CodeTarget plan, saved;
  memset(&plan, 0xa5, sizeof(plan));
  saved = plan;
  for (unsigned cpl = 4; cpl < 8; ++cpl)
    assert(gamepkg_x86_far_plan_code(8, 0, cpl, 0, 0xffff, 0xcf9b00, &plan) ==
               -1 &&
           !memcmp(&plan, &saved, sizeof(plan)));
  assert(gamepkg_x86_far_plan_code(65536, 0, 0, 0, 0, 0, &plan) == -1 &&
         !memcmp(&plan, &saved, sizeof(plan)));
  assert(gamepkg_x86_far_plan_code(8, 0, 0, 2, 0, 0, &plan) == -1 &&
         !memcmp(&plan, &saved, sizeof(plan)));
  for (unsigned type = 0; type < 16; ++type)
    assert(gamepkg_x86_far_plan_code(8, 0, 0, 0, 0xffff, 0x8000 | (type << 8),
                                     &plan) == -1 &&
           !memcmp(&plan, &saved, sizeof(plan)));
  assert(gamepkg_x86_far_plan_code(8, 0, 0, 0, 0, 0x409b00, NULL) == -1);
  /* Fixed raw fields, including upper base byte, expanded granular limit and
   * target width. Result is a proposal; even an accessed update is not applied.
   */
  assert(!gamepkg_x86_far_plan_code(8, 0x1234, 0, 0, 0xcdef2345, 0x89c19aab,
                                    &plan));
  assert(plan.base == 0x89abcdef && plan.limit == 0x12345fff &&
         plan.selector == 8 && plan.offset == 0x1234 &&
         plan.attributes == 0xc9b && plan.mark_accessed == 1);
  assert(!gamepkg_x86_far_plan_code(11, 0, 3, 0, 0, 0x0040fa00, &plan));
  assert(plan.selector == 11 && plan.limit == 0 &&
         gamepkg_x86_code_fetch_span(plan, 0, 1) == 0 &&
         gamepkg_x86_code_fetch_span(plan, 0, 5) == 1 &&
         gamepkg_x86_code_fetch_span(plan, 1, 1) == 1);
  assert(gamepkg_x86_code_fetch_span(plan, 0, 0) == -1 &&
         gamepkg_x86_code_fetch_span(plan, 0, 16) == -1);
  assert(
      !gamepkg_x86_far_plan_code(8, 0xffffffff, 0, 0, 0xffff, 0xcf9b00, &plan));
  assert(gamepkg_x86_code_fetch_span(plan, 0xffffffff, 1) == 0 &&
         gamepkg_x86_code_fetch_span(plan, 0xffffffff, 2) == 1);
  for (unsigned n = 1; n <= 15; ++n) {
    assert(!gamepkg_x86_code_fetch_span(plan, UINT32_MAX - (n - 1), n));
    if (n > 1)
      assert(gamepkg_x86_code_fetch_span(plan, UINT32_MAX - (n - 2), n) == 1);
  }
  for (unsigned which = 0; which < 10; ++which) {
    GamePkgX86CodeTarget bad = plan;
    switch (which) {
    case 0:
      bad.vector = 13;
      break;
    case 1:
      bad.error = 1;
      break;
    case 2:
      bad.selector = 65536;
      break;
    case 3:
      bad.selector = 3;
      break;
    case 4:
      bad.attributes |= 0x1000;
      break;
    case 5:
      bad.attributes &= ~8u;
      break;
    case 6:
      bad.attributes &= ~128u;
      break;
    case 7:
      bad.limit = 0xfffffff0;
      break;
    case 8:
      bad.mark_accessed = 2;
      break;
    default:
      bad.selector = 11;
      break;
    }
    assert(gamepkg_x86_code_fetch_span(bad, 0, 1) == -1);
  }
  GamePkgX86CodeTarget conforming = plan;
  conforming.selector = 11;
  conforming.attributes |= 4;
  assert(!gamepkg_x86_code_fetch_span(conforming, 0, 15));
  conforming.selector = 8;
  conforming.attributes |= 0x60;
  assert(gamepkg_x86_code_fetch_span(conforming, 0, 1) == -1);
  pending_vector = 12;
  pending_error = 65535;
  GamePkgX86Fault explicit_fault;
  assert(gamepkg_x86_run(raise_fault, &explicit_fault) == 1 &&
         explicit_fault.vector == 12 && explicit_fault.error_code == 65535);
  assert(gamepkg_x86_far_lookup(8, 65535, 0, 0,
                                (void *)(UINTPTR_MAX & ~(uintptr_t)3)) == -1);
  _Alignas(GamePkgX86CodeTarget) unsigned char raw[sizeof(plan) + 1];
  assert(gamepkg_x86_far_plan_code(8, 0, 0, 0, 0, 0x409b00,
                                   (void *)(raw + 1)) == -1);
}
int main(int argc, char **argv) {
  assert(argc == 2);
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && !memcmp(magic, "GPKFAR02", 8));
  uint32_t count = word(f), success = 0, np = 0, gp = 0, fetch = 0,
           accessed = 0, tcg_access_omissions = 0;
  assert(count);
  g_eflags = 0x202;
  for (uint32_t i = 0; i < count; ++i) {
    uint32_t w[18];
    for (unsigned j = 0; j < 18; ++j)
      w[j] = word(f);
    GamePkgX86DescriptorLookup lookup;
    int selector = gamepkg_x86_far_lookup(w[1], w[6], w[7], w[8], &lookup);
    assert(selector >= 0);
    GamePkgX86CodeTarget plan = {0};
    int status;
    if (selector) {
      plan.vector = lookup.vector;
      plan.error = lookup.error;
      status = 1;
    } else {
      assert(lookup.table == ((w[1] >> 2) & 1) &&
             lookup.offset == (w[1] & 0xfff8));
      status =
          gamepkg_x86_far_plan_code(w[1], w[3], w[0], w[2], w[4], w[5], &plan);
      assert(status >= 0);
    }
    if (w[16] == 1) {
      if (status != 1 || plan.vector != w[9] || plan.error != w[10]) {
        fprintf(stderr,
                "far record%u CPL%u sel%04x input%08x:%08x actual%u/%u/%d "
                "CPU%u/%u\n",
                i, w[0], w[1], w[5], w[4], plan.vector, plan.error, status,
                w[9], w[10]);
        assert(0);
      }
      assert(w[11] == ((uint32_t[]){8, 65, 82, 35})[w[0]] && w[12] == w[4] &&
             w[13] == w[5]);
      assert(!plan.selector && !plan.base && !plan.limit && !plan.attributes &&
             !plan.mark_accessed);
      pending_vector = plan.vector;
      pending_error = plan.error;
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(raise_fault, &fault) == 1 &&
             fault.vector == plan.vector && fault.error_code == plan.error &&
             fault.eip == 0x123456 && fault.ebp == 0x789abc &&
             fault.fault_address == 0);
      if (plan.vector == 11)
        ++np;
      else {
        assert(plan.vector == 13);
        ++gp;
      }
    } else {
      assert((w[16] == 0 || w[16] == 2) && status == 0 && !plan.vector &&
             !plan.error && plan.selector == w[11] &&
             plan.selector == ((w[1] & 0xfffc) | w[0]));
      /* TCG currently omits the architecturally required clear-A update.
       * Keep raw observations intact and account for that limitation rather
       * than teaching the production plan to omit the requested locked write.
       */
      assert(w[12] == w[4] && (w[13] & ~0x100u) == (w[5] & ~0x100u) &&
             plan.mark_accessed == !(w[5] & 0x100u));
      assert((w[13] & 0x100u) || plan.mark_accessed);
      if (plan.mark_accessed && !(w[13] & 0x100u))
        ++tcg_access_omissions;
      assert(plan.base + plan.offset ==
             ((plan.attributes & 0x400u) ? 0x200000u : 0x210000u));
      accessed += plan.mark_accessed;
      if (w[16] == 2) {
        assert(w[9] == 13 && w[10] == 0 && w[17] == plan.offset);
        ++fetch;
      } else {
        assert(w[9] == 0 && w[14] == w[15]);
      }
      ++success;
    }
  }
  assert(fgetc(f) == EOF && !ferror(f) && !fclose(f));
  invalid_and_boundaries();
  printf("TCG reference limitations: %u clear-accessed-bit updates omitted; "
         "fetch-boundary behavior is qualified separately on native ELF32\n",
         tcg_access_omissions);
  printf(
      "Far-code plans: %u independent records (%u transferred, %u NP, %u GP, "
      "%u accessed requests, %u target-fetch faults); selector errors, "
      "complete "
      "fetch spans and nonmutating rejection passed; pointer_bits=%u\n",
      count, success, np, gp, accessed, fetch, (unsigned)(sizeof(void *) * 8));
  return 0;
}
