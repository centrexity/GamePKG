/* Asset-free import-boundary snapshots. Included after restore helpers. */
static void capture_cases(void) {
  initialize();
  GamePkgX86FpFault saved, fault, other, after;
  /* Pending unmasked x87 and SSE conditions must be copied, never delivered. */
  gamepkg_x86_x87_state()->control &= (uint16_t)~1u;
  gamepkg_x86_x87_state()->status |= 0x8081;
  gamepkg_x86_set_mxcsr(1, 0, 0);
  assert(!gamepkg_x86_capture_pinned(0x10000, 0x12345678, &saved));
  assert(g_ebp ==
         0xdeadbeef); /* Capture cannot publish a different local EBP. */
  assert(!saved.core.vector && !saved.core.fault_address &&
         !saved.core.error_code);
  verify_fp(&saved.fp);
  assert(gamepkg_x86_run_pinned(generated_divide, &fault) == 1);
  assert(!memcmp(&saved, &fault, sizeof(saved)));
  clobber_registers();
  assert(!gamepkg_x86_restore_pinned(&saved));
  assert(!gamepkg_x86_capture_pinned(saved.core.eip, g_ebp, &after));
  assert(!memcmp(&saved, &after, sizeof(saved)));
  verify_core(&saved.core);
  verify_fp(&saved.fp);

  /* Alternating host-owned guest contexts on one worker: raw MMX aliases,
   * XMM NaN payloads, FS/DF, controls and caller's stack must stay independent.
   * Binding RAM/segments and scheduling threads are separate host duties. */
  initialize();
  gamepkg_x86_mmx_commit(3, UINT64_C(0xfedcba9876543210));
  bank(7)[3] = 0xffc01234;
  g_fs_base = 0x1000;
  g_esp = 0x2000;
  g_df = 0;
  assert(!gamepkg_x86_capture_pinned(0x20000, 0x3000, &other));
  for (unsigned i = 0; i < 4; ++i) {
    const GamePkgX86FpFault *selected = i & 1 ? &other : &saved;
    assert(!gamepkg_x86_restore_pinned(selected));
    assert(!gamepkg_x86_capture_pinned(selected->core.eip, g_ebp, &after));
    assert(!memcmp(selected, &after, sizeof(after)));
  }

  initialize();
  GamePkgX86FpSource good = descriptor, bad;
  memset(&after, 0xc3, sizeof(after));
  memcpy(&other, &after, sizeof(other));
#define CAPTURE_BAD(expr)                                                      \
  do {                                                                         \
    assert((expr) == -1);                                                      \
    assert(!memcmp(&after, &other, sizeof(after)));                            \
  } while (0)
  CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, NULL, &good));
  CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, &after, NULL));
  CAPTURE_BAD(gamepkg_x86_capture_registers(
      0, 0, (GamePkgX86FpFault *)((uintptr_t)&after + 1), &good));
  CAPTURE_BAD(gamepkg_x86_capture_registers(
      0, 0, (GamePkgX86FpFault *)(UINTPTR_MAX & ~(uintptr_t)7), &good));
  bad = good;
  bad.x87 = &after.fp.x87;
  CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, &after, &bad));
  bad = good;
  bad.x87 = NULL;
  CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, &after, &bad));
  CAPTURE_BAD(gamepkg_x86_capture_registers(
      0, 0, &after, (const GamePkgX86FpSource *)&after));
  for (unsigned i = 0; i < 8; ++i) {
    bad = good;
    bad.xmm[i] = after.fp.xmm[i];
    CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, &after, &bad));
    bad = good;
    bad.xmm[i] = NULL;
    CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, &after, &bad));
    bad = good;
    bad.xmm[i] = (const uint32_t *)((uintptr_t)good.xmm[i] + 1);
    CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, &after, &bad));
    for (unsigned j = 0; j < i; ++j) {
      bad = good;
      bad.xmm[i] = good.xmm[j] + 1;
      CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, &after, &bad));
    }
  }
  g_df = -1;
  CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, &after, &good));
  assert(g_df == -1);
  g_df = 2;
  CAPTURE_BAD(gamepkg_x86_capture_registers(0, 0, &after, &good));
  assert(g_df == 2);
  g_df = 1;
#undef CAPTURE_BAD
  /* Explicit non-TLS banks are supported too, without changing live banks. */
  GamePkgX87State raw = *good.x87;
  uint32_t lanes[8][4];
  bad.x87 = &raw;
  for (unsigned i = 0; i < 8; ++i) {
    for (unsigned j = 0; j < 4; ++j)
      lanes[i][j] = 0x87654321u ^ (i << 4) ^ j;
    bad.xmm[i] = lanes[i];
  }
  assert(!gamepkg_x86_capture_registers(UINT32_MAX, 0x87654321, &after, &bad));
  assert(after.core.eip == UINT32_MAX && after.core.ebp == 0x87654321);
  assert(!memcmp(after.fp.xmm, lanes, sizeof(lanes)));
  assert(g_ebp == 0xdeadbeef && g_fs_base == 0xff000000 && g_df == 1);
  initialize();
}
