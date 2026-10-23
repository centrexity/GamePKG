/* Asset-free Clang/GCC MinGW TLS and fault-frame ABI qualification. */
#ifndef _WIN32
#error Compile for Windows
#endif
#include "gamepkg_x86_fault.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <windows.h>
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static _Thread_local uint32_t thread_tag;
static void entry(void) {
  gamepkg_x86_page_fault(0x8000 + thread_tag, 6, 0x10000 + thread_tag,
                         0x3000 + thread_tag);
}
static DWORD WINAPI worker(void *raw) {
  uint32_t tag = (uint32_t)(uintptr_t)raw;
  thread_tag = tag;
  assert(gamepkg_x86_get_mxcsr() == 0x1f80);
  uint32_t csr = 0x1f80 | ((tag & 3) << 13);
  gamepkg_x86_set_mxcsr(csr, 0, 0);
  for (unsigned n = 0; n < 1000; ++n) {
    g_eax = (tag << 16) | n;
    g_ecx = g_eax ^ 0x11111111;
    g_edx = g_eax ^ 0x22222222;
    g_ebx = g_eax ^ 0x33333333;
    g_esp = 0x2000 + n;
    g_ebp = 0x777;
    g_esi = g_eax ^ 0x44444444;
    g_edi = g_eax ^ 0x55555555;
    g_eflags = 0x8d7;
    g_fs_base = 0xf0000000 | tag;
    g_df = (int)(tag & 1);
    GamePkgX86Fault f;
    assert(gamepkg_x86_run(entry, &f) == 1 && f.vector == 14 &&
           f.eip == 0x10000 + tag && f.fault_address == 0x8000 + tag &&
           f.error_code == 6 && f.eax == g_eax && f.ecx == g_ecx &&
           f.edx == g_edx && f.ebx == g_ebx && f.esp == g_esp &&
           f.ebp == 0x3000 + tag && g_ebp == f.ebp && f.esi == g_esi &&
           f.edi == g_edi && f.eflags == g_eflags && f.fs_base == g_fs_base &&
           f.direction == (uint32_t)g_df && f.mxcsr == csr &&
           gamepkg_x86_get_mxcsr() == csr);
  }
  return 0;
}
int main(void) {
  g_eax = 0xabcdef01;
  HANDLE threads[8];
  for (unsigned i = 0; i < 8; ++i) {
    threads[i] = CreateThread(0, 0, worker, (void *)(uintptr_t)i, 0, 0);
    assert(threads[i]);
  }
  assert(WaitForMultipleObjects(8, threads, 1, INFINITE) == WAIT_OBJECT_0);
  for (unsigned i = 0; i < 8; ++i)
    CloseHandle(threads[i]);
  assert(g_eax == 0xabcdef01 && gamepkg_x86_get_mxcsr() == 0x1f80);
  puts("Windows Clang/GCC TLS:8000 cross-compiler threaded page-fault contexts "
       "and parent isolation passed");
  return 0;
}
