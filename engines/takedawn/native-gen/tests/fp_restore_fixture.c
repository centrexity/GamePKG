/* Included after capture helpers, before their worker driver. */
static GamePkgX86FpTarget current_target(void) {
  GamePkgX86FpTarget target = {.x87 = gamepkg_x86_x87_state()};
  for (unsigned i = 0; i < 8; ++i)
    target.xmm[i] = bank(i);
  return target;
}
static void clobber_registers(void) {
  g_eax = g_ecx = g_edx = g_ebx = g_esp = g_ebp = g_esi = g_edi = 0xcccccccc;
  g_eflags = g_fs_base = 0xdddddddd;
  g_df = 0;
  gamepkg_x86_set_mxcsr(0x7f80, 0, 0);
  memset(gamepkg_x86_x87_state(), 0xcc, sizeof(GamePkgX87State));
  for (unsigned i = 0; i < 8; ++i)
    for (unsigned j = 0; j < 4; ++j)
      bank(i)[j] = 0xcccccccc;
}
static void verify_core(const GamePkgX86Fault *c) {
  assert(g_eax == c->eax && g_ecx == c->ecx && g_edx == c->edx &&
         g_ebx == c->ebx);
  assert(g_esp == c->esp && g_ebp == c->ebp && g_esi == c->esi &&
         g_edi == c->edi);
  assert(g_eflags == c->eflags && g_fs_base == c->fs_base &&
         (uint32_t)g_df == c->direction);
  assert(gamepkg_x86_get_mxcsr() == c->mxcsr);
}
static void restore_rejections(void) {
  initialize();
  GamePkgX86FpFault good, after, bad;
  assert(gamepkg_x86_run_pinned(generated_divide, &good) == 1);
  GamePkgX86FpTarget target = current_target(), wrong;
#define REJECT(expr)                                                           \
  do {                                                                         \
    assert((expr) == -1);                                                      \
    assert(gamepkg_x86_run_pinned(generated_divide, &after) == 1);             \
    assert(!memcmp(&good, &after, sizeof(good)));                              \
  } while (0)
  REJECT(gamepkg_x86_restore_fault_registers(NULL, &target));
  REJECT(gamepkg_x86_restore_fault_registers(&good, NULL));
  REJECT(gamepkg_x86_restore_fault_registers(
      (const GamePkgX86FpFault *)((uintptr_t)&good + 1), &target));
  REJECT(gamepkg_x86_restore_fault_registers(
      (const GamePkgX86FpFault *)(UINTPTR_MAX & ~(uintptr_t)7), &target));
  REJECT(gamepkg_x86_restore_fault_registers(
      &good, (const GamePkgX86FpTarget *)((uintptr_t)&target + 1)));
  REJECT(gamepkg_x86_restore_fault_registers(
      &good, (const GamePkgX86FpTarget *)(UINTPTR_MAX & ~(uintptr_t)7)));
  REJECT(gamepkg_x86_restore_fault_registers(
      &good, (const GamePkgX86FpTarget *)&good));
  for (unsigned bit = 0; bit < 32; ++bit)
    if ((UINT32_C(1) << bit) & ~0xffbfu) {
      bad = good;
      bad.core.mxcsr |= UINT32_C(1) << bit;
      REJECT(gamepkg_x86_restore_fault_registers(&bad, &target));
    }
  bad = good;
  bad.core.direction = 2;
  REJECT(gamepkg_x86_restore_fault_registers(&bad, &target));
  bad.core.direction = UINT32_MAX;
  REJECT(gamepkg_x86_restore_fault_registers(&bad, &target));
  wrong = target;
  wrong.x87 = NULL;
  REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
  wrong = target;
  wrong.x87 = (GamePkgX87State *)((uintptr_t)target.x87 + 1);
  REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
  wrong = target;
  wrong.x87 = (GamePkgX87State *)(UINTPTR_MAX & ~(uintptr_t)7);
  REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
  wrong = target;
  wrong.x87 = &good.fp.x87;
  REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
  wrong = target;
  wrong.x87 = (GamePkgX87State *)&wrong;
  REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
  for (unsigned i = 0; i < 8; ++i) {
    wrong = target;
    wrong.xmm[i] = NULL;
    REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
    wrong = target;
    wrong.xmm[i] = (uint32_t *)((uintptr_t)target.xmm[i] + 1);
    REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
    wrong = target;
    wrong.xmm[i] = (uint32_t *)(UINTPTR_MAX & ~(uintptr_t)3);
    REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
    wrong = target;
    wrong.xmm[i] = good.fp.xmm[i];
    REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
    wrong = target;
    wrong.xmm[i] = (uint32_t *)target.x87;
    REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
    wrong = target;
    wrong.xmm[i] = (uint32_t *)&wrong;
    REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
    for (unsigned j = 0; j < i; ++j) {
      wrong = target;
      wrong.xmm[i] = target.xmm[j] + 1;
      REJECT(gamepkg_x86_restore_fault_registers(&good, &wrong));
    }
  }
#undef REJECT
}
/* Exact known generated entry, only for this fixture. This is deliberately not
 * a general EIP/native-pointer cast or a claim to dispatch interior addresses.
 */
static int retry_divide(const GamePkgX86FpFault *f, GamePkgX86FpFault *result) {
  if (f->core.eip != 0x10000)
    return -1;
  if (gamepkg_x86_restore_pinned(f))
    return -1;
  return gamepkg_x86_run_pinned(generated_divide, result);
}
static void no_new_simd_exception(void) {
  gamepkg_x86_sse_status(0, 0x9999, 0x12345678);
}
static void restore_cases(void) {
  restore_rejections();
  void (*entries[])(void) = {generated_divide, protection, page, deferred, simd,
                             mmx_fault,        empty_fault};
  for (unsigned k = 0; k < 7; ++k) {
    initialize();
    if (k == 4)
      gamepkg_x86_set_mxcsr(0, 0, 0);
    GamePkgX86FpFault saved, copy, after;
    assert(gamepkg_x86_run_pinned(entries[k], &saved) == 1);
    copy = saved;
    clobber_registers();
    assert(gamepkg_x86_restore_pinned(&saved) == 0);
    assert(!memcmp(&saved, &copy, sizeof(saved)));
    verify_core(&saved.core);
    verify_fp(&saved.fp);
    /* Pending x87 or SSE status must not raise merely on restore or entry. */
    assert(gamepkg_x86_run_pinned(no_new_simd_exception, &after) == 0);
    if (k == 3) {
      assert(gamepkg_x86_run_pinned(generated_wait, &after) == 1 &&
             after.core.vector == 16);
      assert(!memcmp(&after, &saved, sizeof(saved)));
      saved.fp.x87.status &= (uint16_t)~0x8081u;
      assert(gamepkg_x86_restore_pinned(&saved) == 0);
      assert(gamepkg_x86_run_pinned(generated_wait, &after) == 0);
    } else if (k == 4) {
      assert(gamepkg_x86_run_pinned(simd, &after) == 1 &&
             after.core.vector == 19);
      assert(!memcmp(&after, &saved, sizeof(saved)));
    } else {
      assert(gamepkg_x86_run_pinned(generated_divide, &after) == 1);
      verify_fp(&after.fp);
    }
  }
  initialize();
  GamePkgX86FpFault saved, after;
  assert(gamepkg_x86_run_pinned(generated_divide, &saved) == 1);
  /* Editing the recoverable cause permits an actual retry at a known entry. */
  saved.core.eax = 99;
  saved.core.edx = 0;
  saved.core.ecx = 3;
  saved.core.eip = 0x10001;
  assert(retry_divide(&saved, &after) == -1 && g_ecx == 0);
  saved.core.eip = 0x10000;
  assert(retry_divide(&saved, &after) == 0 && g_eax == 33 && g_edx == 0 &&
         g_ecx == 3);
  verify_fp(&saved.fp);
  /* Explicit host targets receive FP state; unrelated TLS FP banks stay live.
   */
  initialize();
  assert(gamepkg_x86_run_pinned(generated_divide, &saved) == 1);
  GamePkgX87State local;
  uint32_t lanes[8][4];
  GamePkgX86FpTarget target = {.x87 = &local};
  for (unsigned i = 0; i < 8; ++i)
    target.xmm[i] = lanes[i];
  clobber_registers();
  assert(gamepkg_x86_restore_fault_registers(&saved, &target) == 0);
  verify_core(&saved.core);
  assert(!memcmp(&local, &saved.fp.x87, sizeof(local)));
  assert(!memcmp(lanes, saved.fp.xmm, sizeof(lanes)));
  assert(gamepkg_x86_x87_state()->significand[0] ==
         UINT64_C(0xcccccccccccccccc));
  for (unsigned i = 0; i < 8; ++i)
    for (unsigned j = 0; j < 4; ++j)
      assert(bank(i)[j] == 0xcccccccc);
  /* Restore inside an active boundary cannot replace that boundary. */
  initialize();
}
static void restore_then_fault(void) {
  GamePkgX86FpFault saved;
  assert(gamepkg_x86_run_pinned(generated_divide, &saved) == 1);
  clobber_registers();
  assert(gamepkg_x86_restore_pinned(&saved) == 0);
  generated_divide();
}
static void restore_control_space(void) {
  initialize();
  GamePkgX86FpFault saved;
  assert(gamepkg_x86_run_pinned(generated_divide, &saved) == 1);
  static const uint64_t payloads[8] = {0,
                                       0,
                                       1,
                                       UINT64_C(0x8000000000000000),
                                       UINT64_C(0x8000000000000000),
                                       UINT64_C(0xc123456789abcdef),
                                       UINT64_C(0x8000000000000001),
                                       UINT64_C(0x7fffffffffffffff)};
  static const uint16_t exponents[8] = {0,      0x8000, 0,      0,
                                        0x7fff, 0xffff, 0x7fff, 0x7fff};
  for (unsigned i = 0; i < 8; ++i) {
    saved.fp.x87.significand[i] = payloads[i];
    saved.fp.x87.sign_exponent[i] = exponents[i];
  }
  for (uint32_t csr = 0; csr < 65536; ++csr) {
    uint32_t previous = gamepkg_x86_get_mxcsr();
    saved.core.mxcsr = csr;
    if (csr & 0x40) {
      assert(gamepkg_x86_restore_pinned(&saved) == -1);
      assert(gamepkg_x86_get_mxcsr() == previous);
    } else {
      saved.core.direction = csr & 1;
      assert(gamepkg_x86_restore_pinned(&saved) == 0);
      verify_core(&saved.core);
      verify_fp(&saved.fp);
    }
  }
  /* All mask/precision/rounding/TOP combinations, retaining pending status,
   * tagged empties and noncanonical physical payloads without interpretation.
   */
  saved.core.mxcsr = 0x1f80;
  for (unsigned cw = 0; cw < 1024; ++cw)
    for (unsigned top = 0; top < 8; ++top) {
      saved.fp.x87.control = (uint16_t)(0x40 | (cw & 63) | ((cw >> 6) << 8));
      saved.fp.x87.status = (uint16_t)(0x80ff | (top << 11));
      saved.fp.x87.tags = (uint16_t)(0x1b1b ^ (cw * 97));
      assert(gamepkg_x86_restore_pinned(&saved) == 0);
      verify_core(&saved.core);
      verify_fp(&saved.fp);
    }
  initialize();
}
