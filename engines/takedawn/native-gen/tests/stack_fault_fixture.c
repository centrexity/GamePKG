/* Logical little-endian test arena and fault injection. Not a RAM adapter. */
#include "gamepkg_x86_fault.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error This fixture requires an actual big-endian target
#endif
_Static_assert(sizeof(void *) == 4, "This fixture requires a 32bit target");
#endif
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <sys/resource.h>
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static _Thread_local unsigned char memory[4096];
static _Thread_local unsigned reads, writes, completed, fail_read, fail_write;
static _Thread_local uint32_t read_address, write_address, work_bp;
uint32_t xbox_MmioLoad(uint32_t address, int width) {
  assert(width == 2 || width == 4);
  ++reads;
  read_address = address;
  if (fail_read)
    gamepkg_x86_page_fault(address, 0, 0x10000, work_bp);
  uint32_t v = 0;
  for (unsigned i = 0; i < (unsigned)width; ++i)
    v |= (uint32_t)memory[(address + i) & 4095] << (8 * i);
  return v;
}
void xbox_MmioStore(uint32_t address, uint32_t value, int width) {
  assert(width == 2 || width == 4);
  ++writes;
  write_address = address;
  if (fail_write)
    gamepkg_x86_page_fault(address, 2, 0x10000, work_bp);
  for (unsigned i = 0; i < (unsigned)width; ++i)
    memory[(address + i) & 4095] = (unsigned char)(value >> (8 * i));
}
/* WRAPPERS */
static void run(unsigned op, uint32_t sp, uint32_t bp, uint32_t value,
                unsigned fault) {
  for (unsigned i = 0; i < sizeof(memory); ++i)
    memory[i] = (unsigned char)(i * 73 + 19);
  uint32_t initial_eax = 0x01020304, flags = 0x247fd7;
  unsigned kind = forms[op][0], width = forms[op][1];
  uint32_t source = kind == 7 ? bp : sp;
  for (unsigned i = 0; i < width; ++i)
    memory[(source + i) & 4095] = (unsigned char)(value >> (8 * i));
  unsigned char before[4096];
  memcpy(before, memory, sizeof(before));
  g_eax = initial_eax;
  g_ecx = 0x22223333;
  g_edx = 0x44445555;
  g_ebx = 0x66667777;
  g_esp = sp;
  g_ebp = bp;
  work_bp = bp;
  g_esi = 0x88889999;
  g_edi = 0xaaaabbbb;
  g_eflags = flags;
  g_fs_base = 0xfffffff0;
  g_df = 1;
  reads = writes = completed = fail_read = fail_write = 0;
  if (fault == 1)
    fail_read = 1;
  else if (fault == 2)
    fail_write = 1;
  GamePkgX86Fault result;
  int failed = gamepkg_x86_run(instructions[op], &result);
  unsigned uses_read = kind != 0,
           uses_write = kind == 0 || kind == 1 || (kind >= 4 && kind <= 6);
  unsigned expected_failure =
      (fault == 1 && uses_read) || (fault == 2 && uses_write);
  assert(failed == (int)expected_failure);
  assert(g_ecx == 0x22223333 && g_edx == 0x44445555 && g_ebx == 0x66667777 &&
         g_esi == 0x88889999 && g_edi == 0xaaaabbbb && g_eflags == flags &&
         g_df == 1);
  if (expected_failure) {
    if (g_esp != sp) {
      fprintf(
          stderr,
          "stack fault op%u kind%u width%u fault%u changed ESP %08x -> %08x\n",
          op, kind, width, fault, sp, g_esp);
      assert(0);
    }
    assert(work_bp == bp && g_eax == initial_eax &&
           !memcmp(before, memory, sizeof(before)) && !completed);
    assert(result.vector == 14 && result.eip == 0x10000 && result.esp == sp &&
           result.ebp == bp);
    assert(result.eax == initial_eax && result.ecx == 0x22223333 &&
           result.edx == 0x44445555 && result.ebx == 0x66667777 &&
           result.esi == 0x88889999 && result.edi == 0xaaaabbbb &&
           result.eflags == flags && result.fs_base == 0xfffffff0 &&
           result.direction == 1 && result.mxcsr == 0x1f80);
    assert(result.fault_address == (fault == 1 ? read_address : write_address));
    assert(result.error_code == (fault == 1 ? 0u : 2u));
  } else {
    assert(completed == 1 && reads == uses_read && writes == uses_write);
    uint32_t next = kind == 7 ? bp + width : sp + width;
    uint32_t mask = width == 2 ? 0xffff : UINT32_MAX;
    if (kind <= 1) {
      assert(g_esp == sp - width && work_bp == bp && g_eax == initial_eax &&
             write_address == sp - width);
      if (kind == 1)
        assert(read_address == sp);
      uint32_t expected = kind == 0 ? sp : value;
      for (unsigned i = 0; i < width; ++i)
        assert(memory[(write_address + i) & 4095] ==
               (unsigned char)(expected >> (8 * i)));
    } else if (kind == 2) {
      assert(g_esp == next &&
             g_eax == ((initial_eax & ~mask) | (value & mask)) &&
             work_bp == bp);
    } else if (kind == 3) {
      assert(g_esp == ((next & ~mask) | (value & mask)) && work_bp == bp &&
             g_eax == initial_eax);
    } else if (kind == 7) {
      assert(g_esp == next && work_bp == ((bp & ~mask) | (value & mask)) &&
             g_eax == initial_eax);
    } else {
      uint32_t expected = next;
      if (kind == 5)
        expected += initial_eax * 4 - 4;
      else if (kind == 6)
        expected += g_fs_base;
      assert(g_esp == next && write_address == expected && read_address == sp &&
             work_bp == bp && g_eax == initial_eax);
      for (unsigned i = 0; i < width; ++i)
        assert(memory[(write_address + i) & 4095] ==
               (unsigned char)(value >> (8 * i)));
    }
  }
}
static void cases(unsigned seed) {
  static const uint32_t positions[] = {0x10400, 0,       0xfffffffc,
                                       0x1fffe, 0x1ffff, 0xffff0000};
  for (unsigned op = 0; op < 16; ++op)
    for (unsigned i = 0; i < 6; ++i)
      for (unsigned fault = 0; fault < 3; ++fault) {
        /* Modular arena checks address arithmetic only, not segment/page
         * wrapping. */
        uint32_t sp = positions[i], bp = positions[(i + 3) % 6],
                 value = 0x89abcdef ^ seed;
        run(op, sp, bp, value, fault);
      }
}
#ifdef _WIN32
static DWORD WINAPI worker(void *arg) {
  for (unsigned i = 0; i < 100; ++i)
    cases((unsigned)(uintptr_t)arg + i);
  return 0;
}
#else
static void *worker(void *arg) {
  for (unsigned i = 0; i < 100; ++i)
    cases((unsigned)(uintptr_t)arg + i);
  return NULL;
}
#endif
int main(void) {
#ifndef _WIN32
  struct rlimit no_core = {0, 0};
  assert(!setrlimit(RLIMIT_CORE, &no_core));
#endif
  cases(0);
#ifdef _WIN32
  HANDLE ids[8];
  for (unsigned i = 0; i < 8; ++i) {
    ids[i] = CreateThread(NULL, 0, worker, (void *)(uintptr_t)i, 0, NULL);
    assert(ids[i]);
  }
  for (unsigned i = 0; i < 8; ++i) {
    assert(WaitForSingleObject(ids[i], INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(ids[i]));
  }
#else
  pthread_t ids[8];
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_create(&ids[i], NULL, worker, (void *)(uintptr_t)i));
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_join(ids[i], NULL));
#endif
  puts("Stack transactions: 16 forms, source/store faults, post-POP addresses, "
       "16bit subregisters, wrap arithmetic and 800 worker iterations passed");
  return 0;
}
