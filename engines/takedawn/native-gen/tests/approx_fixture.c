#include "approx/gamepkg_approx.h"
#include "gamepkg_x86_approx.h"
#include "gamepkg_x86_fault.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#ifdef GAMEPKG_APPROX_ORACLE
#include <gmp.h>
extern uint32_t original_approx(uint32_t, uint32_t);
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base, g_mxcsr;
_Thread_local int g_df;
_Thread_local RecompXmm g_xmm0, g_xmm1;
static _Thread_local unsigned completed, reads, fail_read;
static _Thread_local unsigned char memory[1024];
uint32_t xbox_MmioLoad(uint32_t a, int bytes) {
  assert(bytes == 4 && a <= 1020);
  if (++reads == fail_read)
    gamepkg_x86_page_fault(a, 0, 0x10000, g_ebp);
  uint32_t v = 0;
  for (unsigned i = 0; i < 4; ++i)
    v |= (uint32_t)memory[a + i] << (8 * i);
  return v;
}
void xbox_MmioStore(uint32_t a, uint32_t v, int b) {
  (void)a;
  (void)v;
  (void)b;
  abort();
}
/* WRAPPERS */
static const uint32_t special[] = {
    0,          0x80000000, 1,          0x80000001, 0x7fffff,   0x807fffff,
    0x800000,   0x80800000, 0x3f800000, 0xbf800000, 0x40000000, 0xc0000000,
    0x7f800000, 0xff800000, 0x7fc00001, 0xffc00001, 0x7f800001, 0xff800001,
    0x7fffffff, 0xffffffff, 0x7e800000, 0x7f7fffff};
static uint32_t host_status(void) {
#if defined(__i386__) || defined(__x86_64__)
  uint32_t v;
  __asm__ volatile("stmxcsr %0" : "=m"(v));
  return v;
#else
  return 0;
#endif
}
static void cases(void) {
  for (unsigned f = 0; f < sizeof(forms) / sizeof(forms[0]); ++f)
    for (unsigned v = 0; v < sizeof(special) / sizeof(special[0]); ++v) {
      unsigned op = forms[f][0], lanes = forms[f][1], mode = forms[f][2];
      uint32_t input[4], old[4];
      for (unsigned i = 0; i < 4; ++i) {
        input[i] = special[(v + i) % 22];
        old[i] = 0xface0000 + i;
        g_xmm0.u[i] = old[i];
        g_xmm1.u[i] = input[i];
      }
      if (mode == 4) {
        memcpy(g_xmm0.u, input, 16);
        memcpy(old, input, 16);
      }
      g_eax = mode == 3 ? 0x12340100 : 0x100;
      g_fs_base = mode == 2 ? 0x100 : 0;
      g_ebx = mode == 3 ? 0xabcd0040 : 0x11111111;
      g_esi = mode == 3 ? 0xdef000c0 : 0x22222222;
      g_ebp = 0xabcdef12;
      g_eflags = 0x00240ed7;
      g_df = 1;
      g_mxcsr = 0x9fbf;
      uint32_t addr = mode == 2 ? 0x200 : 0x100;
      for (unsigned i = 0; i < 16; ++i)
        memory[addr + i] = (unsigned char)(input[i / 4] >> (8 * (i % 4)));
      completed = reads = fail_read = 0;
      GamePkgX86Fault fault;
      uint32_t host = host_status();
      assert(!gamepkg_x86_run(instructions[f], &fault) && completed == 1 &&
             host_status() == host);
      for (unsigned i = 0; i < 4; ++i) {
        assert(g_xmm0.u[i] ==
               (i < lanes ? gamepkg_approx_f32(input[i], op) : old[i]));
        assert(g_xmm1.u[i] == input[i]);
      }
      assert(g_mxcsr == 0x9fbf && g_eflags == 0x00240ed7 && g_df == 1);
      if (mode == 0 || mode == 4)
        continue;
      for (unsigned fail = 1; fail <= lanes; ++fail) {
        memcpy(g_xmm0.u, old, 16);
        completed = reads = 0;
        fail_read = fail;
        assert(gamepkg_x86_run(instructions[f], &fault) == 1 && !completed);
        assert(fault.vector == 14 && fault.eip == 0x10000 &&
               fault.ebp == 0xabcdef12);
        assert(!memcmp(g_xmm0.u, old, 16) && g_mxcsr == 0x9fbf &&
               g_eflags == 0x00240ed7);
      }
      if (lanes == 4 && mode != 3) {
        g_eax = 0x101;
        memcpy(g_xmm0.u, old, 16);
        completed = reads = fail_read = 0;
        assert(gamepkg_x86_run(instructions[f], &fault) == 1 &&
               fault.vector == 13 && !reads && !completed);
        assert(!memcmp(g_xmm0.u, old, 16));
      }
    }
  for (uint32_t control = 0; control <= 65535; ++control) {
    if (control & 64)
      continue;
    g_mxcsr = control;
    for (unsigned kind = 0; kind < 2; ++kind)
      for (unsigned v = 0; v < 22; ++v) {
        uint32_t a[8] = {special[v],
                         special[(v + 1) % 22],
                         special[(v + 2) % 22],
                         special[(v + 3) % 22],
                         0,
                         0,
                         0,
                         0},
                 expected[4];
        for (unsigned i = 0; i < 4; ++i)
          expected[i] = gamepkg_approx_f32(a[i], kind);
        gamepkg_x86_approx_f32(a, a + 1, 4, kind);
        assert(!memcmp(a + 1, expected, 16) && g_mxcsr == control);
      }
  }
}
#ifdef _WIN32
static DWORD WINAPI worker(void *arg) {
#else
static void *worker(void *arg) {
#endif
  (void)arg;
  for (unsigned n = 0; n < 64; ++n) {
    uint32_t source[4] = {0x3f800000 + n, 0x40000000 + n, 1, 0xff800001},
             result[4];
    g_mxcsr = n;
    gamepkg_x86_approx_f32(source, result, 4, 1);
    assert(g_mxcsr == n);
    for (unsigned i = 0; i < 4; ++i)
      assert(result[i] == gamepkg_approx_f32(source[i], 1));
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
static uint32_t word(FILE *f) {
  unsigned char b[4];
  assert(fread(b, 1, 4, f) == 4);
  return (uint32_t)b[0] | (uint32_t)b[1] << 8 | (uint32_t)b[2] << 16 |
         (uint32_t)b[3] << 24;
}
static void replay(const char *p) {
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
  uint32_t probe = 1;
  assert(sizeof(void *) == 4 && *(unsigned char *)&probe == 0);
#endif
  FILE *f = fopen(p, "rb");
  assert(f);
  char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && !memcmp(magic, "GPKAPPv1", 8));
  uint32_t count = word(f);
  assert(count == 4681772);
  for (uint32_t i = 0; i < count; ++i) {
    uint32_t op = word(f), input = word(f), output = word(f);
    assert(op < 2 && gamepkg_approx_f32(input, op) == output);
  }
  assert(fgetc(f) == EOF && !ferror(f) && !fclose(f));
  printf("Approximation reference replay: %u exact words; pointer_bits=%u\n",
         count, (unsigned)(sizeof(void *) * 8));
}
#ifdef GAMEPKG_APPROX_ORACLE
static FILE *corpus;
static void out_word(uint32_t v) {
  if (corpus) {
    unsigned char b[4];
    for (unsigned i = 0; i < 4; ++i)
      b[i] = (unsigned char)(v >> (8 * i));
    assert(fwrite(b, 1, 4, corpus) == 4);
  }
}
static void accuracy(void) {
  mpz_t n, low, high;
  mpz_inits(n, low, high, NULL);
  unsigned proofs = 0;
  for (unsigned op = 0; op < 2; ++op)
    for (unsigned parity = 0; parity < (op ? 2u : 1u); ++parity) {
      unsigned shift = op ? 13 : 12, buckets = 1u << (23 - shift),
               exp = 127 + parity;
      for (unsigned i = 0; i < buckets; ++i)
        for (unsigned end = 0; end < 2; ++end) {
          uint32_t input = (exp << 23) | (i << shift) |
                           (end ? ((1u << shift) - 1) : 0),
                   out = gamepkg_approx_f32(input, op),
                   xsig = (input & 0x7fffff) | 0x800000,
                   ysig = (out & 0x7fffff) | 0x800000;
          mpz_set_ui(n, ysig);
          if (op)
            mpz_mul_ui(n, n, ysig);
          mpz_mul_ui(n, n, xsig);
          if (op) {
            mpz_set_ui(low, 8189u * 8189u);
            mpz_set_ui(high, 8195u * 8195u);
            mpz_mul_2exp(low, low, 45 - parity);
            mpz_mul_2exp(high, high, 45 - parity);
          } else {
            mpz_mul_2exp(n, n, 13);
            mpz_set_ui(low, 8189);
            mpz_set_ui(high, 8195);
            mpz_mul_2exp(low, low, 47);
            mpz_mul_2exp(high, high, 47);
          }
          assert(mpz_cmp(n, low) >= 0 && mpz_cmp(n, high) <= 0);
          ++proofs;
        }
    }
  mpz_clears(n, low, high, NULL);
  assert(proofs == 8192);
  puts("Approximation accuracy:8192 exact integer endpoint certificates bound "
       "every normal mantissa bucket by3/8192, excluding specified RCP "
       "underflow-to-zero");
}

#if defined(__i386__) || defined(__x86_64__)
static void hardware_observations(void) {
  uint64_t observations = 0, differences = 0;
  uint32_t before = host_status();
  for (uint32_t control = 0; control <= 65535; ++control) {
    if (control & 64)
      continue;
    for (unsigned op = 0; op < 2; ++op)
      for (unsigned lanes = 1; lanes <= 4; lanes += 3)
        for (unsigned v = 0; v < 22; ++v) {
          typedef struct {
            _Alignas(16) uint32_t u[4];
          } Bits;
          Bits src, dst = {{0xdeadbeef, 0xface1234, 0x87654321, 0x11223344}},
                    result;
          uint32_t after;
          for (unsigned i = 0; i < 4; ++i)
            src.u[i] = special[(v + i) % 22];
#define HW(code)                                                               \
  __asm__ volatile("ldmxcsr %[control];movups %[dst],%%xmm0;" code             \
                   " %[src],%%xmm0;movups %%xmm0,%[result];stmxcsr "           \
                   "%[after];ldmxcsr %[before]"                                \
                   : [result] "=m"(result), [after] "=m"(after)                \
                   : [control] "m"(control), [dst] "m"(dst), [src] "m"(src),   \
                     [before] "m"(before)                                      \
                   : "xmm0", "memory")
          if (op == 0) {
            if (lanes == 1) {
              HW("rcpss");
            } else {
              HW("rcpps");
            }
          } else {
            if (lanes == 1) {
              HW("rsqrtss");
            } else {
              HW("rsqrtps");
            }
          }
#undef HW
          assert(after == control && host_status() == before);
          for (unsigned i = 0; i < 4; ++i) {
            if (i >= lanes) {
              assert(result.u[i] == dst.u[i]);
              continue;
            }
            uint32_t expected = original_approx(src.u[i], op),
                     exp = (src.u[i] >> 23) & 255;
            if (!exp || exp == 255 || (op && (src.u[i] >> 31)) ||
                (!op && exp >= 253)) {
              assert(result.u[i] == expected);
            } else {
              differences += result.u[i] != expected;
            }
            ++observations;
          }
        }
  }
  printf("Native approximation observations:%llu words,unchanged full MXCSR "
         "and exact specials/upper lanes; %llu finite words differ from fixed "
         "profile (recorded vendor variation)\n",
         (unsigned long long)observations, (unsigned long long)differences);
}
#endif
static void oracle(void) {
  const char *path = getenv("GAMEPKG_TAKEDAWN_APPROX_CORPUS");
  if (path) {
    corpus = fopen(path, "wb");
    assert(corpus);
    assert(fwrite("GPKAPPv1", 1, 8, corpus) == 8);
    out_word(4681772);
  }
  uint32_t count = 0;
  for (unsigned op = 0; op < 2; ++op) {
    unsigned shift = op ? 13 : 12, buckets = 1u << (23 - shift);
    for (unsigned exp = 1; exp < 255; ++exp)
      for (unsigned i = 0; i < buckets; ++i)
        for (unsigned edge = 0; edge < 3; ++edge)
          for (unsigned sign = 0; sign < 2; ++sign) {
            uint32_t input = (sign << 31) | (exp << 23) | (i << shift) |
                             (edge == 0   ? 0
                              : edge == 1 ? (1u << (shift - 1))
                                          : ((1u << shift) - 1));
            uint32_t expected = original_approx(input, op);
            assert(gamepkg_approx_f32(input, op) == expected);
            out_word(op);
            out_word(input);
            out_word(expected);
            ++count;
          }
  }
  assert(count == 4681728);
  for (unsigned op = 0; op < 2; ++op)
    for (unsigned i = 0; i < 22; ++i) {
      uint32_t expected = original_approx(special[i], op);
      assert(gamepkg_approx_f32(special[i], op) == expected);
      out_word(op);
      out_word(special[i]);
      out_word(expected);
    }
  if (corpus) {
    assert(!fclose(corpus));
    corpus = NULL;
  }
  printf("Approximation original profile: %u normal boundary/scale/sign cases "
         "and44 special cases match\n",
         count);
  accuracy();
#if defined(__i386__) || defined(__x86_64__)
  hardware_observations();
#endif
}
#endif
int main(int argc, char **argv) {
  if (argc == 2) {
    replay(argv[1]);
    return 0;
  }
  assert(argc == 1);
#ifdef GAMEPKG_APPROX_ORACLE
  oracle();
#endif
  cases();
  workers();
  puts("Approximation:20 emitted forms,scalar/packed/alias/read/alignment "
       "faults,all32768 valid MXCSR words and512 worker iterations passed");
  return 0;
}
