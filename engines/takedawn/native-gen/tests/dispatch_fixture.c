#ifndef _GNU_SOURCE
#define _GNU_SOURCE 1
#endif
#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_dispatch.h"
#include "gamepkg_x86_memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static _Thread_local unsigned char ram[1048576];
static _Thread_local GamePkgX87State x87;
static _Thread_local uint32_t xmm[8][4];
static _Thread_local uintptr_t low_stack, high_stack;
static _Thread_local unsigned steps;
static _Thread_local uint32_t site_pc, site_bp;
uint32_t xbox_MmioLoad(uint32_t address, int bytes) {
  uint32_t words[4];
  assert(!gamepkg_x86_dispatch_site(&site_pc, &site_bp));
  gamepkg_x86_memory_read(address, words, (uint32_t)bytes, site_pc, site_bp);
  return words[0];
}
void xbox_MmioStore(uint32_t address, uint32_t value, int bytes) {
  uint32_t words[4] = {value, 0, 0, 0};
  assert(!gamepkg_x86_dispatch_site(&site_pc, &site_bp));
  gamepkg_x86_memory_write(address, words, (uint32_t)bytes, (1u << bytes) - 1,
                           0, site_pc, site_bp);
}
/* PROGRAM */
static void stop(uint32_t pc) {
  (void)pc;
  gamepkg_x86_dispatch_stop(g_ebp);
}
static void probe(uint32_t pc) {
  uintptr_t at;
#if defined(__x86_64__)
  __asm__ volatile("mov %%rsp,%0" : "=r"(at));
#elif defined(__i386__)
  __asm__ volatile("mov %%esp,%0" : "=r"(at));
#elif defined(__mips__)
  __asm__ volatile("move %0,$sp" : "=r"(at));
#elif defined(__arm__) || defined(__aarch64__)
  __asm__ volatile("mov %0,sp" : "=r"(at));
#else
#error Native stack-pointer probe needs a maintainer implementation for this target
#endif
  if (at < low_stack)
    low_stack = at;
  if (at > high_stack)
    high_stack = at;
  ++steps;
  gamepkg_block_00010000(pc);
}
static GamePkgX86Memory *bind_ram(unsigned hole, unsigned permission) {
  GamePkgX86Region r[3];
  size_t count;
  if (hole == 1048576) {
    r[0] = (GamePkgX86Region){0x100000, 1048576, 3, ram};
    count = 1;
  } else {
    assert(hole > 0 && hole < 1048575);
    r[0] = (GamePkgX86Region){0x100000, hole, 3, ram};
    r[1] = (GamePkgX86Region){0x100000 + hole, 1, permission, ram + hole};
    r[2] =
        (GamePkgX86Region){0x100001 + hole, 1048575 - hole, 3, ram + hole + 1};
    count = 3;
    if (!permission) {
      r[1] = r[2];
      count = 2;
    }
  }
  GamePkgX86Memory *m = gamepkg_x86_memory_create(r, count, 0);
  assert(m);
  gamepkg_x86_memory_bind(m);
  return m;
}

static void unbind_ram(GamePkgX86Memory *m) {
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(m);
}
static void put(uint32_t address, uint32_t v) {
  assert(address >= 0x100000 && address <= 0x1ffffc);
  for (unsigned i = 0; i < 4; ++i)
    ram[address - 0x100000 + i] = (unsigned char)(v >> (8 * i));
}
static void reset(void) {
  memset(ram, 0x5a, sizeof(ram));
  g_eax = g_ecx = g_edx = g_ebx = g_esi = g_edi = 0;
  g_ebp = 0xabcdef12;
  g_esp = 0x1fff00;
  g_fs_base = 0;
  g_eflags = 0x202;
  g_df = 0;
  put(g_esp, 0xf0000000);
  low_stack = UINTPTR_MAX;
  high_stack = steps = 0;
}
static GamePkgX86DispatchEntry map[256];
static size_t count;
static unsigned critical_calls;
static void critical_import(uint32_t pc) {
  assert(pc == 0xf0100000 || pc == 0xf0100004 || pc == 0xf0100008);
  uint32_t address = gamepkg_x86_dispatch_load(g_esp + 4, 1, pc, g_ebp);
  uint32_t op = pc == 0xf0100008 ? 0 : pc == 0xf0100000 ? 2 : 3;
  assert(!gamepkg_x86_critical_section(address, 7, op, pc, g_ebp));
  uint32_t target = gamepkg_x86_dispatch_ret(4, 4, pc, g_ebp);
  ++critical_calls;
  gamepkg_x86_dispatch_transfer(target, g_ebp);
}
static void init(void) {
  assert(gamepkg_x86_generated_count + 5 < 256);
  map[0] = (GamePkgX86DispatchEntry){0x2000, stop};
  count = 1;
  for (size_t i = 0; i < gamepkg_x86_generated_count; ++i) {
    map[count] = gamepkg_x86_generated_entries[i];
    if (map[count].address >= 0x10000 && map[count].address < 0x10009)
      map[count].block = probe;
    ++count;
  }
  map[count++] = (GamePkgX86DispatchEntry){0xf0000000, stop};
  map[count++] = (GamePkgX86DispatchEntry){0xf0100000, critical_import};
  map[count++] = (GamePkgX86DispatchEntry){0xf0100004, critical_import};
  map[count++] = (GamePkgX86DispatchEntry){0xf0100008, critical_import};
}
static void normal(void) {
  GamePkgX86DispatchExit out;
  GamePkgX86Memory *m;
  reset();
  m = bind_ram(1048576, 3);
  g_ecx = 120001;
  assert(!gamepkg_x86_dispatch_run(map, count, 0x10000, &out, NULL));
  assert(g_ecx == 0 && g_esp == 0x1fff04 && steps > 120000 &&
         high_stack - low_stack < 4096);
  unbind_ram(m);
  reset();
  m = bind_ram(1048576, 3);
  assert(!gamepkg_x86_dispatch_run(map, count, 0x11000, &out, NULL));
  assert(g_eax == 13 && g_esp == 0x1fff04);
  unbind_ram(m);
  reset();
  m = bind_ram(1048576, 3);
  put(g_esp, 0x11210);
  put(g_esp + 4, 0xf0000000);
  assert(!gamepkg_x86_dispatch_run(map, count, 0x11200, &out, NULL));
  assert(g_eax == 0x11203 && g_esp == 0x1fff08);
  unbind_ram(m);
  reset();
  m = bind_ram(1048576, 3);
  g_eax = 0x11310;
  assert(!gamepkg_x86_dispatch_run(map, count, 0x11300, &out, NULL));
  assert(g_eax == 0x1234 && g_esp == 0x1fff04);
  unbind_ram(m);
  reset();
  m = bind_ram(1048576, 3);
  assert(!gamepkg_x86_dispatch_run(map, count, 0x11400, &out, NULL));
  assert(g_eax == 2 && g_esp == 0x1fff04);
  unbind_ram(m);
  reset();
  m = bind_ram(1048576, 3);
  g_eax = 0x11310;
  assert(!gamepkg_x86_dispatch_run(map, count, 0x11810, &out, NULL));
  assert(g_eax == 0x1234 && g_esp == 0x1fff04);
  unbind_ram(m);
  reset();
  m = bind_ram(1048576, 3);
  put(g_esp, 0x98762000);
  assert(!gamepkg_x86_dispatch_run(map, count, 0x11610, &out, NULL));
  assert(g_esp == 0x1fff02 && out.pc == 0x2000);
  unbind_ram(m);
  reset();
  m = bind_ram(1048576, 3);
  assert(!gamepkg_x86_dispatch_run(map, count, 0x11620, &out, NULL));
  assert(g_esp == 0x207f03);
  unbind_ram(m);
  reset();
  m = bind_ram(1048576, 3);
  put(g_esp, 0x41414141);
  assert(gamepkg_x86_dispatch_run(map, count, 0x11600, &out, NULL) == 2 &&
         out.pc == 0x41414141 && g_esp == 0x1fff04);
  GamePkgX86FpFault zero = {0};
  assert(!memcmp(&out.fault, &zero, sizeof(zero)));
  unbind_ram(m);
}
static void overlapping_entries(void) {
  for (unsigned which = 0; which < 4; ++which) {
    reset();
    GamePkgX86Memory *m = bind_ram(1048576, 3);
    GamePkgX86DispatchExit out;
    uint32_t pc = which == 0 ? 0x11a00 : which == 1 ? 0x11a01 : 0x11b00;
    if (which == 3)
      g_eflags |= 64;
    assert(!gamepkg_x86_dispatch_run(map, count, pc, &out, NULL));
    assert(g_eax == (which == 1 || which == 3 ? 0xc300002au : 0x2ab8u) &&
           g_esp == 0x1fff04);
    unbind_ram(m);
  }
}
static void faults(void) {
  GamePkgX86DispatchExit out;
  GamePkgX86Memory *m;
  for (unsigned bad = 0; bad < 4; ++bad)
    for (unsigned kind = 0; kind < 2; ++kind) {
      reset();
      uint32_t sp = g_esp;
      unsigned hole =
          (sp - 0x100000) + (kind ? 0 : 0) -
          bad; // RET gap inside its source or CALL destination below it.
      if (kind) {
        hole = sp - 0x100000 - 4 + bad;
      } else {
        hole = sp - 0x100000 + bad;
      }
      m = bind_ram(hole, kind ? 1 : 0);
      unsigned char before[16];
      memcpy(before, ram + sp - 0x100000 - 8, 16);
      uint32_t pc = kind ? 0x11700 : 0x11600;
      assert(gamepkg_x86_dispatch_run(map, count, pc, &out, NULL) == 1 &&
             out.fault.core.vector == 14 && out.pc == pc && g_esp == sp);
      assert(out.fault.core.error_code == (kind ? 3u : 0u));
      assert(!memcmp(before, ram + sp - 0x100000 - 8, 16));
      unbind_ram(m);
      m = bind_ram(1048576, 3);
      assert(!gamepkg_x86_dispatch_run(map, count, pc, &out, NULL));
      assert(g_esp == sp + 4);
      unbind_ram(m);
    }
  reset();
  uint32_t sp = g_esp;
  put(sp, 0x11210);
  m = bind_ram(sp - 0x100000 + 3, 0);
  unsigned char before[4];
  memcpy(before, ram + sp - 0x100000 - 4, 4);
  assert(gamepkg_x86_dispatch_run(map, count, 0x11200, &out, NULL) == 1 &&
         out.pc == 0x11200 && g_esp == sp && out.fault.core.error_code == 0);
  assert(!memcmp(before, ram + sp - 0x100000 - 4, 4));
  unbind_ram(m);
  reset();
  g_esp = UINT32_MAX - 1;
  assert(gamepkg_x86_dispatch_run(map, count, 0x11600, &out, NULL) == 1 &&
         out.fault.core.vector == 12 && g_esp == UINT32_MAX - 1);
  reset();
  g_eax = UINT32_MAX - 1;
  assert(gamepkg_x86_dispatch_run(map, count, 0x11800, &out, NULL) == 1 &&
         out.fault.core.vector == 13);
  reset();
  g_esp = UINT32_MAX - 1;
  assert(gamepkg_x86_dispatch_run(map, count, 0x11200, &out, NULL) == 1 &&
         out.fault.core.vector == 12);
  reset();
  g_eax = 0x100080;
  g_ebp = 0x123;
  m = bind_ram(0x80, 0);
  assert(gamepkg_x86_dispatch_run(map, count, 0x11900, &out, NULL) == 1 &&
         out.pc == 0x11905 && out.fault.core.ebp == 0x12345678 &&
         g_ebp == 0x12345678);
  unbind_ram(m);
}
static void fp_retry(void) {
  reset();
  GamePkgX86Memory *m = bind_ram(1048576, 3);
  GamePkgX86FpSource source = {.x87 = &x87};
  GamePkgX86FpTarget target = {.x87 = &x87};
  memset(&x87, 0, sizeof(x87));
  for (unsigned i = 0; i < 8; ++i) {
    x87.significand[i] = UINT64_C(0x8000000000000001) + i;
    x87.sign_exponent[i] = (uint16_t)(0x7fff + i);
    source.xmm[i] = xmm[i];
    target.xmm[i] = xmm[i];
    for (unsigned j = 0; j < 4; ++j)
      xmm[i][j] = 0x7f800001 + i * 4 + j;
  }
  x87.control = 0x37f;
  x87.tags = 0xaaaa;
  GamePkgX87State original = x87;
  uint32_t old_xmm[8][4];
  memcpy(old_xmm, xmm, sizeof(xmm));
  g_eax = 21;
  g_edx = 0;
  g_ecx = 0;
  g_ebx = 7;
  g_esi = 9;
  gamepkg_x86_set_mxcsr(0x1f80, 0, 0);
  GamePkgX86DispatchExit out;
  assert(gamepkg_x86_dispatch_run(map, count, 0x11500, &out, &source) == 1 &&
         out.pc == 0x11501 && out.fault.core.vector == 0 && g_ebx == 8 &&
         g_esi == 9);
  assert(!memcmp(&out.fault.fp.x87, &original, sizeof(x87)) &&
         !memcmp(out.fault.fp.xmm, old_xmm, sizeof(xmm)));
  memset(&x87, 0, sizeof(x87));
  memset(xmm, 0, sizeof(xmm));
  assert(!gamepkg_x86_restore_fault_registers(&out.fault, &target));
  assert(!memcmp(&x87, &original, sizeof(x87)) &&
         !memcmp(xmm, old_xmm, sizeof(xmm)));
  g_ecx = 3;
  assert(!gamepkg_x86_dispatch_run(map, count, out.pc, &out, &source));
  assert(g_eax == 7 && g_ebx == 8 && g_esi == 10 && g_esp == 0x1fff04);
  unbind_ram(m);
}
static void breakpoint_retry(void) {
  reset();
  GamePkgX86Memory *m = bind_ram(1048576, 3);
  GamePkgX86FpSource source = {.x87 = &x87};
  GamePkgX86FpTarget target = {.x87 = &x87};
  for (unsigned i = 0; i < 8; ++i) {
    source.xmm[i] = xmm[i];
    target.xmm[i] = xmm[i];
  }
  g_ebx = 40;
  g_esi = 6;
  g_eflags = 0xcd7;
  g_df = 1;
  GamePkgX86FpFault before;
  assert(!gamepkg_x86_capture_registers(0x11c02, g_ebp, &before, &source));
  GamePkgX86DispatchExit out;
  assert(gamepkg_x86_dispatch_run(map, count, 0x11c00, &out, &source) == 1);
  assert(out.pc == 0x11c02 && out.fault.core.vector == 3 &&
         !out.fault.core.error_code && !out.fault.core.fault_address);
  assert(g_ebx == 41 && g_esi == 6 && g_esp == before.core.esp && g_df == 1);
  assert(!memcmp(&out.fault.fp, &before.fp, sizeof(before.fp)));
  assert(!gamepkg_x86_restore_fault_registers(&out.fault, &target));
  assert(!gamepkg_x86_dispatch_run(map, count, out.pc, &out, &source));
  assert(g_ebx == 41 && g_esi == 7 && g_esp == before.core.esp + 4);
  unbind_ram(m);
}

static _Thread_local GamePkgX86FpSource *changing_source;
static _Thread_local GamePkgX87State alternate_x87;
static _Thread_local uint32_t alternate_xmm[8][4];
static void binding_hook(uint32_t pc) {
  (void)pc;
  changing_source->x87 = &alternate_x87;
  for (unsigned i = 0; i < 8; ++i)
    changing_source->xmm[i] = alternate_xmm[i];
  gamepkg_x86_dispatch_transfer(0x11501, g_ebp);
}
static void frozen_bindings(void) {
  reset();
  GamePkgX86FpSource source = {.x87 = &x87};
  for (unsigned i = 0; i < 8; ++i)
    source.xmm[i] = xmm[i];
  changing_source = &source;
  memset(&x87, 0, sizeof(x87));
  x87.significand[3] = 0x1122334455667788ull;
  x87.tags = 0x1234;
  memset(&alternate_x87, 0x5a, sizeof(alternate_x87));
  memset(alternate_xmm, 0xa5, sizeof(alternate_xmm));
  GamePkgX86DispatchEntry local[256];
  memcpy(local, map, count * sizeof(*map));
  local[count] = (GamePkgX86DispatchEntry){0xf3000000, binding_hook};
  GamePkgX86DispatchExit out;
  g_eax = 21;
  g_ecx = g_edx = 0;
  assert(gamepkg_x86_dispatch_run(local, count + 1, 0xf3000000, &out,
                                  &source) == 1 &&
         out.pc == 0x11501 &&
         out.fault.fp.x87.significand[3] == 0x1122334455667788ull &&
         out.fault.fp.x87.tags == 0x1234);
  assert(source.x87 == &alternate_x87); // Caller descriptor changed, live run
                                        // used its frozen copy.
}
static void invalid_block(uint32_t pc) { (void)pc; }
static void double_block(uint32_t pc) {
  gamepkg_x86_dispatch_transfer(pc, 1);
  gamepkg_x86_dispatch_transfer(pc, 2);
}
static void validation(void) {
  GamePkgX86DispatchExit out, before;
  memset(&out, 0xa5, sizeof(out));
  memcpy(&before, &out, sizeof(out));
  GamePkgX86DispatchEntry bad[] = {{2, stop}, {1, stop}};
  assert(gamepkg_x86_dispatch_run(bad, 2, 1, &out, NULL) == -1 &&
         !memcmp(&out, &before, sizeof(out)));
  bad[1].address = 2;
  assert(gamepkg_x86_dispatch_run(bad, 2, 2, &out, NULL) == -1);
  bad[1].address = 3;
  bad[1].block = NULL;
  assert(gamepkg_x86_dispatch_run(bad, 2, 2, &out, NULL) == -1);
  assert(gamepkg_x86_dispatch_run(map, 0, 0, &out, NULL) == -1);
  assert(gamepkg_x86_dispatch_run(map, 1048577, 0, &out, NULL) == -1);
  GamePkgX86FpSource source = {0};
  assert(gamepkg_x86_dispatch_run(map, count, 0, &out, &source) == -1 &&
         !memcmp(&out, &before, sizeof(out)));
  GamePkgX86DispatchEntry malformed = {1, invalid_block};
  assert(gamepkg_x86_dispatch_run(&malformed, 1, 1, &out, NULL) == 3);
  malformed.block = double_block;
  assert(gamepkg_x86_dispatch_run(&malformed, 1, 1, &out, NULL) == 3);
  uint32_t a, b;
  assert(gamepkg_x86_dispatch_site(&a, &b) == -1);
  memset(&out, 0xa5, sizeof(out));
  before = out;
  GamePkgX86FpSource alias = {.x87 = &out.fault.fp.x87};
  for (unsigned i = 0; i < 8; ++i)
    alias.xmm[i] = out.fault.fp.xmm[i];
  assert(gamepkg_x86_dispatch_run(map, count, 0x10000, &out, &alias) == -1 &&
         !memcmp(&out, &before, sizeof(out)));
  union {
    GamePkgX86DispatchEntry entries[256];
    GamePkgX86DispatchExit output;
  } overlap_map;
  overlap_map.entries[0] = (GamePkgX86DispatchEntry){1, stop};
  assert(gamepkg_x86_dispatch_run(overlap_map.entries, 1, 1,
                                  &overlap_map.output, NULL) == -1);
}
#ifdef _WIN32
static DWORD WINAPI worker(void *arg) {
#else
static void *worker(void *arg) {
#endif
  (void)arg;
  for (unsigned n = 0; n < 32; ++n) {
    reset();
    GamePkgX86Memory *m = bind_ram(1048576, 3);
    GamePkgX86DispatchExit out;
    g_ecx = 128 + n;
    assert(!gamepkg_x86_dispatch_run(map, count, 0x10000, &out, NULL) &&
           g_ecx == 0 && g_esp == 0x1fff04);
    unbind_ram(m);
  }
  return 0;
}
static void workers(void) {
#ifdef _WIN32
  HANDLE t[8];
  for (unsigned i = 0; i < 8; ++i) {
    t[i] = CreateThread(NULL, 0, worker, NULL, 0, NULL);
    assert(t[i]);
  }
  for (unsigned i = 0; i < 8; ++i) {
    assert(WaitForSingleObject(t[i], INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(t[i]));
  }
#else
  pthread_t t[8];
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_create(t + i, NULL, worker, NULL));
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_join(t[i], NULL));
#endif
}

static void inner_stop(uint32_t pc) {
  assert(pc == 0xf2000000);
  gamepkg_x86_dispatch_stop(0x789abcde);
}
static void outer_hook(uint32_t pc) {
  uint32_t a, b, saved = g_ebp;
  assert(!gamepkg_x86_dispatch_site(&a, &b) && a == pc && b == saved);
  GamePkgX86DispatchEntry inner = {0xf2000000, inner_stop};
  GamePkgX86DispatchExit out;
  assert(!gamepkg_x86_dispatch_run(&inner, 1, inner.address, &out, NULL) &&
         out.pc == inner.address && g_ebp == 0x789abcde);
  assert(!gamepkg_x86_dispatch_site(&a, &b) && a == pc);
  gamepkg_x86_dispatch_stop(saved);
}
static void nested(void) {
  reset();
  GamePkgX86DispatchEntry outer = {0xf1000000, outer_hook};
  GamePkgX86DispatchExit out;
  assert(!gamepkg_x86_dispatch_run(&outer, 1, outer.address, &out, NULL) &&
         out.pc == outer.address && g_ebp == 0xabcdef12);
}
#ifdef GAMEPKG_DISPATCH_NATIVE_ORACLE
#if !defined(__i386__) || defined(_WIN32)
#error This independent CPU probe requires native Linux i386
#endif
#include <sys/mman.h>
static volatile uint32_t native_entry, native_before, native_after,
    native_flags = 0x202;
static void native_case(uint32_t pc, uint32_t eax, uint32_t ecx,
                        int stack_target, uint32_t expected) {
  uint32_t edx = 0, ebx = 0, esi = 0, edi = 0;
  native_entry = pc;
  if (stack_target) {
    __asm__ volatile("pushl native_flags;popfl;movl %%esp,native_before;pushl "
                     "$1f;pushl $0x11210;jmp "
                     "*native_entry;1:movl %%esp,native_after"
                     : "+a"(eax), "+c"(ecx), "+d"(edx), "+b"(ebx), "+S"(esi),
                       "+D"(edi)::"cc", "memory");
  } else {
    __asm__ volatile(
        "pushl native_flags;popfl;movl %%esp,native_before;pushl $1f;jmp "
        "*native_entry;1:movl %%esp,native_after"
        : "+a"(eax), "+c"(ecx), "+d"(edx), "+b"(ebx), "+S"(esi),
          "+D"(edi)::"cc", "memory");
  }
  if (eax != expected || native_before != native_after) {
    fprintf(stderr,
            "native entry %08x result %08x expected %08x stack before %08x "
            "after %08x\n",
            pc, eax, expected, native_before, native_after);
    abort();
  }
  if (pc == 0x10000)
    assert(ecx == 0);
}
static void native_oracle(void) {
  unsigned char *code =
      mmap((void *)0x10000, 8192, PROT_READ | PROT_WRITE,
           MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0);
  assert(code == (void *)0x10000);
  memset(code, 0xcc, 8192);
  /* CPU_BYTES */
  assert(!mprotect(code, 8192, PROT_READ | PROT_EXEC));
  native_case(0x11000, 0, 0, 0, 13);
  native_case(0x11400, 0, 0, 0, 2);
  native_case(0x11300, 0x11310, 0, 0, 0x1234);
  native_case(0x11810, 0x11310, 0, 0, 0x1234);
  native_case(0x11200, 0, 0, 1, 0x11203);
  native_case(0x10000, 0, 120001, 0, 0);
  native_case(0x11a00, 0, 0, 0, 0x2ab8);
  native_case(0x11a01, 0, 0, 0, 0xc300002a);
  native_flags = 0x202;
  native_case(0x11b00, 0, 0, 0, 0x2ab8);
  native_flags = 0x242;
  native_case(0x11b00, 0, 0, 0, 0xc300002a);
  native_flags = 0x202;
  assert(!munmap(code, 8192));
  puts("Native i386 instruction oracle:direct/indirect/ESP-source "
       "calls,changed return target,tail jump,overlapping instruction streams "
       "and120000 call depth match "
       "register/stack results");
}
#endif
/* CONTROLLED_CASES */
int main(void) {
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
  uint32_t endian_probe = 1;
  assert(sizeof(void *) == 4 && *(unsigned char *)&endian_probe == 0);
#endif
  init();
  controlled_cases();
  reset();
  GamePkgX86Memory *critical_memory = bind_ram(1048576, 3);
  GamePkgX86DispatchExit critical_exit;
  uint32_t critical_sp = g_esp;
  g_eax = 0x76543210;
  assert(!gamepkg_x86_dispatch_run(map, count, 0x11d00, &critical_exit, NULL));
  assert(critical_calls == 5 && g_esp == critical_sp + 4 && g_esi == 1 &&
         g_eax == 0x76543210 && g_ebp == 0xabcdef12);
  uint32_t critical_words[4];
  gamepkg_x86_memory_read(0x100050, critical_words, 16, 0, 0);
  assert(critical_words[0] == UINT32_MAX && !critical_words[1] &&
         !critical_words[2]);
  unbind_ram(critical_memory);
  normal();
  overlapping_entries();
  faults();
  fp_retry();
  breakpoint_retry();
  validation();
  workers();
  nested();
  frozen_bindings();
#ifdef GAMEPKG_DISPATCH_NATIVE_ORACLE
  native_oracle();
#endif
  puts("Dispatch:120000 guest call depth with bounded native stack,changed "
       "return PCs,indirect source ordering,RET/stack faults and "
       "repair,interior FP/core retry,256 worker runs passed");
  return 0;
}
