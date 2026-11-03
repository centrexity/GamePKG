/* Portable concrete RAM adapter checks: no x86 instructions or game data. */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include <assert.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <sched.h>
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Expected big endian
#endif
_Static_assert(sizeof(void *) == 4, "expected actual32bit pointers");
#endif
static _Thread_local uint32_t at, size, selected, source[4], destination[4];
static void store(void) {
  gamepkg_x86_memory_write(at, source, size, selected, 1, 0x10000, 0xabcdef12);
}
static void load(void) {
  gamepkg_x86_memory_read(at, destination, size, 0x10010, 0x76543210);
}
static void admit_read(void) {
  gamepkg_x86_memory_check(at, size, 0, 0x10020, 0x76543210);
}
static void admit_write(void) {
  gamepkg_x86_memory_check(at, size, 1, 0x10030, 0xabcdef12);
}
static void valid_configs(void) {
  unsigned char data[64] = {0};
  GamePkgX86Region r[65];
  for (unsigned i = 0; i < 65; ++i)
    r[i] = (GamePkgX86Region){i * 64, 64, 3, data};
  assert(!gamepkg_x86_memory_create(r, 65, 0));
  assert(!gamepkg_x86_memory_create(0, 1, 0));
  assert(!gamepkg_x86_memory_create(r, 1, 2));
  for (unsigned which = 0; which < 7; ++which) {
    GamePkgX86Region bad = r[0];
    if (which == 0)
      bad.length = 0;
    if (which == 1)
      bad.data = 0;
    if (which == 2)
      bad.permissions = 0;
    if (which == 3)
      bad.permissions = 4;
    if (which == 4)
      bad.base = UINT32_MAX - 31;
    if (which == 5)
      bad.data = (unsigned char *)(uintptr_t)(UINTPTR_MAX - 1);
    if (which == 6) {
      bad.base = 2;
      bad.length = UINT32_MAX;
    }
    assert(!gamepkg_x86_memory_create(&bad, 1, 0));
  }
  r[1] = r[0];
  assert(!gamepkg_x86_memory_create(r, 2, 0));
  r[1].base = 63;
  assert(!gamepkg_x86_memory_create(r, 2, 0));
  r[1].base = 64;
  GamePkgX86Memory *m = gamepkg_x86_memory_create(r, 2, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  gamepkg_x86_memory_bind(m);
  gamepkg_x86_memory_release(m);
  gamepkg_x86_memory_bind(0);
  m = gamepkg_x86_memory_create(0, 0, 0);
  assert(m);
  gamepkg_x86_memory_bind(m);
  gamepkg_x86_memory_release(m);
  at = 5;
  size = 1;
  selected = 1;
  GamePkgX86Fault f;
  assert(gamepkg_x86_run(store, &f) == 1 && f.vector == 14 &&
         f.fault_address == 5 && f.error_code == 2 && f.eip == 0x10000 &&
         f.ebp == 0xabcdef12);
  gamepkg_x86_memory_bind(0);
}
static unsigned accesses;
static void bytes_and_masks(void) {
  unsigned char data[80];
  GamePkgX86Region r = {1000, sizeof(data), 3, data};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&r, 1, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  GamePkgX86Fault f;
  const unsigned widths[] = {1, 2, 4, 8, 10, 16};
  for (unsigned w = 0; w < 6; ++w)
    for (unsigned offset = 0; offset < 32; ++offset)
      for (unsigned pattern = 0; pattern < (widths[w] == 10 ? 1026u : 258u);
           ++pattern) {
        memset(data, 0xa5, sizeof(data));
        at = 1000 + offset;
        size = widths[w];
        for (unsigned i = 0; i < 4; ++i)
          source[i] = UINT32_C(0xfedcba98) ^ (i * 0x1020304u) ^ pattern;
        unsigned variants = size == 10 ? 1024u : 256u;
        selected = pattern == variants       ? 0
                   : pattern == variants + 1 ? (1u << size) - 1
                                             : pattern & ((1u << size) - 1);
        assert(gamepkg_x86_run(admit_read, &f) == 0 &&
               gamepkg_x86_run(admit_write, &f) == 0);
        for (unsigned i = 0; i < sizeof(data); ++i)
          assert(data[i] == 0xa5);
        assert(gamepkg_x86_run(store, &f) == 0 && f.fault_address == 0 &&
               f.error_code == 0);
        for (unsigned i = 0; i < sizeof(data); ++i) {
          unsigned char expected = 0xa5;
          if (i >= offset && i < offset + size &&
              (selected & (1u << (i - offset))))
            expected = (unsigned char)(source[(i - offset) / 4] >>
                                       (8 * ((i - offset) % 4)));
          assert(data[i] == expected);
        }
        memset(destination, 0xcc, sizeof(destination));
        assert(gamepkg_x86_run(load, &f) == 0);
        for (unsigned i = 0; i < 16; ++i)
          assert(((destination[i / 4] >> (8 * (i % 4))) & 255u) ==
                 (i < size ? data[offset + i] : 0u));
        ++accesses;
      }
  gamepkg_x86_memory_bind(0);
  gamepkg_x86_memory_release(m);
}
static void atomic_faults(void) {
  unsigned char left[8], right[8];
  GamePkgX86Fault f;
  for (unsigned user = 0; user < 2; ++user)
    for (unsigned kind = 0; kind < 3; ++kind) {
      memset(left, 0x11, sizeof(left));
      memset(right, 0x22, sizeof(right));
      GamePkgX86Region r[2] = {{1000, 8, 3, left},
                               {1008, 8, kind == 1 ? 1u : 3u, right}};
      GamePkgX86Memory *m =
          gamepkg_x86_memory_create(r, kind == 0 ? 1 : 2, user);
      assert(m);
      gamepkg_x86_memory_bind(m);
      size = 16;
      at = 1000;
      for (unsigned i = 0; i < 4; ++i)
        source[i] = UINT32_MAX;
      int check_error = gamepkg_x86_run(admit_write, &f);
      assert(check_error == (kind != 2));
      if (check_error)
        assert(f.vector == 14 && f.fault_address == 1008 &&
               f.error_code ==
                   ((kind == 1 ? 1u : 0u) | 2u | (user ? 4u : 0u)) &&
               f.eip == 0x10030 && f.ebp == 0xabcdef12);
      check_error = gamepkg_x86_run(admit_read, &f);
      assert(check_error == (kind == 0));
      if (check_error)
        assert(f.vector == 14 && f.fault_address == 1008 &&
               f.error_code == (user ? 4u : 0u) && f.eip == 0x10020 &&
               f.ebp == 0x76543210);
      for (unsigned i = 0; i < 8; ++i)
        assert(left[i] == 0x11 && right[i] == 0x22);
      for (unsigned mask = 0; mask < 3; ++mask) {
        selected = mask == 0 ? 0 : mask == 1 ? 255 : 65535;
        int error = gamepkg_x86_run(store, &f);
        assert(error == (kind != 2));
        if (error) {
          assert(f.vector == 14 && f.fault_address == 1008 &&
                 f.error_code ==
                     ((kind == 1 ? 1u : 0u) | 2u | (user ? 4u : 0u)));
          for (unsigned i = 0; i < 8; ++i) {
            assert(left[i] == 0x11 && right[i] == 0x22);
          }
        } else
          for (unsigned i = 0; i < 16; ++i)
            assert((i < 8 ? left[i] : right[i - 8]) ==
                   ((selected & (1u << i)) ? 255u : (i < 8 ? 0x11u : 0x22u)));
      }
      /* A fault must release the lock, and must not leak old PF metadata. */
      size = 4;
      selected = 15;
      at = 1000;
      assert(gamepkg_x86_run(store, &f) == 0 && f.vector == 0 &&
             f.fault_address == 0 && f.error_code == 0);
      gamepkg_x86_memory_bind(0);
      gamepkg_x86_memory_release(m);
    }
  /* Failed reads leave the complete caller staging array untouched. */
  memset(left, 0x11, 8);
  GamePkgX86Region r = {1000, 8, 3, left};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&r, 1, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  at = 1004;
  size = 8;
  for (unsigned i = 0; i < 4; ++i)
    destination[i] = 0xdecafbad;
  assert(gamepkg_x86_run(load, &f) == 1 && f.vector == 14 &&
         f.fault_address == 1008 && f.error_code == 4);
  for (unsigned i = 0; i < 4; ++i)
    assert(destination[i] == 0xdecafbad);
  for (unsigned hint = 0; hint < 4; ++hint) {
    gamepkg_x86_prefetch(1000, hint);
    gamepkg_x86_prefetch(1008, hint);
    gamepkg_x86_prefetch(UINT32_MAX, hint);
  }
  gamepkg_x86_store_fence();
  gamepkg_x86_memory_bind(0);
  gamepkg_x86_memory_release(m);
}
static void wrapped_maps(void) {
  unsigned char high[4] = {0}, low[16] = {0};
  GamePkgX86Region r[2] = {{0, 16, 3, low}, {UINT32_MAX - 3, 4, 3, high}};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(r, 2, 1);
  assert(m);
  gamepkg_x86_memory_bind(m);
  GamePkgX86Fault f;
  for (unsigned width = 8; width <= 10; width += 2) {
    at = UINT32_MAX - 3;
    size = width;
    selected = (1u << width) - 1;
    source[0] = 0x44332211;
    source[1] = 0x88776655;
    source[2] = 0x0000aa99;
    source[3] = 0;
    assert(gamepkg_x86_run(admit_read, &f) == 0 &&
           gamepkg_x86_run(admit_write, &f) == 0);
    assert(gamepkg_x86_run(store, &f) == 0 && gamepkg_x86_run(load, &f) == 0);
    assert(destination[0] == source[0] && destination[1] == source[1] &&
           destination[2] == (width == 10 ? source[2] : 0u) &&
           destination[3] == 0 && high[0] == 0x11 && high[3] == 0x44 &&
           low[0] == 0x55 && low[3] == 0x88);
    if (width == 10)
      assert(low[4] == 0x99 && low[5] == 0xaa);
  }
  gamepkg_x86_memory_bind(0);
  gamepkg_x86_memory_release(m);
}
typedef struct {
  GamePkgX86Memory *memory;
  unsigned index;
} Args;
static atomic_uint started;
#ifdef _WIN32
static DWORD WINAPI worker(void *raw) {
#else
static void *worker(void *raw) {
#endif
  Args *a = raw;
  gamepkg_x86_memory_bind(a->memory);
  atomic_fetch_add_explicit(&started, 1, memory_order_release);
  at = 2000 + a->index * 16;
  size = 16;
  selected = 65535;
  for (unsigned round = 0; round < 2000; ++round) {
    for (unsigned i = 0; i < 4; ++i)
      source[i] = (a->index << 24) | round;
    GamePkgX86Fault f;
    assert(gamepkg_x86_run(store, &f) == 0);
    gamepkg_x86_store_fence();
    assert(gamepkg_x86_run(load, &f) == 0);
    for (unsigned i = 0; i < 4; ++i)
      assert(destination[i] == source[i]);
  }
  gamepkg_x86_memory_bind(0);
  return 0;
}
static void threads(void) {
  unsigned char data[128] = {0};
  GamePkgX86Region r = {2000, sizeof(data), 3, data};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&r, 1, 1);
  assert(m);
  Args args[8];
  atomic_init(&started, 0);
#ifdef _WIN32
  HANDLE h[8];
  for (unsigned i = 0; i < 8; ++i) {
    args[i] = (Args){m, i};
    h[i] = CreateThread(0, 0, worker, &args[i], 0, 0);
    assert(h[i]);
  }
  while (atomic_load_explicit(&started, memory_order_acquire) < 8)
    Sleep(0);
  gamepkg_x86_memory_release(m);
  assert(WaitForMultipleObjects(8, h, 1, INFINITE) == WAIT_OBJECT_0);
  for (unsigned i = 0; i < 8; ++i)
    CloseHandle(h[i]);
#else
  pthread_t h[8];
  for (unsigned i = 0; i < 8; ++i) {
    args[i] = (Args){m, i};
    assert(pthread_create(&h[i], 0, worker, &args[i]) == 0);
  }
  while (atomic_load_explicit(&started, memory_order_acquire) < 8)
    sched_yield();
  gamepkg_x86_memory_release(m);
  for (unsigned i = 0; i < 8; ++i)
    assert(pthread_join(h[i], 0) == 0);
#endif
  for (unsigned i = 0; i < 8; ++i)
    assert(data[i * 16] == (unsigned char)1999 && data[i * 16 + 3] == i);
}
static atomic_uint stress_done;
typedef struct {
  GamePkgX86Memory *memory;
  unsigned writer;
} StressArgs;
#ifdef _WIN32
static DWORD WINAPI stress_worker(void *raw) {
#else
static void *stress_worker(void *raw) {
#endif
  StressArgs *a = raw;
  gamepkg_x86_memory_bind(a->memory);
  atomic_fetch_add_explicit(&started, 1, memory_order_release);
  at = 2060;
  size = 16;
  selected = 65535;
  if (a->writer) {
    for (unsigned n = 0; n < 20000; ++n) {
      for (unsigned i = 0; i < 4; ++i)
        source[i] = (n & 1) ? 0x11111111u : 0xeeeeeeeeu;
      GamePkgX86Fault f;
      assert(gamepkg_x86_run(store, &f) == 0);
    }
    atomic_store_explicit(&stress_done, 1, memory_order_release);
  } else {
    do {
      GamePkgX86Fault f;
      assert(gamepkg_x86_run(load, &f) == 0);
      assert(destination[0] == 0 || destination[0] == 0x11111111u ||
             destination[0] == 0xeeeeeeeeu);
      for (unsigned i = 1; i < 4; ++i)
        assert(destination[i] == destination[0]);
    } while (!atomic_load_explicit(&stress_done, memory_order_acquire));
  }
  gamepkg_x86_memory_bind(0);
  return 0;
}
static void no_torn_transactions(void) {
  _Alignas(64) unsigned char data[128] = {0};
  GamePkgX86Region r = {2000, sizeof(data), 3, data};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&r, 1, 1);
  assert(m);
  StressArgs args[2] = {{m, 1}, {m, 0}};
  atomic_store(&started, 0);
  atomic_init(&stress_done, 0);
#ifdef _WIN32
  HANDLE h[2];
  for (unsigned i = 0; i < 2; ++i) {
    h[i] = CreateThread(0, 0, stress_worker, &args[i], 0, 0);
    assert(h[i]);
  }
  while (atomic_load_explicit(&started, memory_order_acquire) < 2)
    Sleep(0);
  gamepkg_x86_memory_release(m);
  assert(WaitForMultipleObjects(2, h, 1, INFINITE) == WAIT_OBJECT_0);
  for (unsigned i = 0; i < 2; ++i)
    CloseHandle(h[i]);
#else
  pthread_t h[2];
  for (unsigned i = 0; i < 2; ++i)
    assert(pthread_create(&h[i], 0, stress_worker, &args[i]) == 0);
  while (atomic_load_explicit(&started, memory_order_acquire) < 2)
    sched_yield();
  gamepkg_x86_memory_release(m);
  for (unsigned i = 0; i < 2; ++i)
    assert(pthread_join(h[i], 0) == 0);
#endif
}

static _Thread_local unsigned char image_buffer[512];
static _Thread_local uint32_t image_write;
static unsigned image_cases;
static void image_access(void) {
  gamepkg_x86_memory_bytes(at, image_buffer, size, image_write, 0x10040,
                           0x12345678);
}
static void image_transactions(void) {
  unsigned char data[512], old[512], expected[512];
  GamePkgX86Fault f;
  for (size = 1; size <= 512; ++size) {
    for (unsigned i = 0; i < 512; ++i)
      data[i] = old[i] = (unsigned char)(i * 17 + size);
    /* The linear address wraps across two separate, nonwrapping maps. */
    GamePkgX86Region wrap[2] = {{0xffffff00u, 256, 3, data},
                                {0, 256, 3, data + 256}};
    GamePkgX86Memory *m = gamepkg_x86_memory_create(wrap, 2, 1);
    assert(m);
    gamepkg_x86_memory_bind(m);
    at = 0xffffff00u;
    image_write = 0;
    memset(image_buffer, 0xa5, 512);
    assert(!gamepkg_x86_run(image_access, &f));
    assert(!memcmp(data, image_buffer, size));
    for (unsigned i = size; i < 512; ++i)
      assert(image_buffer[i] == 0xa5);
    image_write = 1;
    for (unsigned i = 0; i < 512; ++i)
      image_buffer[i] = (unsigned char)(i * 31 + size);
    memcpy(expected, image_buffer, 512);
    assert(!gamepkg_x86_run(image_access, &f));
    assert(!memcmp(data, expected, size));
    assert(!memcmp(data + size, old + size, 512 - size));
    image_cases += 2;
    gamepkg_x86_memory_bind(0);
    gamepkg_x86_memory_release(m);
    for (unsigned cut = 0; cut < size; ++cut)
      for (unsigned mode = 0; mode < 4; ++mode) {
        image_write = mode & 1;
        unsigned permission = mode < 2 ? 0 : image_write ? 1 : 2;
        GamePkgX86Region maps[2];
        size_t count = 0;
        if (cut)
          maps[count++] = (GamePkgX86Region){1000, cut, 3, data};
        if (permission)
          maps[count++] = (GamePkgX86Region){1000 + cut, size - cut, permission,
                                             data + cut};
        m = gamepkg_x86_memory_create(maps, count, 1);
        assert(m);
        gamepkg_x86_memory_bind(m);
        at = 1000;
        memcpy(data, old, 512);
        memset(image_buffer, 0xa5, 512);
        assert(gamepkg_x86_run(image_access, &f) == 1);
        assert(f.vector == 14 && f.fault_address == 1000 + cut &&
               f.error_code ==
                   (4u | (image_write ? 2u : 0u) | (permission ? 1u : 0u)) &&
               f.eip == 0x10040 && f.ebp == 0x12345678);
        assert(!memcmp(data, old, 512));
        for (unsigned i = 0; i < 512; ++i)
          assert(image_buffer[i] == 0xa5);
        ++image_cases;
        gamepkg_x86_memory_bind(0);
        gamepkg_x86_memory_release(m);
      }
  }
}

#include "critical_section_fixture.c"
int main(void) {
  critical_sections();
  image_transactions();
  valid_configs();
  bytes_and_masks();
  atomic_faults();
  wrapped_maps();
  threads();
  no_torn_transactions();
  printf("RAM transactions: %u byte/mask/access cases; mapping rejection, PF "
         "address/error, rollback, wrap, fence,16000 threaded transactions and "
         "owner/binding lifetime passed\n",
         accesses);
  printf("Bounded image RAM: %u whole-span cases, widths1..512, every failing "
         "byte, permissions, rollback and linear32 wrap passed\n",
         image_cases);
  return 0;
}
