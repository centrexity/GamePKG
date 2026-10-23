/* Synthetic RAM transactions and partial-progress checks, independent of the
 * protected-mode corpus. No host pointers enter generated guest expressions. */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_string.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <sys/resource.h>
#endif
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error This fixture requires a real big-endian target
#endif
_Static_assert(sizeof(void *) == 4, "This fixture requires a 32bit target");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static _Thread_local unsigned char memory[0x10000], expected[0x10000];
static _Thread_local unsigned completed;
/* WRAPPERS */
static void initialize(unsigned a16, unsigned df, unsigned count) {
  g_eax = 0x89abcdef;
  g_ecx = (a16 ? 0x76540000 : 0) | count;
  g_esi = (a16 ? 0xabcd0000 : 0) | 0x3000;
  g_edi = (a16 ? 0x12340000 : 0) | 0x7000;
  g_eflags = 0x8d7 | (df << 10);
  g_df = (int)df;
  g_fs_base = 0x2000;
  g_edx = 0x22223333;
  g_ebx = 0x44445555;
  g_esp = 0x66667777;
  g_ebp = 0x9999aaaa;
  completed = 0;
}
static uint32_t progress(uint32_t value, unsigned bytes, unsigned n,
                         unsigned a16, unsigned df) {
  uint32_t mask = a16 ? 65535 : UINT32_MAX;
  uint32_t delta = n * bytes;
  if (df)
    delta = 0u - delta;
  return (value & ~mask) | ((value + delta) & mask);
}
static void fault_case(unsigned form, unsigned df, unsigned n,
                       unsigned destination, unsigned readonly) {
  unsigned op = forms[form][0], width = forms[form][1], a16 = forms[form][2],
           repeat = forms[form][3], fs = forms[form][4];
  if ((op == 1 || op == 4) && !destination)
    return;
  if (op == 2 && destination)
    return;
  if (readonly && !(destination && op < 2))
    return;
  if (!repeat && n)
    return;
  initialize(a16, df, 7);
  uint32_t si = g_esi, di = g_edi, eax = g_eax, cx = g_ecx, flags = g_eflags,
           mask = UINT32_MAX >> (32 - width * 8);
  for (unsigned i = 0; i < sizeof(memory); ++i)
    memory[i] = (unsigned char)(i * 29 + form * 17 + n);
  for (unsigned i = 0; i < 7; ++i) {
    uint32_t s = (progress(si, width, i, a16, df) &
                  (a16 ? 65535u : UINT32_MAX)) +
                 (fs ? 0x2000 : 0),
             d = progress(di, width, i, a16, df) & (a16 ? 65535u : UINT32_MAX);
    uint32_t value = op == 4 ? eax : 0x11223344u + i * 0x04030201u;
    for (unsigned j = 0; j < width; ++j) {
      memory[s + j] = (unsigned char)(value >> (8 * j));
      memory[d + j] =
          (unsigned char)((value ^ (repeat == 2 ? 1u : 0u)) >> (8 * j));
    }
  }
  memcpy(expected, memory, sizeof(memory));
  for (unsigned i = 0; i < n; ++i) {
    uint32_t s = (progress(si, width, i, a16, df) &
                  (a16 ? 65535u : UINT32_MAX)) +
                 (fs ? 0x2000 : 0),
             d = progress(di, width, i, a16, df) & (a16 ? 65535u : UINT32_MAX);
    if (op == 0)
      for (unsigned j = 0; j < width; ++j)
        expected[d + j] = memory[s + j];
    if (op == 1)
      for (unsigned j = 0; j < width; ++j)
        expected[d + j] = (unsigned char)(eax >> (8 * j));
    if (op == 2) {
      uint32_t value = 0;
      for (unsigned j = 0; j < width; ++j)
        value |= (uint32_t)memory[s + j] << (8 * j);
      eax = (eax & ~mask) | value;
    }
  }
  uint32_t target =
      destination
          ? (progress(di, width, n, a16, df) & (a16 ? 65535u : UINT32_MAX))
          : (progress(si, width, n, a16, df) & (a16 ? 65535u : UINT32_MAX)) +
                (fs ? 0x2000 : 0);
  target += width - 1;
  GamePkgX86Region regions[3] = {
      {0, target, 3, memory},
      {target, 1, readonly ? 1u : 0u, memory + target},
      {target + 1, sizeof(memory) - target - 1, 3, memory + target + 1}};
  if (!readonly)
    regions[1] = regions[2];
  GamePkgX86Memory *map =
      gamepkg_x86_memory_create(regions, readonly ? 3 : 2, 0);
  assert(map);
  gamepkg_x86_memory_bind(map);
  GamePkgX86Fault fault;
  int result = gamepkg_x86_run(instructions[form], &fault);
  if (result != 1 || fault.vector != 14 || g_eflags != flags ||
      memcmp(memory, expected, sizeof(memory))) {
    fprintf(stderr,
            "string fault form%u op%u width%u a16%u rep%u fs%u df%u n%u dest%u "
            "ro%u result%d vector%u flags%08x/%08x\n",
            form, op, width, a16, repeat, fs, df, n, destination, readonly,
            result, fault.vector, g_eflags, flags);
    assert(0);
  }
  uint32_t want_si =
      (op == 0 || op == 2 || op == 3) ? progress(si, width, n, a16, df) : si;
  uint32_t want_di = op != 2 ? progress(di, width, n, a16, df) : di;
  uint32_t want_cx = repeat ? progress(cx, 1, n, a16, 1) : cx;
  assert(g_eax == eax && g_ecx == want_cx && g_esi == want_si &&
         g_edi == want_di && !completed);
  assert(fault.eip == 0x10000 && fault.ebp == 0xabcdef12 &&
         fault.fault_address == target &&
         fault.error_code ==
             ((destination && op < 2 ? 2u : 0u) | (readonly ? 1u : 0u)));
  assert(fault.eax == eax && fault.ecx == want_cx && fault.esi == want_si &&
         fault.edi == want_di && fault.eflags == flags &&
         fault.direction == df && fault.fs_base == 0x2000 &&
         fault.mxcsr == 0x1f80);
  assert(g_edx == 0x22223333 && g_ebx == 0x44445555 && g_esp == 0x66667777 &&
         g_ebp == 0xabcdef12);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(map);
  // Repair the same arena and retry at this known instruction entry. Earlier
  // writes/register progress must not be repeated or silently discarded.
  unsigned total = repeat ? 7 : 1;
  for (unsigned i = n; i < total; ++i) {
    uint32_t s =
        (progress(si, width, i, a16, df) & (a16 ? 65535u : UINT32_MAX)) +
        (fs ? 0x2000 : 0);
    uint32_t d = progress(di, width, i, a16, df) & (a16 ? 65535u : UINT32_MAX);
    if (op == 0)
      for (unsigned j = 0; j < width; ++j)
        expected[d + j] = memory[s + j];
    if (op == 1)
      for (unsigned j = 0; j < width; ++j)
        expected[d + j] = (unsigned char)(eax >> (8 * j));
    if (op == 2) {
      uint32_t value = 0;
      for (unsigned j = 0; j < width; ++j)
        value |= (uint32_t)memory[s + j] << (8 * j);
      eax = (eax & ~mask) | value;
    }
  }
  GamePkgX86Region repaired = {0, sizeof(memory), 3, memory};
  map = gamepkg_x86_memory_create(&repaired, 1, 0);
  assert(map);
  gamepkg_x86_memory_bind(map);
  assert(!gamepkg_x86_run(instructions[form], &fault) && completed == 1);
  assert(!memcmp(memory, expected, sizeof(memory)) && g_eax == eax);
  assert(g_ecx == (repeat ? (cx & (a16 ? 0xffff0000u : 0u)) : cx));
  assert(g_esi == ((op == 0 || op == 2 || op == 3)
                       ? progress(si, width, total, a16, df)
                       : si));
  assert(g_edi == (op != 2 ? progress(di, width, total, a16, df) : di));
  if (op < 3)
    assert(g_eflags == flags);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(map);
}
static void span_faults(void) {
  for (unsigned form = 0; form < sizeof(forms) / sizeof(*forms); ++form) {
    unsigned op = forms[form][0], width = forms[form][1];
    if (forms[form][2] || forms[form][3] != 1 || !forms[form][4] ||
        width == 1 || !(op == 0 || op == 2 || op == 3))
      continue;
    initialize(0, 0, 7);
    g_esi = UINT32_MAX - 2 * width + 2;
    uint32_t source = g_esi + g_fs_base, oldsi = g_esi, olddi = g_edi,
             oldflags = g_eflags;
    for (unsigned i = 0; i < sizeof(memory); ++i)
      memory[i] = (unsigned char)(i * 17);
    for (unsigned j = 0; j < width; ++j)
      memory[g_edi + j] = memory[source + j];
    GamePkgX86Region region = {0, sizeof(memory), 3, memory};
    GamePkgX86Memory *map = gamepkg_x86_memory_create(&region, 1, 0);
    assert(map);
    gamepkg_x86_memory_bind(map);
    GamePkgX86Fault fault;
    assert(gamepkg_x86_run(instructions[form], &fault) == 1 &&
           fault.vector == 13 && fault.eip == 0x10000 &&
           fault.ebp == 0xabcdef12 && fault.fault_address == 0 &&
           fault.error_code == 0);
    assert(g_esi == oldsi + width &&
           g_edi == (op == 2 ? olddi : olddi + width) && g_ecx == 6 &&
           g_eflags == oldflags && !completed);
    gamepkg_x86_memory_bind(NULL);
    gamepkg_x86_memory_release(map);
  }
}
static void zero_counts(void) {
  gamepkg_x86_memory_bind(NULL);
  for (unsigned i = 0; i < sizeof(forms) / sizeof(*forms); ++i)
    if (forms[i][3]) {
      unsigned a16 = forms[i][2];
      initialize(a16, 0, 0);
      g_esi = g_edi = UINT32_MAX;
      uint32_t cx = g_ecx, flags = g_eflags;
      GamePkgX86Fault fault;
      assert(!gamepkg_x86_run(instructions[i], &fault) && completed == 1 &&
             g_esi == UINT32_MAX && g_edi == UINT32_MAX && g_ecx == cx &&
             g_eax == 0x89abcdef && g_eflags == flags && g_ebp == 0x9999aaaa);
    }
}
static void cases(void) {
  zero_counts();
  span_faults();
  for (unsigned form = 0; form < sizeof(forms) / sizeof(*forms); ++form)
    for (unsigned df = 0; df < 2; ++df)
      for (unsigned n = 0; n < 4; ++n)
        for (unsigned destination = 0; destination < 2; ++destination)
          for (unsigned readonly = 0; readonly < 2; ++readonly)
            fault_case(form, df, n, destination, readonly);
}
/* CHECKPOINT_FAULT_CASES */
#ifdef _WIN32
static DWORD WINAPI worker(void *arg) {
  (void)arg;
  for (unsigned i = 0; i < 10; ++i)
    cases();
  return 0;
}
#else
static void *worker(void *arg) {
  (void)arg;
  for (unsigned i = 0; i < 10; ++i)
    cases();
  return NULL;
}
#endif
int main(void) {
#ifndef _WIN32
  struct rlimit no_core = {0, 0};
  assert(!setrlimit(RLIMIT_CORE, &no_core));
#endif
  checkpoint_fault_cases();
  cases();
#ifdef _WIN32
  HANDLE ids[8];
  for (unsigned i = 0; i < 8; ++i) {
    ids[i] = CreateThread(NULL, 0, worker, NULL, 0, NULL);
    assert(ids[i]);
  }
  for (unsigned i = 0; i < 8; ++i) {
    assert(WaitForSingleObject(ids[i], INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(ids[i]));
  }
#else
  pthread_t ids[8];
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_create(&ids[i], NULL, worker, NULL));
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_join(ids[i], NULL));
#endif
  puts("Strings: zero counts unbound, partial committed data/registers, "
       "original compare flags, permission/boundary faults and 80 worker "
       "iterations passed");
  return 0;
}
