#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_status.h"
#include "status/reference/inputs.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
typedef ULONG(WINAPI *NativeMap)(LONG);
static NativeMap native_map, native_no_teb;
#endif
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big-endian compiler required
#endif
_Static_assert(sizeof(void *) == 4, "big-endian32 required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
extern uint32_t gamepkg_status_reference(uint32_t);
static FILE *output;
static uint64_t count, digest = UINT64_C(14695981039346656037);
static void put32(unsigned char *p, uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(v >> (i * 8));
}
static uint32_t get32(const unsigned char *p) {
  return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) |
         ((uint32_t)p[3] << 24);
}
static void check(uint32_t status, uint32_t expected) {
  uint32_t value = gamepkg_x86_status_to_dos(status);
  if (value != expected) {
    fprintf(stderr, "status %08x expected %08x got %08x\n", status, expected,
            value);
    abort();
  }
#ifdef _WIN32
  if (native_map) {
    LONG raw;
    memcpy(&raw, &status, 4);
    SetLastError(0x5a6b7c8d);
    uint32_t native = native_map(raw), other = native_no_teb(raw);
    assert(GetLastError() == 0x5a6b7c8d);
    if (native != value || other != value) {
      fprintf(stderr,
              "native status %08x runtime %08x native %08x no_teb %08x\n",
              status, value, native, other);
      abort();
    }
  }
#endif
  unsigned char pair[8];
  put32(pair, status);
  put32(pair + 4, expected);
  for (unsigned i = 0; i < 8; ++i) {
    digest ^= pair[i];
    digest *= UINT64_C(1099511628211);
  }
  if (output)
    assert(fwrite(pair, 1, 8, output) == 8);
  ++count;
}
static void sample(uint32_t value) {
  check(value, gamepkg_status_reference(value));
}
static void exercise(void) {
  const uint32_t edge[] = {0,          0xffffffff, 0x20000000, 0x80000000,
                           0xc0000000, 0xd0000000, 0x10000000, 0x40000000,
                           0xe0000000, 0x7fffffff, 0x80010000, 0xd001ffff};
  for (size_t i = 0; i < sizeof(edge) / sizeof(edge[0]); ++i)
    sample(edge[i]);
  for (unsigned i = 0; i < 1586; ++i) {
    uint32_t v = gamepkg_status_inputs[i];
    sample(v);
    sample(v | 0x10000000u);
    sample(v | 0x20000000u);
    sample(v ^ 0x80000000u);
  }
  for (unsigned i = 0; i < 172; ++i)
    sample(gamepkg_status_boundaries[i]);
  const uint32_t facilities[] = {0x80070000, 0xc0010000, 0xc0070000,
                                 0xd0010000, 0xd0070000, 0xa0070000,
                                 0xe0010000, 0xe0070000};
  for (unsigned f = 0; f < 8; ++f)
    for (uint32_t low = 0; low < 65536; ++low)
      sample(facilities[f] | low);
  uint32_t random = 0x6d25a3b1;
  for (unsigned i = 0; i < 200000; ++i) {
    random ^= random << 13;
    random ^= random >> 17;
    random ^= random << 5;
    sample(random);
  }
}
static void replay(void) {
  const char *path = getenv("GAMEPKG_STATUS_REPLAY");
  assert(path);
  FILE *input = fopen(path, "rb");
  assert(input);
  unsigned char header[24];
  assert(fread(header, 1, 24, input) == 24 &&
         !memcmp(header, "GPKNTD01Wine9.0\0", 16));
  uint64_t records =
      (uint64_t)get32(header + 16) | ((uint64_t)get32(header + 20) << 32);
  assert(records <= (UINT64_MAX - 24) / 8);
  for (uint64_t i = 0; i < records; ++i) {
    unsigned char pair[8];
    assert(fread(pair, 1, 8, input) == 8);
    uint32_t status = get32(pair), expected = get32(pair + 4);
    assert(gamepkg_status_reference(status) == expected);
    check(status, expected);
  }
  assert(fgetc(input) == EOF && !ferror(input) && !fclose(input));
}
int main(void) {
#ifdef _WIN32
  if (getenv("GAMEPKG_STATUS_NATIVE")) {
    HMODULE module = GetModuleHandleA("ntdll.dll");
    assert(module);
    FARPROC map = GetProcAddress(module, "RtlNtStatusToDosError"),
            no_teb = GetProcAddress(module, "RtlNtStatusToDosErrorNoTeb");
    assert(map && no_teb);
    _Static_assert(sizeof(map) == sizeof(native_map),
                   "native function pointer ABI");
    memcpy(&native_map, &map, sizeof(map));
    memcpy(&native_no_teb, &no_teb, sizeof(no_teb));
  }
#endif
  const char *path = getenv("GAMEPKG_STATUS_CORPUS");
  if (path) {
    output = fopen(path, "wb");
    assert(output);
    unsigned char header[24] = {0};
    memcpy(header, "GPKNTD01Wine9.0\0", 16);
    assert(fwrite(header, 1, 24, output) == 24);
  }
  memset(&fp, 0xa5, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(xmm, 0x5a, sizeof(xmm));
  gamepkg_x86_x87_bind_segments(8, 16, 16, 24);
  g_eax = 0x12345678;
  g_ecx = 0x23456789;
  g_edx = 0x3456789a;
  g_ebx = 0x456789ab;
  g_esp = 0x56789abc;
  g_ebp = 0x6789abcd;
  g_esi = 0x789abcde;
  g_edi = 0x89abcdef;
  g_eflags = 0x202;
  GamePkgX86FpFault before, after, fault;
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &before, &source));
  assert(!gamepkg_x86_run_with_fp(
      getenv("GAMEPKG_STATUS_REPLAY") ? replay : exercise, &fault, &source));
  assert(!gamepkg_x86_capture_registers(0, g_ebp, &after, &source) &&
         !memcmp(&before, &after, sizeof(before)));
  if (output) {
    unsigned char words[8];
    put32(words, (uint32_t)count);
    put32(words + 4, (uint32_t)(count >> 32));
    assert(!fseek(output, 16, SEEK_SET) && fwrite(words, 1, 8, output) == 8 &&
           !fclose(output));
  }
  printf("NTSTATUS: %llu cases, digest=%016llx, pending x87-MMX-XMM/GPR "
         "preserved; pointer_bits=%u\n",
         (unsigned long long)count, (unsigned long long)digest,
         (unsigned)(sizeof(void *) * 8));
  return 0;
}
