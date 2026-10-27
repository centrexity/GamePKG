/* Included inside the reviewed kernel_hal.c, after its gate implementation.
 * The site adapter calls enable only after the instruction following STI.
 * This is a native critical section, not a run-time instruction decoder.
 */
extern RECOMP_TLS uint32_t g_eflags;

void xbox_NativeInterruptDisable(void)
{
    if (!t_native_irq_masked) {
        gate_enter();
        t_native_irq_masked = 1;
        InterlockedIncrement(&g_native_irq_masked_count);
    }
    g_eflags &= ~0x200u;
}

void xbox_NativeInterruptEnableAfterShadow(void)
{
    g_eflags |= 0x200u;
    if (t_native_irq_masked) {
        t_native_irq_masked = 0;
        InterlockedDecrement(&g_native_irq_masked_count);
        xbox_DispatchGateLeave();
    }
}

int xbox_NativeInterruptsEnabled(void)
{
    return !t_native_irq_masked;
}
