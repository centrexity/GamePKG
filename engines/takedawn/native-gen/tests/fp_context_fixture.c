#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
_Static_assert(sizeof(GamePkgX86Fault) == 64, "preserve core fault ABI");
static _Thread_local GamePkgX86FpSource descriptor;
static _Thread_local unsigned seed, entered;
static uint32_t *bank(unsigned i) {
  uint32_t *banks[] = {g_xmm0.u, g_xmm1.u, g_xmm2.u, g_xmm3.u,
                       g_xmm4.u, g_xmm5.u, g_xmm6.u, g_xmm7.u};
  return banks[i];
}
static void initialize(void) {
  GamePkgX87State *s = gamepkg_x86_x87_state();
  memset(s, 0xa5, sizeof(*s)); /* Padding must not leak into capture. */
  for (unsigned i = 0; i < 8; ++i) {
    s->significand[i] =
        UINT64_C(0x7fff0000abcdef00) ^ ((uint64_t)seed << 32) ^ i;
    s->sign_exponent[i] =
        (uint16_t)(0x7fffu + i); /* Noncanonical, NaN, signed payloads. */
    for (unsigned j = 0; j < 4; ++j)
      bank(i)[j] = 0x7f800001u ^ (seed << 8) ^ (i << 4) ^ j;
  }
  s->control = 0x37f;
  s->status = 0x2800;
  s->tags = 0x1b1b;
  s->instruction_pointer = 0x80001234;
  s->data_pointer = 0xf1234567;
  s->opcode = 0x5ab;
  s->instruction_selector = 0x1b;
  s->data_selector = 0x23;
  g_eax = seed;
  g_ecx = 0;
  g_edx = 77;
  g_ebx = 88;
  g_esp = 0x11110000;
  g_ebp = 0xdeadbeef;
  g_esi = 0x22220000;
  g_edi = 0x33330000;
  g_eflags = 0x202;
  g_fs_base = 0xff000000;
  g_df = 1;
  gamepkg_x86_set_mxcsr(0x1f80, 0, 0);
  descriptor.x87 = s;
  for (unsigned i = 0; i < 8; ++i)
    descriptor.xmm[i] = bank(i);
}
static void verify_fp(const GamePkgX86FpSnapshot *f) {
  const GamePkgX87State *s = gamepkg_x86_x87_state();
  GamePkgX87State expected;
  memset(&expected, 0, sizeof(expected));
  for (unsigned i = 0; i < 8; ++i) {
    expected.significand[i] = s->significand[i];
    expected.sign_exponent[i] = s->sign_exponent[i];
    for (unsigned j = 0; j < 4; ++j)
      assert(f->xmm[i][j] == bank(i)[j]);
  }
  expected.control = s->control;
  expected.status = s->status;
  expected.tags = s->tags;
  expected.instruction_pointer = s->instruction_pointer;
  expected.data_pointer = s->data_pointer;
  expected.opcode = s->opcode;
  expected.instruction_selector = s->instruction_selector;
  expected.data_selector = s->data_selector;
  assert(!memcmp(&f->x87, &expected, sizeof(expected)));
}
static void normal(void) { ++entered; }
static void page(void) {
  gamepkg_x86_page_fault(0xfffffffcu, 6, 0x7777, 0x12345678);
}
static void protection(void) {
  gamepkg_x86_set_mxcsr(0x40, 0x8888, 0x12345678);
}
static void simd(void) { gamepkg_x86_sse_status(1, 0x9999, 0x12345678); }
static void deferred(void) {
  gamepkg_x86_x87_state()->control &= (uint16_t)~1u;
  gamepkg_x86_x87_state()->status |= 0x8081;
  generated_wait();
}
static void descriptor_change(void) {
  memset(&descriptor, 0, sizeof(descriptor));
  bank(7)[3] = 0x80000001;
  generated_divide();
}
static void mmx_fault(void) {
  generated_mmx();
  generated_divide();
}
static void empty_fault(void) {
  generated_mmx();
  generated_empty();
  generated_divide();
}
static void invalid(void);
static void nested(void) {
  invalid(); /* Rejection cannot replace or unlink the live outer boundary. */
  GamePkgX86FpFault inner;
  assert(gamepkg_x86_run_pinned(simd, &inner) == 1);
  assert(inner.core.vector == 19 && inner.core.mxcsr == 1);
  verify_fp(&inner.fp);
  GamePkgX86Fault light;
  assert(gamepkg_x86_run(page, &light) == 1 && light.vector == 14);
  bank(1)[2] = 0xfeed0123;
  generated_divide();
}
static void invalid(void) {
  GamePkgX86FpFault f, old;
  memset(&f, 0xc3, sizeof(f));
  memcpy(&old, &f, sizeof(f));
  GamePkgX86FpSource good = descriptor, bad;
#define BAD(expr)                                                              \
  do {                                                                         \
    assert((expr) == -1);                                                      \
    assert(!memcmp(&old, &f, sizeof(f)));                                      \
  } while (0)
  BAD(gamepkg_x86_run_with_fp(NULL, &f, &good));
  BAD(gamepkg_x86_run_with_fp(normal, &f, NULL));
  BAD(gamepkg_x86_run_with_fp(normal, NULL, &good));
  bad = good;
  bad.x87 = NULL;
  BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
  bad = good;
  bad.x87 = (const GamePkgX87State *)(UINTPTR_MAX & ~(uintptr_t)7);
  BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
  bad = good;
  bad.x87 = (const GamePkgX87State *)((const unsigned char *)good.x87 + 1);
  BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
  for (unsigned i = 0; i < 8; ++i) {
    bad = good;
    bad.xmm[i] = NULL;
    BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
    bad = good;
    bad.xmm[i] = (const uint32_t *)((uintptr_t)good.xmm[i] + 1);
    BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
    bad = good;
    bad.xmm[i] = (const uint32_t *)(UINTPTR_MAX - 3);
    BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
    bad = good;
    bad.xmm[i] = f.fp.xmm[i];
    BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
    bad = good;
    bad.xmm[i] = (const uint32_t *)good.x87;
    BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
    for (unsigned j = 0; j < i; ++j) {
      bad = good;
      bad.xmm[i] = good.xmm[j] + 1;
      BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
    }
  }
  bad = good;
  bad.x87 = &f.fp.x87;
  BAD(gamepkg_x86_run_with_fp(normal, &f, &bad));
  BAD(gamepkg_x86_run_with_fp(normal, &f, (const GamePkgX86FpSource *)&f));
  BAD(gamepkg_x86_run_with_fp(normal, (GamePkgX86FpFault *)((uintptr_t)&f + 1),
                              &good));
  BAD(gamepkg_x86_run_with_fp(
      normal, &f, (const GamePkgX86FpSource *)((uintptr_t)&good + 1)));
  assert(entered == 0);
#undef BAD
}
static void explicit_bindings(void) {
  GamePkgX87State state = *gamepkg_x86_x87_state();
  uint32_t lanes[8][4];
  GamePkgX86FpSource source = {.x87 = &state};
  for (unsigned i = 0; i < 8; ++i) {
    state.significand[i] ^= UINT64_C(0x8000000000000000);
    state.sign_exponent[i] ^= 0x8000;
    source.xmm[i] = lanes[i];
    for (unsigned j = 0; j < 4; ++j)
      lanes[i][j] = bank(i)[j] ^ 0x80000000u;
  }
  GamePkgX86FpFault fault;
  assert(gamepkg_x86_run_with_fp(generated_divide, &fault, &source) == 1);
  assert(fault.core.vector == 0 && fault.core.eax == seed);
  for (unsigned i = 0; i < 8; ++i) {
    assert(fault.fp.x87.significand[i] == state.significand[i]);
    assert(fault.fp.x87.sign_exponent[i] == state.sign_exponent[i]);
    for (unsigned j = 0; j < 4; ++j)
      assert(fault.fp.xmm[i][j] == lanes[i][j]);
  }
  assert(fault.fp.x87.control == state.control);
  assert(fault.fp.x87.status == state.status);
  assert(fault.fp.x87.tags == state.tags);
  assert(fault.fp.x87.instruction_pointer == state.instruction_pointer);
  assert(fault.fp.x87.data_pointer == state.data_pointer);
  assert(fault.fp.x87.opcode == state.opcode);
  assert(fault.fp.x87.instruction_selector == state.instruction_selector);
  assert(fault.fp.x87.data_selector == state.data_selector);
}
/* RESTORE_CASES */
static void cases(void) {
  capture_cases();
  initialize();
  invalid();
  explicit_bindings();
  GamePkgX86FpFault f, zero;
  memset(&zero, 0, sizeof(zero));
  assert(gamepkg_x86_run_pinned(normal, &f) == 0 && entered == 1);
  assert(!memcmp(&f, &zero, sizeof(f)));
  entered = 0;
  void (*entries[])(void) = {
      generated_divide,  protection, page,        deferred, simd,
      descriptor_change, mmx_fault,  empty_fault, nested,   restore_then_fault};
  const unsigned vectors[] = {0, 13, 14, 16, 19, 0, 0, 0, 0, 0};
  for (unsigned k = 0; k < 10; ++k) {
    initialize();
    if (k == 4 || k == 8)
      gamepkg_x86_set_mxcsr(0, 0, 0);
    assert(gamepkg_x86_run_with_fp(entries[k], &f, &descriptor) == 1);
    assert(f.core.vector == vectors[k]);
    verify_fp(&f.fp);
    assert(f.core.eax == seed && f.core.ecx == 0 && f.core.edx == 77 &&
           f.core.ebx == 88);
    assert(f.core.ebp == 0x12345678 && g_ebp == 0x12345678);
    assert(f.core.esp == 0x11110000 && f.core.esi == 0x22220000 &&
           f.core.edi == 0x33330000);
    assert(f.core.eflags == 0x202 && f.core.fs_base == 0xff000000 &&
           f.core.direction == 1);
    if (k == 2)
      assert(f.core.fault_address == 0xfffffffc && f.core.error_code == 6);
    if (k == 3)
      assert(f.core.eip == 0x10000 &&
             f.fp.x87.instruction_pointer == 0x80001234);
    if (k == 4 || k == 8)
      assert(f.core.mxcsr == 1);
    if (k == 6 || k == 7) {
      assert(f.fp.x87.significand[0] == seed &&
             f.fp.x87.sign_exponent[0] == 0xffff);
      assert(f.fp.x87.tags == (k == 7 ? 0xffff : 0));
      assert(!(f.fp.x87.status & 0x3800));
    }
    /* A post-fault boundary must also work; previous frame cannot remain live.
     */
    assert(gamepkg_x86_run_pinned(normal, &f) == 0);
    entered = 0;
    assert(!memcmp(&f, &zero, sizeof(f)));
  }
  restore_cases();
}
#ifdef _WIN32
static DWORD WINAPI worker(LPVOID arg) {
  seed = (unsigned)(uintptr_t)arg;
  for (unsigned i = 0; i < 1000; ++i)
    cases();
  return 0;
}
#else
static void *worker(void *arg) {
  seed = (unsigned)(uintptr_t)arg;
  for (unsigned i = 0; i < 1000; ++i)
    cases();
  return NULL;
}
#endif
int main(void) {
  seed = 0x123456;
  cases();
  restore_control_space();
#ifdef _WIN32
  HANDLE threads[8];
  for (unsigned i = 0; i < 8; ++i) {
    threads[i] =
        CreateThread(NULL, 0, worker, (LPVOID)(uintptr_t)(i + 1), 0, NULL);
    assert(threads[i]);
  }
  for (unsigned i = 0; i < 8; ++i) {
    assert(WaitForSingleObject(threads[i], INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(threads[i]));
  }
#else
  pthread_t threads[8];
  for (unsigned i = 0; i < 8; ++i)
    assert(
        !pthread_create(&threads[i], NULL, worker, (void *)(uintptr_t)(i + 1)));
  for (unsigned i = 0; i < 8; ++i)
    assert(!pthread_join(threads[i], NULL));
#endif
  puts("FP boundary snapshot/capture/restore: five fault vectors, raw "
       "x87/MMX/XMM payloads, "
       "nested/light "
       "boundaries, pending faults, real divide retry, rejection and 8000 "
       "worker iterations "
       "passed");
  return 0;
}
