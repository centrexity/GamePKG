#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_flags.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
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
static _Thread_local unsigned char memory[64];
static _Thread_local unsigned reads, writes, completed, fail_read, fail_write;
static _Thread_local uint32_t address, write_value;
uint32_t xbox_MmioLoad(uint32_t a, int width) {
  ++reads;
  address = a;
  if (fail_read)
    gamepkg_x86_page_fault(a, 0, 0x10000, 0xabcdef12);
  uint32_t v = 0;
  for (int i = 0; i < width; ++i)
    v |= (uint32_t)memory[(a + (unsigned)i) & 63] << (8 * (unsigned)i);
  return v;
}
void xbox_MmioStore(uint32_t a, uint32_t v, int width) {
  ++writes;
  address = a;
  write_value = v;
  if (fail_write)
    gamepkg_x86_page_fault(a, 2, 0x10000, 0xabcdef12);
  for (int i = 0; i < width; ++i)
    memory[(a + (unsigned)i) & 63] = (unsigned char)(v >> (8 * (unsigned)i));
}
/* WRAPPERS */
static uint32_t word_reference(uint32_t f, unsigned direction) {
  static const unsigned positions[] = {0,  2,  4,  6,  7,  8,  9,  11, 12,
                                       13, 14, 16, 17, 18, 19, 20, 21};
  uint32_t result = 2 | (direction ? 1u << 10 : 0);
  for (unsigned i = 0; i < sizeof(positions) / sizeof(*positions); ++i)
    result |= f & (1u << positions[i]);
  return result;
}
static uint32_t pop_reference(uint32_t old, unsigned direction, uint32_t input,
                              unsigned bytes) {
  uint32_t result = word_reference(old, direction);
  static const unsigned writable[] = {0,  2,  4,  6,  7,  8,  9,
                                      10, 11, 12, 13, 14, 18, 21};
  for (unsigned i = 0; i < sizeof(writable) / sizeof(*writable); ++i) {
    unsigned bit = writable[i];
    if (bytes == 4 || bit < 16)
      result = (result & ~(1u << bit)) | (input & (1u << bit));
  }
  return result & ~(1u << 16);
}
static void check(unsigned op, uint32_t initial, unsigned direction,
                  uint32_t eax, uint32_t input, unsigned fail) {
  unsigned kind = forms[op][0], bytes = forms[op][1];
  uint32_t sp = 0x1234ffff;
  for (unsigned i = 0; i < sizeof(memory); ++i)
    memory[i] = (unsigned char)(i * 31 + 19);
  for (unsigned i = 0; i < 4; ++i)
    memory[(sp + i) & 63] = (unsigned char)(input >> (8 * i));
  unsigned char before[64];
  memcpy(before, memory, sizeof(before));
  g_eax = eax;
  g_ecx = 0x22223333;
  g_edx = 0x44445555;
  g_ebx = 0x66667777;
  g_esp = sp;
  g_ebp = 0xabcdef12;
  g_esi = 0x88889999;
  g_edi = 0xaaaabbbb;
  g_fs_base = 0xff000000;
  g_df = (int)direction;
  g_eflags = initial;
  reads = writes = completed = fail_read = fail_write = 0;
  if (fail == 1)
    fail_read = 1;
  else if (fail == 2)
    fail_write = 1;
  GamePkgX86Fault fault;
  int error = gamepkg_x86_run(instructions[op], &fault);
  unsigned rejected = (kind == 7 && fail == 2) || (kind == 8 && fail == 1);
  assert(error == (int)rejected && g_ecx == 0x22223333 && g_edx == 0x44445555 &&
         g_ebx == 0x66667777 && g_esi == 0x88889999 && g_edi == 0xaaaabbbb &&
         g_ebp == 0xabcdef12 && gamepkg_x86_get_mxcsr() == 0x1f80);
  if (rejected) {
    assert(g_eax == eax && g_esp == sp && g_eflags == initial &&
           g_df == (int)direction && !completed &&
           !memcmp(memory, before, sizeof(memory)));
    assert(fault.vector == 14 && fault.eip == 0x10000 && fault.esp == sp &&
           fault.eflags == initial && fault.direction == direction &&
           fault.fault_address == address &&
           fault.error_code == (fail == 2 ? 2u : 0u));
    return;
  }
  uint32_t expected = initial, expected_eax = eax;
  unsigned df = direction;
  switch (kind) {
  case 0:
    expected_eax = (eax & ~0xff00u) | (((initial & 0xd5u) | 2) << 8);
    break;
  case 1:
    expected = (initial & ~255u) | ((eax >> 8) & 0xd5u) | 2;
    break;
  case 2:
    expected = (initial & ~1u) | 2;
    break;
  case 3:
    expected = initial | 3;
    break;
  case 4:
    expected = (initial ^ 1u) | 2;
    break;
  case 5:
    expected = (initial & ~(1u << 10)) | 2;
    df = 0;
    break;
  case 6:
    expected = initial | (1u << 10) | 2;
    df = 1;
    break;
  case 7: {
    uint32_t image =
        word_reference(initial, direction) & ~((1u << 16) | (1u << 17));
    assert(g_esp == sp - bytes && address == sp - bytes &&
           write_value == (bytes == 2 ? image & 65535 : image));
    break;
  }
  default:
    expected = pop_reference(initial, direction, input, bytes);
    df = (expected >> 10) & 1;
    assert(g_esp == sp + bytes && address == sp);
    break;
  }
  assert(g_eflags == expected && g_df == (int)df && g_eax == expected_eax &&
         completed == 1 && reads == (kind == 8) && writes == (kind == 7));
  if (kind < 7)
    assert(g_esp == sp && !memcmp(memory, before, sizeof(memory)));
}
static void all(unsigned seed) {
  for (unsigned op = 0; op < sizeof(forms) / sizeof(*forms); ++op)
    for (unsigned i = 0; i < 256; ++i) {
      uint32_t f = (i * 0x10204081u) ^ seed;
      uint32_t eax = (0x12ab0034u | (i << 8)) ^ seed,
               input = (0x89abcdefu * (i + 1)) ^ seed;
      check(op, f, i & 1, eax, input, 0);
      if (forms[op][0] >= 7) {
        check(op, f, i & 1, eax, input, 1);
        check(op, f, i & 1, eax, input, 2);
      }
    }
}
#ifdef _WIN32
static DWORD WINAPI worker(void *arg) {
  for (unsigned i = 0; i < 100; ++i)
    all((unsigned)(uintptr_t)arg * 127 + i);
  return 0;
}
#else
static void *worker(void *arg) {
  for (unsigned i = 0; i < 100; ++i)
    all((unsigned)(uintptr_t)arg * 127 + i);
  return NULL;
}
#endif
#if defined(__i386__) || defined(__x86_64__)
#ifdef __x86_64__
#define HOST_PUSH "pushfq"
#define HOST_POP "popfq"
#else
#define HOST_PUSH "pushfl"
#define HOST_POP "popfl"
#endif
static void native_checks(void) {
  unsigned checks = 0;
  const unsigned positions[] = {0, 2, 4, 6, 7};
  for (unsigned kind = 0; kind < 7; ++kind)
    for (unsigned bits = 0; bits < 32; ++bits)
      for (unsigned upper = 0; upper < 4; ++upper)
        for (unsigned ah = 0; ah < 256; ++ah) {
          uint32_t flags = 2 | ((upper & 1) << 11) | ((upper >> 1) << 10);
          for (unsigned i = 0; i < 5; ++i)
            flags |= ((bits >> i) & 1) << positions[i];
          uint32_t input = 0x12340078 | (ah << 8), output = 0;
          unsigned long host, seen = 0;
          __asm__ volatile(HOST_PUSH ";pop %0" : "=r"(host)::"memory");
          unsigned long control = (host & ~0xcd5ul) | (flags & 0xcd5u);
#define OBSERVE(op)                                                            \
  __asm__ volatile(                                                            \
      "push %[control];" HOST_POP ";mov %[input],%%eax;" op ";" HOST_PUSH      \
      ";pop %[seen];mov %%eax,%[output];push %[host];" HOST_POP ";cld"         \
      : [seen] "=m"(seen), [output] "=m"(output)                               \
      : [control] "r"(control), [host] "r"(host), [input] "m"(input)           \
      : "eax", "cc", "memory")
          switch (kind) {
          case 0:
            OBSERVE("lahf");
            break;
          case 1:
            OBSERVE("sahf");
            break;
          case 2:
            OBSERVE("clc");
            break;
          case 3:
            OBSERVE("stc");
            break;
          case 4:
            OBSERVE("cmc");
            break;
          case 5:
            OBSERVE("cld");
            break;
          default:
            OBSERVE("std");
            break;
          }
#undef OBSERVE
          g_eax = input;
          g_eflags = flags;
          g_df = (int)((flags >> 10) & 1);
          GamePkgX86Fault fault;
          assert(!gamepkg_x86_run(instructions[kind * 3], &fault));
          assert(g_eax == output && (g_eflags & 0xcd5u) == (seen & 0xcd5u) &&
                 g_df == (int)((seen >> 10) & 1));
          ++checks;
        }
  printf("Native status/carry/direction hardware: %u exact observations\n",
         checks);
}
#else
static void native_checks(void) {}
#endif
int main(void) {
  native_checks();
  all(0);
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
  puts("Flag emission: status/carry/direction, word/dword stack images, "
       "rejected memory transactions and 800 worker iterations passed");
  return 0;
}
