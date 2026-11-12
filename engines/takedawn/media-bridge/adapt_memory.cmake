# A failed required alias must stop startup before generated game code executes.
# Successful mapping behavior is unchanged. Never modify the pinned checkout.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT for the pinned memory adaptation")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite the pinned upstream memory source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "c83cb0a4586ef3298911f4639e79ec1f904b4133b8469676ff0fa461b63f82f4")
    message(FATAL_ERROR "Unreviewed xboxrecomp memory layout; pinned hash differs")
endif()
file(READ "${INPUT}" source)
set(before [[            XR_ERROR( "  WARNING: tiled aperture at 0x%08X failed"
                    " (error %lu); rendering writes will fault\n",
                    XBOX_TILED_BASE, GetLastError());]])
set(after [[            XR_ERROR( "GamePKG: required tiled memory mapping at 0x%08X failed"
                    " (error %lu); stopping before game startup.\n",
                    XBOX_TILED_BASE, (unsigned long)GetLastError());
            return FALSE;]])
string(FIND "${source}" "${before}" location)
if(location LESS 0)
    message(FATAL_ERROR "Pinned tiled mapping failure branch is missing")
endif()
string(REPLACE "${before}" "${after}" adapted "${source}")
# The executor is enabled during ordinary play. It must not implicitly enable
# register dumps and the diagnostic framebuffer checksum on its worker thread.
# Explicit register tracing and pushbuffer surveys retain their existing logs.
set(trace_before [[        s_nv2a_trace = recomp_env(RENV_NV2A_TRACE) != NULL
                    || recomp_env(RENV_PB_SCAN) != NULL
                    || recomp_env(RENV_PB_EXEC) != NULL;]])
set(trace_after [[        s_nv2a_trace = recomp_env(RENV_NV2A_TRACE) != NULL
                    || recomp_env(RENV_PB_SCAN) != NULL;]])
string(FIND "${adapted}" "${trace_before}" trace_location)
if(trace_location LESS 0)
    message(FATAL_ERROR "Pinned explicit NV2A trace policy site is missing")
endif()
string(REPLACE "${trace_before}" "${trace_after}" adapted "${adapted}")
# The periodic executor report also carries renderer health diagnostics. Keep
# its original enablement when execution is on, independently of verbose logs.
set(report_before [[if (s_nv2a_trace && now_ms - last_report > (uint64_t)report_ms)]])
set(report_after [[if ((s_nv2a_trace || s_pb_exec) && now_ms - last_report > (uint64_t)report_ms)]])
string(FIND "${adapted}" "${report_before}" report_location)
if(report_location LESS 0)
    message(FATAL_ERROR "Pinned periodic renderer health report site is missing")
endif()
string(REPLACE "${report_before}" "${report_after}" adapted "${adapted}")
if(WINDOWS_LAYOUT)
set(wake_before_0 [[static void kick_nap_us(unsigned us)
{
#if defined(_WIN32)
    /* Sleep() is whole milliseconds; NtDelayExecution takes 100 ns units,
     * which Wine honours (select with a timeval) and native Windows rounds
     * to its timer resolution. */
    typedef LONG (WINAPI *delay_fn)(BOOLEAN, PLARGE_INTEGER);
    static delay_fn delay;
    static int looked;
    if (!looked) {
        HMODULE nt = GetModuleHandleA("ntdll.dll");
        looked = 1;
        if (nt)
            delay = (delay_fn)(void *)GetProcAddress(nt, "NtDelayExecution");
    }
    if (delay) {
        LARGE_INTEGER t;
        t.QuadPart = -(LONGLONG)us * 10;
        delay(FALSE, &t);
    } else {
        Sleep(1);
    }
#else
    struct timespec ts;
    ts.tv_sec = 0;
    ts.tv_nsec = (long)us * 1000L;
    nanosleep(&ts, NULL);
#endif
}

]])
set(wake_after_0 [[/* Private candidate: an auto-reset event coalesces notifications from the
 * title's existing lowered MMIO poll. The original scan/GET/flush ordering
 * remains in nv2a_ack_thread; this event only interrupts its idle nap. */
#if defined(_WIN32)
static HANDLE g_gamepkg_kick_ready;
static volatile LONG g_gamepkg_ack_tid;
void xbox_Nv2aKickWake(void)
{
    if (g_gamepkg_kick_ready && g_nv2a_memory &&
        (*(volatile uint32_t *)((char *)g_nv2a_memory + NV2A_KICK_REG) & NV2A_KICK_BIT))
        SetEvent(g_gamepkg_kick_ready);
}
#endif
static void kick_nap_us(unsigned us)
{
#if defined(_WIN32)
    /* Only the command worker consumes the event. The optional statistics
     * observer has its own existing per-thread sleep timer. */
    if (g_gamepkg_kick_ready && GetCurrentThreadId() ==
        (DWORD)InterlockedCompareExchange(&g_gamepkg_ack_tid, 0, 0)) {
        static HANDLE timer;
        static int looked;
        if (!looked) {
            looked = 1;
            timer = CreateWaitableTimerExW(NULL, NULL, 0x00000002, TIMER_ALL_ACCESS);
        }
        if (timer) {
            LARGE_INTEGER due;
            HANDLE handles[2] = { g_gamepkg_kick_ready, timer };
            due.QuadPart = -(LONGLONG)us * 10;
            if (SetWaitableTimer(timer, &due, 0, NULL, NULL, FALSE)) {
                DWORD r = WaitForMultipleObjects(2, handles, FALSE, INFINITE);
                if (r == WAIT_OBJECT_0) {
                    CancelWaitableTimer(timer);
                    return;
                }
                if (r == WAIT_OBJECT_0 + 1)
                    return;
            }
        }
    }
    /* Failed event/timer setup retains the qualified timer-only path. */
    xbox_HostSleepNs((uint64_t)us * 1000u);
#else
    struct timespec ts;
    ts.tv_sec = 0;
    ts.tv_nsec = (long)us * 1000L;
    nanosleep(&ts, NULL);
#endif
}

]])
string(FIND "${adapted}" "${wake_before_0}" wake_location)
if(wake_location LESS 0)
    message(FATAL_ERROR "Pinned kick-wake source site 0 missing")
endif()
string(REPLACE "${wake_before_0}" "${wake_after_0}" adapted "${adapted}")
set(wake_before_1 [[    xbox_log_thread_role("nv2a-ack", 0);]])
set(wake_after_1 [[    xbox_log_thread_role("nv2a-ack", 0);
#if defined(_WIN32)
    InterlockedExchange(&g_gamepkg_ack_tid, (LONG)GetCurrentThreadId());
#endif]])
string(FIND "${adapted}" "${wake_before_1}" wake_location)
if(wake_location LESS 0)
    message(FATAL_ERROR "Pinned kick-wake source site 1 missing")
endif()
string(REPLACE "${wake_before_1}" "${wake_after_1}" adapted "${adapted}")
set(wake_before_2 [[    g_nv2a_ack_stop = 0;]])
set(wake_after_2 [[    g_nv2a_ack_stop = 0;
#if defined(_WIN32)
    /* Process-lifetime object, created before the worker and game start.
     * Auto-reset retains an early request and coalesces repeated polls. */
    if (!g_gamepkg_kick_ready)
        g_gamepkg_kick_ready = CreateEventW(NULL, FALSE, FALSE, NULL);
#endif]])
string(FIND "${adapted}" "${wake_before_2}" wake_location)
if(wake_location LESS 0)
    message(FATAL_ERROR "Pinned kick-wake source site 2 missing")
endif()
string(REPLACE "${wake_before_2}" "${wake_after_2}" adapted "${adapted}")
set(wake_before_3 [[                if (*kr & NV2A_KICK_BIT) {
                    *kr &= ~NV2A_KICK_BIT;]])
set(wake_after_3 [[                if (*kr & NV2A_KICK_BIT) {
                    /* PUT precedes this flush in the producer. Recheck after
                     * observing the flush so a publication between the first
                     * PUT read and this bit cannot be acknowledged early. */
                    __atomic_thread_fence(__ATOMIC_ACQUIRE);
                    if (*(volatile uint32_t *)((char *)regs + NV2A_USER_DMA_PUT)
                            != g_scanned_put) {
                        last_active = now;
                        break;
                    }
                    *kr &= ~NV2A_KICK_BIT;]])
string(FIND "${adapted}" "${wake_before_3}" wake_location)
if(wake_location LESS 0)
    message(FATAL_ERROR "Pinned kick-wake source site 3 missing")
endif()
string(REPLACE "${wake_before_3}" "${wake_after_3}" adapted "${adapted}")
endif()
file(WRITE "${OUTPUT}.tmp" "/* GamePKG: stop cleanly when the required tiled alias cannot map. */\n${adapted}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
