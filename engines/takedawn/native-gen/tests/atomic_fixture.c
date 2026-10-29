#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include <assert.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
_Thread_local uint32_t g_eax, g_ebx, g_ecx, g_edx, g_esi, g_edi, g_esp, g_ebp,
    g_eflags, g_fs_base, g_mxcsr;
_Thread_local int g_df;
static _Thread_local unsigned completed;
static unsigned char arena[256];
static GamePkgX86Memory *shared;
/* WRAPPERS */
static uint32_t mask(unsigned b) { return UINT32_MAX >> (32 - b * 8); }
static uint32_t load(const unsigned char *p, unsigned b) {
  uint32_t v = 0;
  for (unsigned i = 0; i < b; ++i)
    v |= (uint32_t)p[i] << (i * 8);
  return v;
}
static void store(unsigned char *p, unsigned b, uint32_t v) {
  for (unsigned i = 0; i < b; ++i)
    p[i] = (unsigned char)(v >> (i * 8));
}
static uint32_t ref_flags(uint32_t a, uint32_t b, unsigned bytes, int add,
                          uint32_t before) {
  uint32_t m = mask(bytes), sign = (m >> 1) + 1, r = (add ? a + b : a - b) & m;
  unsigned ones = 0;
  for (unsigned i = 0; i < 8; ++i)
    ones += (r >> i) & 1;
  int64_t sa = (a & sign) ? (int64_t)a - (int64_t)m - 1 : a;
  int64_t sb = (b & sign) ? (int64_t)b - (int64_t)m - 1 : b;
  int64_t sr = add ? sa + sb : sa - sb;
  return (before & ~0x8d5u) | 2u |
         ((add ? (uint64_t)a + b > m : a < b) ? 1u : 0u) |
         (!(ones & 1) ? 4u : 0u) | ((a ^ b ^ r) & 16u) | (r ? 0 : 64u) |
         (r & sign ? 128u : 0u) |
         ((sr < -(int64_t)sign || sr >= (int64_t)sign) ? 2048u : 0u);
}
static void setup(unsigned mode, uint32_t a, uint32_t b, uint32_t acc,
                  unsigned bytes) {
  memset(arena, 0xa5, sizeof(arena));
  g_eax = acc;
  g_ecx = b;
  g_ebx = mode == 3 ? 0xabc00040 : a;
  g_esi = mode == 3 ? 0xdef00041 : 0x16161616;
  g_edi = 0x20000081;
  g_fs_base = mode == 2 ? 0x10000000 : 0;
  g_edx = 0x12121212;
  g_ebp = 0xabab1234;
  g_esp = 0x45454545;
  g_eflags = 0x00240ed7;
  g_mxcsr = 0x1f80;
  g_df = 1;
  completed = 0;
  store(arena + 129, bytes, a);
}
static GamePkgX86Memory *map(unsigned mode, unsigned bytes, unsigned bad,
                             unsigned permission) {
  uint32_t base = mode == 3 ? 0 : mode == 2 ? 0x30000000 : 0x20000000;
  GamePkgX86Region r[3] = {{base, 129 + bad, 3, arena},
                           {base + 129 + bad, 1, permission, arena + 129 + bad},
                           {base + 130 + bad, 126 - bad, 3, arena + 130 + bad}};
  size_t n = 3;
  if (bad == bytes) {
    r[0].length = 256;
    n = 1;
  } else if (!permission) {
    r[1] = r[2];
    n = 2;
  }
  GamePkgX86Memory *m = gamepkg_x86_memory_create(r, n, 0);
  assert(m);
  gamepkg_x86_memory_bind(m);
  return m;
}
static uint64_t cases, faults;
static void logical(void) {
  for (unsigned f = 0; f < sizeof(forms) / sizeof(forms[0]); ++f) {
    if (f >= 54)
      continue;
    unsigned kind = forms[f][0], bytes = forms[f][1], mode = forms[f][2];
    uint32_t m = mask(bytes);
    unsigned count = bytes == 1 ? 65536 : 2048;
    for (unsigned v = 0; v < count; ++v) {
      uint32_t a = bytes == 1 ? v >> 8 : (v * 0x9e3779b9u),
               b = bytes == 1 ? v & 255 : (v * 0xdeadbeefu + 0x80000000u),
               acc = (v & 1) ? a : a ^ 0x80u;
      a = (a & m) | (~m & 0x12345678);
      b = (b & m) | (~m & 0x87654321);
      acc = (acc & m) | (~m & 0xcafebabe);
      setup(mode, a, b, acc, bytes);
      if (mode == 5)
        a = b;
      GamePkgX86Memory *mem = 0;
      if (mode >= 1 && mode <= 4)
        mem = map(mode, bytes, bytes, 3);
      uint32_t ae = a & m, be = b & m, ac = acc & m;
      uint32_t wanta = kind == 0   ? be
                       : kind == 1 ? (ae + be) & m
                       : ac == ae  ? be
                                   : ae;
      uint32_t wantb = kind <= 1 ? ae : be,
               wantacc = kind == 2 && ac != ae ? ae : ac;
      uint32_t flags = kind == 0
                           ? g_eflags
                           : ref_flags(kind == 1 ? ae : ac, kind == 1 ? be : ae,
                                       bytes, kind == 1, g_eflags);
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(instructions[f], &fault) == 0);
      assert(completed == 1);
      if (mode >= 1 && mode <= 4) {
        assert(load(arena + 129, bytes) == wanta);
        for (unsigned i = 0; i < 256; ++i)
          if (i < 129 || i >= 129 + bytes)
            assert(arena[i] == 0xa5);
      } else if (mode == 0)
        assert(g_ebx == ((a & ~m) | wanta));
      if (mode == 5)
        wantb = wanta;
      assert(g_ecx == ((b & ~m) | wantb));
      assert(g_eax == ((acc & ~m) | wantacc));
      assert(g_eflags == flags);
      assert(g_edx == 0x12121212 && g_ebp == 0xabab1234 &&
             g_esp == 0x45454545 && g_df == 1 && g_mxcsr == 0x1f80);
      gamepkg_x86_memory_bind(0);
      gamepkg_x86_memory_release(mem);
      ++cases;
    }
    if (mode < 1 || mode > 4)
      continue;
    for (unsigned bad = 0; bad < bytes; ++bad)
      for (unsigned perm = 0; perm < 3; ++perm)
        for (unsigned equal = 0; equal < 2; ++equal) {
          setup(mode, 0x12345678, 0x87654321, equal ? 0x12345678 : 0x33333333,
                bytes);
          unsigned char before[256];
          memcpy(before, arena, 256);
          GamePkgX86Memory *mem = map(mode, bytes, bad, perm);
          GamePkgX86Fault fault;
          assert(gamepkg_x86_run(instructions[f], &fault) == 1);
          assert(!completed && fault.vector == 14 && fault.eip == 0x10000 &&
                 fault.ebp == 0xabab1234);
          assert(fault.error_code == (perm ? 3u : 2u));
          uint32_t base = mode == 3 ? 0 : mode == 2 ? 0x30000000 : 0x20000000;
          assert(fault.fault_address == base + 129 + bad);
          assert(g_ecx == 0x87654321 &&
                 g_eax == (equal ? 0x12345678u : 0x33333333u) &&
                 g_eflags == 0x00240ed7);
          assert(memcmp(before, arena, 256) == 0);
          uint32_t unlocked[4];
          gamepkg_x86_memory_read(base + 16, unlocked, 4, 0x10000, 0);
          assert(unlocked[0] == 0xa5a5a5a5);
          gamepkg_x86_memory_bind(0);
          gamepkg_x86_memory_release(mem);
          // Repair and execute the same known entry; a rejected RMW changed
          // nothing.
          mem = map(mode, bytes, bytes, 3);
          assert(gamepkg_x86_run(instructions[f], &fault) == 0 &&
                 completed == 1);
          gamepkg_x86_memory_bind(0);
          gamepkg_x86_memory_release(mem);
          ++faults;
        }
  }
}
#ifdef _WIN32
static DWORD WINAPI worker(void *arg) {
#else
static void *worker(void *arg) {
#endif
  unsigned kind = (unsigned)(uintptr_t)arg;
  gamepkg_x86_memory_bind(shared);
  for (unsigned i = 0; i < 10000; ++i) {
    if (kind == 0) {
      g_ecx = 1;
      g_edi = 0x20000081;
      g_eax = 0;
      op_34();
      assert(g_ecx < 80000);
    } else if (kind == 1) {
      uint32_t w[4];
      do {
        gamepkg_x86_memory_read(0x20000081, w, 4, 0x10000, 0);
        g_eax = w[0];
        g_ecx = w[0] + 1;
        g_edi = 0x20000081;
        op_52();
      } while (!(g_eflags & 64));
    } else {
      while (gamepkg_x86_memory_rmw(0x20000090, 4, 0, 1, 0, 0x10000, 0)) {
      }
      uint32_t w[4];
      gamepkg_x86_memory_read(0x20000081, w, 4, 0x10000, 0);
      ++w[0];
      gamepkg_x86_memory_write(0x20000081, w, 4, 15, 0, 0x10000, 0);
      gamepkg_x86_memory_rmw(0x20000090, 4, 0, 0, 0, 0x10000, 0);
    }
  }
  gamepkg_x86_memory_bind(0);
  return 0;
}
static void concurrency(void) {
  for (unsigned kind = 0; kind < 3; ++kind) {
    memset(arena, 0, 256);
    GamePkgX86Region r = {0x20000000, 256, 3, arena};
    shared = gamepkg_x86_memory_create(&r, 1, 0);
    assert(shared);
#ifdef _WIN32
    HANDLE t[8];
    for (unsigned i = 0; i < 8; ++i) {
      t[i] = CreateThread(NULL, 0, worker, (void *)(uintptr_t)kind, 0, NULL);
      assert(t[i]);
    }
    for (unsigned i = 0; i < 8; ++i) {
      assert(WaitForSingleObject(t[i], INFINITE) == WAIT_OBJECT_0);
      assert(CloseHandle(t[i]));
    }
#else
    pthread_t t[8];
    for (unsigned i = 0; i < 8; ++i)
      assert(!pthread_create(&t[i], 0, worker, (void *)(uintptr_t)kind));
    for (unsigned i = 0; i < 8; ++i)
      assert(!pthread_join(t[i], 0));
#endif
    assert(load(arena + 129, 4) == 80000);
    assert(load(arena + 144, 4) == 0);
    gamepkg_x86_memory_release(shared);
  }
}

static void boundaries(void) {
  unsigned splits = 0, limits = 0;
  for (unsigned f = 0; f < 54; ++f) {
    unsigned kind = forms[f][0], bytes = forms[f][1], mode = forms[f][2];
    if (mode < 1 || mode > 4)
      continue;
    uint32_t m = mask(bytes);
    for (unsigned bad = 0; bad < bytes; ++bad) {
      setup(mode, 0x12345678, 0x87654321, 0x12345678, bytes);
      GamePkgX86Memory *mem = map(mode, bytes, bad, 3);
      GamePkgX86Fault fault;
      assert(!gamepkg_x86_run(instructions[f], &fault) && completed == 1);
      uint32_t a = 0x12345678 & m, b = 0x87654321 & m;
      assert(load(arena + 129, bytes) == (kind == 1 ? (a + b) & m : b));
      assert(g_ecx == (kind < 2 ? ((0x87654321 & ~m) | a) : 0x87654321));
      gamepkg_x86_memory_bind(0);
      gamepkg_x86_memory_release(mem);
      ++splits;
    }
  }
  for (unsigned kind = 0; kind < 3; ++kind)
    for (unsigned bytes = 2; bytes <= 4; bytes *= 2) {
      setup(1, 0x12345678, 0x87654321, 0x12345678, bytes);
      g_edi = UINT32_MAX - bytes + 2;
      GamePkgX86Fault fault;
      unsigned f = kind * 18 + (bytes == 2 ? 6 : 12) + 1;
      assert(gamepkg_x86_run(instructions[f], &fault) == 1 && !completed &&
             fault.vector == 13 && fault.eip == 0x10000 &&
             fault.ebp == 0xabab1234);
      assert(g_eax == 0x12345678 && g_ecx == 0x87654321 &&
             g_eflags == 0x00240ed7);
      ++limits;
    }
  assert(splits == 84 && limits == 6);
  puts("Atomic: 84 split-region successes,6 segment-limit faults before "
       "unbound RAM access passed");
}
static uint32_t word(FILE *f) {
  unsigned char b[4];
  assert(fread(b, 1, 4, f) == 4);
  return (uint32_t)b[0] | (uint32_t)b[1] << 8 | (uint32_t)b[2] << 16 |
         (uint32_t)b[3] << 24;
}
static void replay(const char *path) {
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
  uint32_t probe = 1;
  assert(sizeof(void *) == 4 && *(unsigned char *)&probe == 0);
#endif
  FILE *file = fopen(path, "rb");
  assert(file);
  char magic[8];
  assert(fread(magic, 1, 8, file) == 8 && !memcmp(magic, "GPKATMv1", 8));
  assert(word(file) == 59 * 1024);
  for (unsigned f = 0; f < 59; ++f)
    for (unsigned scenario = 0; scenario < 1024; ++scenario) {
      assert(word(file) == f && word(file) == scenario);
      uint32_t expected[7];
      for (unsigned i = 0; i < 7; ++i)
        expected[i] = word(file);
      struct AtomicInput in = atomic_input(f, scenario);
      memset(arena, 0xa5, 256);
      store(arena + 129, 4, in.initial_memory);
      unsigned char want[256];
      memcpy(want, arena, 256);
      store(want + 129, 4, expected[6]);
      GamePkgX86Region region = {in.base, 256, 3, arena};
      GamePkgX86Memory *mem = gamepkg_x86_memory_create(&region, 1, 0);
      assert(mem);
      gamepkg_x86_memory_bind(mem);
      g_eax = in.r.eax;
      g_ecx = in.r.ecx;
      g_ebx = in.r.ebx;
      g_esi = in.r.esi;
      g_edi = in.r.edi;
      g_eflags = in.r.flags;
      g_df = (in.r.flags >> 10) & 1;
      g_fs_base = f < 54 && f % 6 == 2 ? 0x2000 : 0;
      g_ebp = 0xabcdef12;
      g_esp = 0x11223344;
      completed = 0;
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(instructions[f], &fault) == 0 && completed == 1);
      assert(g_eax == expected[0] && g_ecx == expected[1] &&
             g_ebx == expected[2] && g_esi == expected[3] &&
             g_edi == expected[4] && g_eflags == expected[5]);
      assert(!memcmp(arena, want, 256) && g_ebp == 0xabcdef12 &&
             g_esp == 0x11223344);
      gamepkg_x86_memory_bind(0);
      gamepkg_x86_memory_release(mem);
    }
  assert(fgetc(file) == EOF && !ferror(file));
  assert(!fclose(file));
  printf("Atomic CPU replay: 60416 full register/arena states,59 "
         "opcode/width/address/FS/LOCK forms; pointer_bits=%u\n",
         (unsigned)(sizeof(void *) * 8));
}
int main(int argc, char **argv) {
  if (argc == 2) {
    replay(argv[1]);
    return 0;
  }
  assert(argc == 1);
  logical();
  boundaries();
  concurrency();
  printf("Atomic: %llu result cases, %llu faults and repairs, three "
         "eight-worker 80000-update contention runs passed\n",
         (unsigned long long)cases, (unsigned long long)faults);
  return 0;
}
