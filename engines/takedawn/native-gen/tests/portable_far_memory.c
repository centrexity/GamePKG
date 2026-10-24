/* Maintainer synthetic descriptor RAM/fault/concurrency qualification. */
#include "gamepkg_x86_far_memory.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big-endian compiler required
#endif
_Static_assert(sizeof(void *) == 4, "actual big-endian32 required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static GamePkgX86FarTables tables;
static uint32_t selector, target, cpl, operand16, action, address;
static GamePkgX86CodeTarget result, old_result;
static GamePkgX86CodeTarget *destination;
static uint32_t read_words[2], old_words[2];
static int returned;
static unsigned checks;
static GamePkgX87State x87;
static uint32_t xmm[8][4];
static GamePkgX86FpSnapshot expected;
static const GamePkgX86FpSource source = {
    &x87, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
static void entry(void) {
  if (action == 0)
    returned =
        gamepkg_x86_far_resolve_code(tables, selector, target, cpl, operand16,
                                     0x123456, 0x789abc, destination);
  else if (action == 1)
    gamepkg_x86_memory_descriptor_read(address, read_words, 0x123456, 0x789abc);
  else if (action == 2)
    gamepkg_x86_memory_descriptor_accessed(address, 0x123456, 0x789abc);
  else {
    uint32_t words[4];
    gamepkg_x86_memory_read(address, words, 1, 0x123456, 0x789abc);
  }
}
static void defaults(void) {
  tables = (GamePkgX86FarTables){0xff8, 15, 0xff8, 15, 1};
  selector = 8;
  target = 0x1234;
  cpl = operand16 = action = 0;
  address = 0x1000;
  memset(&result, 0x5a, sizeof(result));
  old_result = result;
  destination = &result;
  read_words[0] = 0x12345678;
  read_words[1] = 0xabcdef01;
  memcpy(old_words, read_words, sizeof(old_words));
  returned = 123;
  g_eax = 0x11223344;
  g_eflags = 0x10647; /* Includes raw RF; no normalization on either path. */
  g_df = 1;
  g_ecx = 0x2211;
  g_esp = 0x987654;
  g_fs_base = 0xabbacddc;
}
static void check(unsigned vector, unsigned error, unsigned fault_address) {
  GamePkgX86FpFault fault;
  int status = gamepkg_x86_run_with_fp(entry, &fault, &source);
  ++checks;
  assert(status == (vector ? 1 : 0));
  if (vector) {
    assert(fault.core.vector == vector && fault.core.error_code == error &&
           fault.core.fault_address == fault_address &&
           fault.core.eip == 0x123456 && fault.core.ebp == 0x789abc &&
           fault.core.eax == 0x11223344 && fault.core.ecx == 0x2211 &&
           fault.core.esp == 0x987654 && fault.core.eflags == 0x10647 &&
           fault.core.direction == 1 && fault.core.fs_base == 0xabbacddc);
    assert(!memcmp(&fault.fp.x87, &expected.x87, sizeof(x87)) &&
           !memcmp(fault.fp.xmm, xmm, sizeof(xmm)));
    assert(returned == 123 && !memcmp(&result, &old_result, sizeof(result)) &&
           !memcmp(read_words, old_words, sizeof(read_words)));
  }
  assert(g_eax == 0x11223344 && g_eflags == 0x10647 && g_df == 1 &&
         g_ecx == 0x2211 && g_esp == 0x987654 && g_fs_base == 0xabbacddc);
}
static void bind_regions(const GamePkgX86Region *regions, size_t count,
                         unsigned user) {
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(regions, count, user);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  gamepkg_x86_memory_release(memory);
}
static void put(unsigned char *p, uint32_t lo, uint32_t hi) {
  for (unsigned i = 0; i < 4; ++i) {
    p[i] = (unsigned char)(lo >> (8 * i));
    p[i + 4] = (unsigned char)(hi >> (8 * i));
  }
}
static void success(void) {
  check(0, 0, 0);
  assert(returned == 0 && result.vector == 0 && result.error == 0 &&
         result.selector == ((selector & 0xfffcu) | cpl) &&
         result.offset == (operand16 ? target & 65535u : target) &&
         result.base == 0x89abcdef && result.limit == 0x12345fff &&
         result.attributes == (cpl == 3 ? 0xcfb : 0xc9b) &&
         result.mark_accessed == 0);
}
static void selectors_before_ram(void) {
  bind_regions(NULL, 0, 1);
  for (unsigned rpl = 0; rpl < 4; ++rpl) {
    defaults();
    selector = rpl;
    check(13, 0, 0);
    defaults();
    selector = 16 | rpl;
    check(13, 16, 0);
    defaults();
    selector = 12 | rpl;
    tables.ldt_usable = 0;
    check(13, 12, 0);
    defaults();
    selector = 12 | rpl;
    tables.ldt_limit = 14;
    check(13, 12, 0);
  }
  for (unsigned invalid = 0; invalid < 9; ++invalid) {
    defaults();
    if (invalid == 0)
      cpl = 4;
    if (invalid == 1)
      operand16 = 2;
    if (invalid == 2)
      selector = 65536;
    if (invalid == 3)
      tables.gdt_limit = 65536;
    if (invalid == 4)
      tables.ldt_usable = 2;
    if (invalid == 5)
      destination = NULL;
    if (invalid == 6)
      destination = (void *)((unsigned char *)&result + 1);
    if (invalid == 7)
      destination = (void *)(UINTPTR_MAX & ~(uintptr_t)3);
    if (invalid == 8) {
      cpl = 4;
      selector = 0;
    }
    check(0, 0, 0);
    assert(returned == -1 && !memcmp(&result, &old_result, sizeof(result)));
  }
  defaults();
  check(14, 0, 0x1000); /* Implicit supervisor access, despite user_fault=1. */
  defaults();
  action = 3;
  check(14, 4, 0x1000); /* Ordinary memory keeps the original user PF bit. */
}
static void descriptor_cases(void) {
  unsigned char ram[16], saved[16];
  GamePkgX86Region region = {0x1000, sizeof(ram), 3, ram};
  for (unsigned accessed = 0; accessed < 2; ++accessed)
    for (unsigned table = 0; table < 2; ++table)
      for (unsigned small = 0; small < 2; ++small)
        for (unsigned user = 0; user < 2; ++user) {
          memset(ram, 0xa5, sizeof(ram));
          put(ram, 0xcdef2345,
              0x89c19aab | (accessed << 8) | (user ? 0x6000 : 0));
          memcpy(saved, ram, sizeof(ram));
          bind_regions(&region, 1, user);
          defaults();
          selector = (table ? 12u : 8u) | (user ? 3u : 0u);
          operand16 = small;
          target = small ? 0xa5a51234 : 0x1234;
          cpl = user ? 3 : 0;
          success();
          saved[5] |= 1;
          assert(!memcmp(ram, saved, sizeof(ram)));
        }
  /* Each failure competes with a forbidden write; semantic faults win. */
  const uint32_t hi[] = {0x89c112ab, 0x89c11aab, 0x89c11aab, 0x89c19aab,
                         0x89c18aab};
  const unsigned vectors[] = {13, 11, 13, 13, 0};
  for (unsigned i = 0; i < 5; ++i) {
    memset(ram, 0xa5, sizeof(ram));
    put(ram, 0xcdef2345, hi[i]);
    memcpy(saved, ram, sizeof(ram));
    region.permissions = 1;
    bind_regions(&region, 1, 1);
    defaults();
    if (i == 2)
      cpl = 3;
    if (i == 3)
      target = 0x12346000;
    check(vectors[i], i == 3 ? 0 : 8, 0);
    assert(!memcmp(ram, saved, sizeof(ram)));
    if (i == 4)
      assert(returned == -1 && !memcmp(&result, &old_result, sizeof(result)));
  }
  for (unsigned accessed = 0; accessed < 2; ++accessed) {
    put(ram, 0xcdef2345, 0x89c19aab | (accessed << 8));
    memcpy(saved, ram, sizeof(ram));
    region.permissions = 1;
    bind_regions(&region, 1, 1);
    defaults();
    if (accessed)
      success();
    else
      check(14, 3, 0x1005);
    assert(!memcmp(ram, saved, sizeof(ram)));
  }
  /* Whole eight-byte admission precedes any descriptor observation/update. */
  for (unsigned length = 1; length < 8; ++length) {
    put(ram, 0xcdef2345, 0x89c19aab);
    memcpy(saved, ram, sizeof(ram));
    region.length = length;
    region.permissions = 3;
    bind_regions(&region, 1, 1);
    defaults();
    check(14, 0, 0x1000 + length);
    defaults();
    action = 1;
    check(14, 0, 0x1000 + length);
    assert(!memcmp(ram, saved, sizeof(ram)));
  }
  region.length = sizeof(ram);
  region.permissions = 2;
  bind_regions(&region, 1, 1);
  defaults();
  check(14, 1, 0x1000);
  /* LDT entry0 is valid, unlike GDT entry0; avoid invented null rejection. */
  region.permissions = 3;
  put(ram, 0xcdef2345, 0x89c19aab);
  bind_regions(&region, 1, 0);
  defaults();
  selector = 4;
  tables.ldt_base = 0x1000;
  tables.ldt_limit = 7;
  success();
}
static void split_and_wrap(void) {
  for (unsigned split = 1; split < 8; ++split)
    for (unsigned wrap = 0; wrap < 2; ++wrap)
      for (unsigned readonly = 0; readonly < 2; ++readonly) {
        unsigned char first[8], second[8], image[8], old_first[8],
            old_second[8];
        put(image, 0xcdef2345, 0x89c19aab);
        memset(first, 0x5a, sizeof(first));
        memset(second, 0xa5, sizeof(second));
        memcpy(first, image, split);
        memcpy(second, image + split, 8 - split);
        memcpy(old_first, first, sizeof(first));
        memcpy(old_second, second, sizeof(second));
        uint32_t base = wrap ? 0u - split : 0x1000;
        GamePkgX86Region regions[2] = {
            {base, split, readonly && split > 5 ? 1 : 3, first},
            {base + split, 8 - split, readonly && split <= 5 ? 1 : 3, second}};
        bind_regions(regions, 2, 1);
        defaults();
        tables.gdt_base = base - 8u;
        if (readonly)
          check(14, 3, base + 5u);
        else {
          success();
          if (split > 5)
            old_first[5] |= 1;
          else
            old_second[5 - split] |= 1;
        }
        assert(!memcmp(first, old_first, sizeof(first)) &&
               !memcmp(second, old_second, sizeof(second)));
      }
}
static void all_rights_bytes(void) {
  unsigned char ram[3] = {0x12, 0, 0x34};
  for (unsigned permissions = 1; permissions <= 3; ++permissions)
    for (unsigned byte = 0; byte < 256; ++byte) {
      ram[1] = byte;
      GamePkgX86Region region = {0x1000, 3, permissions, ram};
      bind_regions(&region, 1, 1);
      defaults();
      action = 2;
      address = 0x1001;
      if (permissions == 2)
        check(14, 1, 0x1001);
      else if (permissions == 1 && !(byte & 1))
        check(14, 3, 0x1001);
      else {
        check(0, 0, 0);
        assert(ram[1] == (byte | 1));
      }
      if (permissions == 2 || (permissions == 1 && !(byte & 1)))
        assert(ram[1] == byte);
      assert(ram[0] == 0x12 && ram[2] == 0x34);
    }
}
static GamePkgX86Memory *shared;
#ifdef _WIN32
static DWORD WINAPI worker(void *unused) {
#else
static void *worker(void *unused) {
#endif
  (void)unused;
  gamepkg_x86_memory_bind(shared);
  for (unsigned i = 0; i < 2000; ++i) {
    uint32_t words[4], value;
    do {
      gamepkg_x86_memory_read(0x1000, words, 4, 0, 0);
      value = words[0];
    } while (gamepkg_x86_memory_rmw(0x1000, 4, 2, value & ~1u, value, 0, 0) !=
             value);
    gamepkg_x86_memory_rmw(0x1000, 4, 1, 2, 0, 0, 0);
    gamepkg_x86_memory_descriptor_accessed(0x1000, 0, 0);
  }
  gamepkg_x86_memory_bind(NULL);
  return 0;
}
static void contention(void) {
  unsigned char ram[6] = {0, 0, 0, 0, 0xa5, 0x5a};
  GamePkgX86Region region = {0x1000, 4, 3, ram};
  shared = gamepkg_x86_memory_create(&region, 1, 1);
  assert(shared);
#ifdef _WIN32
  HANDLE threads[8];
  for (unsigned i = 0; i < 8; ++i) {
    threads[i] = CreateThread(NULL, 0, worker, NULL, 0, NULL);
    assert(threads[i]);
  }
  for (unsigned i = 0; i < 8; ++i) {
    assert(WaitForSingleObject(threads[i], INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(threads[i]));
  }
#else
  pthread_t threads[8];
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_create(threads + i, NULL, worker, NULL));
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_join(threads[i], NULL));
#endif
  gamepkg_x86_memory_bind(shared);
  gamepkg_x86_memory_release(shared);
  uint32_t words[4];
  gamepkg_x86_memory_read(0x1000, words, 4, 0, 0);
  assert(words[0] == 32001 && ram[4] == 0xa5 && ram[5] == 0x5a);
  gamepkg_x86_memory_bind(NULL);
}
int main(void) {
  memset(&x87, 0xa5, sizeof(x87));
  memset(xmm, 0x5a, sizeof(xmm));
  GamePkgX87State original_x87 = x87;
  GamePkgX86FpFault captured;
  assert(!gamepkg_x86_capture_registers(0, 0, &captured, &source));
  expected = captured.fp;
  selectors_before_ram();
  descriptor_cases();
  split_and_wrap();
  all_rights_bytes();
  gamepkg_x86_memory_bind(NULL);
  contention();
  assert(!memcmp(&x87, &original_x87, sizeof(x87)) &&
         !memcmp(xmm, expected.xmm, sizeof(xmm)));
  printf("Far descriptor memory: %u fault/priority/permission/wrap/raw-state "
         "cases and 16000 eight-worker accessed/RMW transitions passed; "
         "pointer_bits=%u\n",
         checks, (unsigned)(8 * sizeof(void *)));
  return 0;
}
