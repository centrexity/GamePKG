#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_virtual_pages.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big endian required
#endif
_Static_assert(sizeof(void *) == 4, "actual32bit required");
#endif
/* Maintainer-only allocation failure injection; no runtime hook/default. */
#ifdef GAMEPKG_VIRTUAL_ALLOCATOR_TEST
static unsigned allocation_calls, fail_allocation;
void *gamepkg_virtual_test_calloc(size_t n, size_t size) {
  ++allocation_calls;
  if (fail_allocation && allocation_calls == fail_allocation)
    return NULL;
  return calloc(n, size);
}
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
static GamePkgX86Memory *memory;
static uint32_t address, op, words[4], loaded[4], width = 4, rmw_result, user;
static unsigned checks;
static void body(void) {
  switch (op) {
  case 0:
    gamepkg_x86_memory_read(address, loaded, width, 0x1234, 0x5678);
    break;
  case 1:
    gamepkg_x86_memory_write(address, words, width, (1u << width) - 1, 0,
                             0x1234, 0x5678);
    break;
  case 2:
    rmw_result = gamepkg_x86_memory_rmw(address, 4, 1, 1, 0, 0x1234, 0x5678);
    break;
  default:
    abort();
  }
}
static void access(uint32_t a, unsigned operation, unsigned vector,
                   unsigned error, uint32_t fault_address) {
  address = a;
  op = operation;
  GamePkgX86FpFault before, after, fault;
  assert(!gamepkg_x86_capture_registers(0, 0, &before, &source));
  int rc = gamepkg_x86_run_with_fp(body, &fault, &source);
  if (rc != (vector ? 1 : 0) ||
      (vector && (fault.core.vector != vector ||
                  fault.core.error_code != (error | (user ? 4u : 0u)) ||
                  fault.core.fault_address != fault_address)))
    fprintf(stderr,
            "access op=%u at=%08x rc=%d vector=%u/%u error=%u/%u "
            "address=%08x/%08x\n",
            op, a, rc, fault.core.vector, vector, fault.core.error_code, error,
            fault.core.fault_address, fault_address);
  assert(rc == (vector ? 1 : 0));
  if (vector)
    assert(fault.core.vector == vector &&
           fault.core.error_code == (error | (user ? 4u : 0u)) &&
           fault.core.fault_address == fault_address &&
           !memcmp(&before.fp, &fault.fp, sizeof(before.fp)));
  assert(!gamepkg_x86_capture_registers(0, 0, &after, &source) &&
         !memcmp(&before.fp, &after.fp, sizeof(before.fp)));
  ++checks;
}
static GamePkgX86VirtualStats stats(void) {
  GamePkgX86VirtualStats s;
  assert(!gamepkg_x86_virtual_stats(memory, &s));
  return s;
}
static void new_memory(uint32_t base, uint32_t length, uint32_t limit) {
  static unsigned char fixed[4096];
  GamePkgX86Region r = {0x1000, 4096, 3, fixed};
  memory = gamepkg_x86_memory_create(&r, 1, user);
  assert(memory);
  assert(gamepkg_x86_virtual_configure(memory, 0x1000, 4096, limit) == -1);
  assert(!gamepkg_x86_virtual_configure(memory, base, length, limit));
  assert(gamepkg_x86_virtual_configure(memory, base, length, limit) == -1);
  assert(gamepkg_x86_virtual_is_configured(memory) == -1);
  gamepkg_x86_memory_bind(memory);
  assert(!gamepkg_x86_virtual_is_configured(memory));
  assert(gamepkg_x86_virtual_configure(memory, base, length, limit) == -1);
}
static void teardown(void) {
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
  memory = NULL;
}
static void basic(void) {
  new_memory(0x100000, 0x40000, 8 * 4096);
  GamePkgX86VirtualResult r =
      gamepkg_x86_virtual_reserve(memory, 0, 0x4000, UINT32_MAX, 0, 4, 0);
  assert(!r.status && r.base == 0x100000 && r.length == 0x4000);
  assert(stats().reserved == 0x4000 && !stats().committed);
  access(r.base, 0, 14, 0, r.base);
  access(r.base, 1, 14, 2, r.base);
  assert(!gamepkg_x86_virtual_commit(memory, r.base + 4096, 4096, 4).status);
  access(r.base + 4096, 0, 0, 0, 0);
  assert(!loaded[0]);
  words[0] = 0x78563412;
  access(r.base + 4096, 1, 0, 0, 0);
  assert(!gamepkg_x86_virtual_commit(memory, r.base + 4096, 4096, 2).status);
  access(r.base + 4096, 0, 0, 0, 0);
  assert(loaded[0] == 0x78563412);
  access(r.base + 4096, 1, 14, 3, r.base + 4096);
  access(r.base + 4096, 2, 14, 3, r.base + 4096);
  assert(!gamepkg_x86_virtual_protect(memory, r.base + 4096, 4096, 1).status);
  access(r.base + 4096, 0, 14, 0, r.base + 4096);
  assert(stats().committed == 4096);
  assert(!gamepkg_x86_virtual_protect(memory, r.base + 4096, 4096, 4).status);
  access(r.base + 4096, 2, 0, 0, 0);
  assert(rmw_result == 0x78563412);
  assert(gamepkg_x86_virtual_protect(memory, r.base, 0x4000, 2).status == 4);
  access(r.base + 4096, 0, 0, 0, 0);
  assert(loaded[0] == 0x78563413);
  assert(!gamepkg_x86_virtual_free(memory, r.base + 4096, 4096, 0).status);
  assert(!gamepkg_x86_virtual_free(memory, r.base + 4096, 4096, 0).status);
  assert(!stats().committed);
  access(r.base + 4096, 0, 14, 0, r.base + 4096);
  assert(!gamepkg_x86_virtual_commit(memory, r.base + 4096, 4096, 4).status);
  access(r.base + 4096, 0, 0, 0, 0);
  assert(!loaded[0]);
  // Whole-span admission across a late uncommitted page never partially writes.
  words[0] = 0x11111111;
  access(r.base + 8192 - 4, 1, 0, 0, 0);
  for (unsigned i = 0; i < 4; ++i)
    words[i] = 0xa5a5a5a5;
  width = 16;
  access(r.base + 8192 - 4, 1, 14, 2, r.base + 8192);
  width = 4;
  access(r.base + 8192 - 4, 0, 0, 0, 0);
  assert(loaded[0] == 0x11111111);
  assert(!gamepkg_x86_virtual_free(memory, r.base + 4096, 4096, 1).status);
  assert(stats().reserved == 0x3000 && !stats().committed);
  assert(gamepkg_x86_virtual_query(memory, r.base + 4096).state == 0x10000);
  assert(gamepkg_x86_virtual_query(memory, r.base + 8192).allocation_base ==
         r.base + 8192);
  assert(gamepkg_x86_virtual_commit(memory, r.base, 0x4000, 4).status == 3);
  assert(!gamepkg_x86_virtual_free(memory, r.base, 0, 1).status);
  assert(!gamepkg_x86_virtual_free(memory, r.base + 8192, 0, 1).status);
  assert(!stats().reserved);
  r = gamepkg_x86_virtual_reserve(memory, 0, 4096, UINT32_MAX, 1, 4, 1);
  assert(!r.status && r.base == 0x130000);
  assert(gamepkg_x86_virtual_reserve(memory, r.base, 4096, UINT32_MAX, 0, 4, 0)
             .status == 1);
  assert(
      gamepkg_x86_virtual_reserve(memory, 0, 4096, 0xfffff, 0, 4, 0).status ==
      2);
  assert(gamepkg_x86_virtual_reserve(memory, 0, 4096, UINT32_MAX, 0, 0x104, 0)
             .status == -2);
  assert(gamepkg_x86_virtual_commit(memory, r.base, 4096, 8).status == -2);
  assert(
      gamepkg_x86_virtual_reserve(memory, 0x100001, 4096, UINT32_MAX, 0, 4, 0)
          .status == -1);
  assert(gamepkg_x86_virtual_free(memory, r.base + 1, 4096, 0).status == -1);
  teardown();
  // A gigabyte of reserved VA consumes no committed backing. The explicit
  // physical profile here has one page, independent of native pointer size.
  new_memory(0x10000000, 0x70000000, 4096);
  r = gamepkg_x86_virtual_reserve(memory, 0, 0x40000000, 0x7fffffff, 0, 4, 0);
  assert(!r.status && stats().reserved == 0x40000000 && !stats().committed);
  assert(
      !gamepkg_x86_virtual_commit(memory, r.base + 0x3ffff000, 4096, 4).status);
  assert(gamepkg_x86_virtual_commit(memory, r.base, 4096, 4).status == 2);
  assert(stats().committed == 4096);
  assert(gamepkg_x86_virtual_query(memory, r.base).state == 0x2000);
  access(r.base + 0x3ffff000, 0, 0, 0, 0);
  assert(!loaded[0]);
  teardown();
}
#ifdef GAMEPKG_VIRTUAL_ALLOCATOR_TEST
static void allocation_failures(void) {
  new_memory(0x100000, 0x40000, 8 * 4096);
  for (unsigned fail = 1; fail <= 7; ++fail) {
    fail_allocation = fail;
    allocation_calls = 0;
    GamePkgX86VirtualResult r =
        gamepkg_x86_virtual_reserve(memory, 0, 4 * 4096, UINT32_MAX, 0, 4, 1);
    assert(r.status == 2 && !stats().reserved && !stats().committed);
    fail_allocation = 0;
    ++checks;
  }
  assert(!gamepkg_x86_virtual_reserve(memory, 0, 4 * 4096, UINT32_MAX, 0, 4, 0)
              .status);
  assert(!gamepkg_x86_virtual_commit(memory, 0x101000, 4096, 2).status);
  for (unsigned fail = 1; fail <= 4; ++fail) {
    fail_allocation = fail;
    allocation_calls = 0;
    assert(gamepkg_x86_virtual_commit(memory, 0x100000, 4 * 4096, 4).status ==
           2);
    fail_allocation = 0;
    assert(stats().committed == 4096 &&
           gamepkg_x86_virtual_query(memory, 0x101000).protection == 2);
    ++checks;
  }
  for (unsigned fail = 1; fail <= 4; ++fail) {
    fail_allocation = fail;
    allocation_calls = 0;
    assert(gamepkg_x86_virtual_free(memory, 0x101000, 4096, 1).status == 2);
    fail_allocation = 0;
    assert(stats().reserved == 4 * 4096 && stats().committed == 4096 &&
           gamepkg_x86_virtual_query(memory, 0x101000).allocation_base ==
               0x100000);
    ++checks;
  }
  teardown();
}
#endif
// Independent flat page-state oracle. No host VAD nodes or page-store lookup.
typedef struct Model {
  uint32_t id, protection, value, allocation_protection;
  unsigned committed;
} Model;
static uint32_t random_state = 0x7392b415;
static uint32_t random_value(void) {
  random_state ^= random_state << 13;
  random_state ^= random_state >> 17;
  random_state ^= random_state << 5;
  return random_state;
}
static void modeled(void) {
  Model pages[64] = {0};
  new_memory(0x100000, 64 * 4096, 8 * 4096);
  for (unsigned step = 0; step < 8000; ++step) {
    unsigned operation = random_value() % 5, first = random_value() % 64,
             count = 1 + random_value() % 4;
    uint32_t protection =
        (uint32_t[]){1, 2, 4, 0x10, 0x20, 0x40}[random_value() % 6];
    int expected = 0;
    GamePkgX86VirtualResult result;
    unsigned committed = 0;
    for (unsigned i = 0; i < 64; ++i)
      committed += pages[i].committed;
    if (operation == 0) {
      first = (random_value() % 4) * 16;
      count = 1 + random_value() % 16;
      unsigned commit = random_value() % 2;
      for (unsigned i = first; i < first + count; ++i)
        if (pages[i].id)
          expected = 1;
      if (!expected && commit && committed + count > 8)
        expected = 2;
      result = gamepkg_x86_virtual_reserve(memory, 0x100000 + first * 4096,
                                           count * 4096, UINT32_MAX, 0,
                                           protection, commit);
      if (!expected)
        for (unsigned i = first; i < first + count; ++i)
          pages[i] = (Model){first + 1, protection, 0, protection, commit};
    } else {
      if (first + count > 64 || !pages[first].id)
        expected = 3;
      if (!expected)
        for (unsigned i = first; i < first + count; ++i)
          if (pages[i].id != pages[first].id)
            expected = 3;
      if (operation == 1 || operation == 2) {
        if (first + count > 64)
          expected = -1;
        unsigned needed = 0;
        if (!expected)
          for (unsigned i = first; i < first + count; ++i)
            needed += !pages[i].committed;
        if (!expected && operation == 2 && needed)
          expected = 4;
        if (!expected && committed + needed > 8)
          expected = 2;
        result =
            operation == 1
                ? gamepkg_x86_virtual_commit(memory, 0x100000 + first * 4096,
                                             count * 4096, protection)
                : gamepkg_x86_virtual_protect(memory, 0x100000 + first * 4096,
                                              count * 4096, protection);
        if (!expected)
          for (unsigned i = first; i < first + count; ++i) {
            pages[i].committed = 1;
            pages[i].protection = protection;
          }
      } else {
        result = gamepkg_x86_virtual_free(memory, 0x100000 + first * 4096,
                                          count * 4096, operation == 4);
        if (!expected) {
          uint32_t id = pages[first].id;
          if (operation == 4) {
            for (unsigned i = first + count; i < 64 && pages[i].id == id; ++i)
              pages[i].id = first + count + 1;
          }
          for (unsigned i = first; i < first + count; ++i) {
            if (operation == 4)
              pages[i] = (Model){0};
            else {
              pages[i].committed = 0;
              pages[i].protection = 0;
              pages[i].value = 0;
            }
          }
        }
      }
    }
    if (result.status != expected)
      fprintf(stderr,
              "model step=%u op=%u first=%u count=%u actual=%d expected=%d\n",
              step, operation, first, count, result.status, expected);
    assert(result.status == expected);
    ++checks;
    unsigned reserved = 0;
    committed = 0;
    for (unsigned i = 0; i < 64; ++i) {
      reserved += pages[i].id != 0;
      committed += pages[i].committed;
      GamePkgX86VirtualInfo q =
          gamepkg_x86_virtual_query(memory, 0x100000 + i * 4096 + 71);
      assert(!q.status && q.base == 0x100000 + i * 4096);
      assert(q.state == (pages[i].id ? (pages[i].committed ? 0x1000u : 0x2000u)
                                     : 0x10000u));
      if (pages[i].id) {
        assert(q.allocation_base == 0x100000 + (pages[i].id - 1) * 4096 &&
               q.allocation_protection == pages[i].allocation_protection);
        assert(q.protection == (pages[i].committed ? pages[i].protection : 0));
      }
      if (pages[i].committed && pages[i].protection != 1) {
        access(0x100000 + i * 4096, 0, 0, 0, 0);
        assert(loaded[0] == pages[i].value);
        if (pages[i].protection == 4 || pages[i].protection == 0x40) {
          words[0] = random_value();
          pages[i].value = words[0];
          access(0x100000 + i * 4096, 1, 0, 0, 0);
        }
      }
    }
    GamePkgX86VirtualStats s = stats();
    assert(s.reserved == reserved * 4096 && s.committed == committed * 4096 &&
           s.physical_limit == 8 * 4096);
  }
  teardown();
}
static void fixed_exclusions(void) {
  unsigned char bytes[16] = {19, 23, 29, 31};
  GamePkgX86Region r[] = {{0x104001, 1, 1, bytes + 3},
                          {0x101fff, 2, 1, bytes},
                          {0x103004, 4, 3, bytes + 4}};
  memory = gamepkg_x86_memory_create(r, 3, user);
  assert(memory);
  assert(!gamepkg_x86_virtual_configure_excluding_fixed(memory, 0x100000,
                                                        0x40000, 8192));
  gamepkg_x86_memory_bind(memory);
  GamePkgX86VirtualResult a =
      gamepkg_x86_virtual_reserve(memory, 0, 4096, UINT32_MAX, 0, 4, 0);
  GamePkgX86VirtualResult b =
      gamepkg_x86_virtual_reserve(memory, 0, 4096, UINT32_MAX, 0, 4, 0);
  assert(!a.status && a.base == 0x100000 && !b.status && b.base == 0x110000);
  assert(gamepkg_x86_virtual_query(memory, 0x102000).status == -2);
  assert(
      gamepkg_x86_virtual_reserve(memory, 0x100000, 0x6000, UINT32_MAX, 0, 4, 0)
          .status == 1);
  assert(gamepkg_x86_virtual_commit(memory, 0x102000, 4096, 4).status == 3);
  assert(gamepkg_x86_virtual_free(memory, 0x102000, 4096, 1).status == 3);
  assert(stats().reserved == 8192 && !stats().committed);
  width = 1;
  access(0x101fff, 0, 0, 0, 0);
  assert(loaded[0] == 19);
  access(0x102000, 0, 0, 0, 0);
  assert(loaded[0] == 23);
  access(0x102001, 0, 14, 0, 0x102001);
  width = 4;
  teardown();
}
int main(void) {
  memset(&fp, 0, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(fp.significand, 0xa5, sizeof(fp.significand));
  memset(xmm, 0x5a, sizeof(xmm));
  g_eflags = 0x202;
  for (user = 0; user < 2; ++user) {
    basic();
    modeled();
    fixed_exclusions();
  }
#ifdef GAMEPKG_VIRTUAL_ALLOCATOR_TEST
  user = 0;
  allocation_failures();
#endif
  printf("Virtual pages: %u checked transitions/accesses; independent "
         "16000-step oracle, reservation/commit/protect/decommit/partial "
         "release, physical exhaustion and fault/FP atomicity passed\n",
         checks);
  return 0;
}
