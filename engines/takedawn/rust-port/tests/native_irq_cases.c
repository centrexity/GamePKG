/* Included by the original irq_safe_points fixture, before main. */
extern void xbox_NativeInterruptDisable(void);
extern void xbox_NativeInterruptEnableAfterShadow(void);
extern int xbox_NativeInterruptsEnabled(void);
extern void gamepkg_takedawn_cli(void);
extern void gamepkg_takedawn_sti_after_shadow(void);
extern void gamepkg_takedawn_wbinvd(void);
extern RECOMP_TLS uint32_t g_eflags;
static volatile LONG native_runs, native_shadow, native_saw_shadow;
static unsigned long native_tid;
static void native_isr(void)
{
    InterlockedIncrement(&native_runs);
    native_tid = GetCurrentThreadId();
    native_saw_shadow = native_shadow;
    g_eax = 0xBAD;
    g_esi = 0xBAD;
}
static DWORD WINAPI native_poster(LPVOID out)
{
    int *results = out;
    results[0] = deliver(XBOX_IRQ_APU);
    results[1] = xbox_NativeInterruptsEnabled();
    results[2] = xbox_IrqlBlocksInterrupts();
    return 0;
}
static DWORD WINAPI native_gate_probe(LPVOID out)
{
    *(int *)out = xbox_DispatchGateTryEnter();
    if (*(int *)out) xbox_DispatchGateLeave();
    return 0;
}
static void native_thread_join(HANDLE thread)
{
    check(thread && WaitForSingleObject(thread, 5000) == WAIT_OBJECT_0,
          "native IF: control thread completes within five seconds");
    if (thread) CloseHandle(thread);
}
static void native_mask_cases(void)
{
    uint32_t flags = 0x00240ED7u;
    unsigned long me = GetCurrentThreadId();
    int before = xbox_IrqlCurrent(), probe = 0;
    HANDLE thread;
    reset_counts();
    xbox_IrqSetHandler(XBOX_IRQ_APU, native_isr);
    native_runs = native_shadow = native_saw_shadow = 0;
    native_tid = 0;
    g_eflags = flags;
    gamepkg_takedawn_cli();
    gamepkg_takedawn_cli(); /* clearing an already-clear IF is not nesting */
    check(!xbox_NativeInterruptsEnabled() && g_eflags == (flags & ~0x200u),
          "native IF: CLI clears only IF, repeated CLI is idempotent");
    check(xbox_IrqlCurrent() == before && xbox_IrqlBlocksInterrupts(),
          "native IF: interrupts masked without changing visible IRQL");
    check(deliver(XBOX_IRQ_APU) == XBOX_IRQ_POSTED,
          "native IF: recursive same-thread post cannot enter ISR");
    xbox_IrqSafePoint();
    check(native_runs == 0, "native IF: safe point inside CLI does not deliver");
    g_eax = 0x12345678u;
    g_esi = g_eax; /* exact original MOV after STI: its interrupt shadow */
    native_shadow = 1;
    gamepkg_takedawn_sti_after_shadow();
    check(native_runs == 1 && native_saw_shadow == 1 && native_tid == me,
          "native IF: pending ISR delivered on holder after shadow instruction");
    check(g_eax == 0x12345678u && g_esi == g_eax && g_eflags == flags,
          "native IF: ISR preserves completed sample registers and flags");
    check(xbox_NativeInterruptsEnabled() && xbox_IrqlCurrent() == before
          && !xbox_IrqlBlocksInterrupts() && xbox_DispatchGateOwner() == 0,
          "native IF: one enable releases repeated CLI without leaking the gate");
    gamepkg_takedawn_sti_after_shadow();
    thread = CreateThread(NULL, 0, native_gate_probe, &probe, 0, NULL);
    native_thread_join(thread);
    check(probe == 1, "native IF: redundant STI does not corrupt gate ownership");

    for (int mode = 0; mode < 3; ++mode) {
        int results[3] = {-1,-1,-1};
        set_env(RENV_IRQ_SAFE_POINTS, mode == 1 ? "0" : "1");
        set_env(RENV_IRQ_SAFE_FORCE, mode == 2 ? "1" : "0");
        set_env(RENV_IRQ_SAFE_MS, "1");
        native_runs = native_shadow = native_saw_shadow = 0;
        gamepkg_takedawn_cli();
        thread = CreateThread(NULL, 0, native_poster, results, 0, NULL);
        native_thread_join(thread);
        check(results[0] == XBOX_IRQ_POSTED && results[1] == 1 && results[2],
              "native IF: other thread sees own IF enabled but global mask active");
        Sleep(5);
        check(deliver(XBOX_IRQ_APU) == XBOX_IRQ_POSTED && native_runs == 0,
              "native IF: forced/legacy diagnostic modes cannot bypass CLI");
        g_esi = g_eax;
        native_shadow = 1;
        gamepkg_takedawn_sti_after_shadow();
        check(native_runs == 1 && native_saw_shadow == 1,
              "native IF: repeated posted IRQ coalesces and releases after STI");
    }
    set_env(RENV_IRQ_SAFE_POINTS, "1");
    set_env(RENV_IRQ_SAFE_FORCE, "0");
    set_env(RENV_IRQ_SAFE_MS, "250");
    native_runs = 0;
    {
        int saved = xbox_IrqlEnterInterrupt(DISPATCH_LEVEL);
        gamepkg_takedawn_cli();
        check(deliver(XBOX_IRQ_APU) == XBOX_IRQ_POSTED, "native IF: masked inside raised IRQL");
        g_esi = g_eax;
        gamepkg_takedawn_sti_after_shadow();
        check(native_runs == 1 && xbox_IrqlCurrent() == DISPATCH_LEVEL
              && xbox_DispatchGateOwner() == me && xbox_IrqlBlocksInterrupts(),
              "native IF: STI leaves enclosing IRQL and gate ownership intact");
        xbox_IrqlLeaveInterrupt(saved);
    }
    check(xbox_DispatchGateOwner() == 0 && !xbox_IrqlBlocksInterrupts(),
          "native IF: outer lower releases the remaining gate");
    xbox_IrqSetHandler(XBOX_IRQ_APU, apu_routine);
}

static void native_cache_alias_case(void)
{
    HANDLE mapping = CreateFileMappingW(INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE, 0, 4096, NULL);
    unsigned char *a = mapping ? MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, 4096) : NULL;
    unsigned char *b = mapping ? MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, 4096) : NULL;
    int ok = a && b && a != b;
    if (ok) {
        for (int round = 0; round < 256; ++round) {
            for (unsigned i = 0; i < 4096; ++i) a[i] = (unsigned char)(i ^ round);
            gamepkg_takedawn_wbinvd();
            for (unsigned i = 0; i < 4096; ++i) if (b[i] != (unsigned char)(i ^ round)) ok = 0;
        }
    }
    check(ok, "native cache: one million published bytes agree through separate coherent RAM views");
    if (a) UnmapViewOfFile(a);
    if (b) UnmapViewOfFile(b);
    if (mapping) CloseHandle(mapping);
}
