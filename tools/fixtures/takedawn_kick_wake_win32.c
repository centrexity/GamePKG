/* Maintainer-only synthetic handoffs; the included helpers are extracted
 * from the real pinned/staged runtime by check_takedawn_kick_handoff.py. */
#include <windows.h>
#include <mmsystem.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define HOST_THREAD_LOCAL _Thread_local
#define NV2A_KICK_REG 0x100410u
#define NV2A_KICK_BIT 0x10000u
static void *g_nv2a_memory;
#include "gamepkg_test_host_sleep.inc"
/* Fault injection wraps only the real kick helper. Its fallback still calls
 * the unchanged real HostSleepNs implementation included above. */
static int fault_mode;
static volatile LONG fault_hits, fallback_calls;
static HANDLE WINAPI fixture_create_timer(LPSECURITY_ATTRIBUTES attrs, LPCWSTR name, DWORD flags,
                                          DWORD access) {
    if (fault_mode == 1) {
        InterlockedIncrement(&fault_hits);
        return NULL;
    }
    return CreateWaitableTimerExW(attrs, name, flags, access);
}
static BOOL WINAPI fixture_arm_timer(HANDLE timer, const LARGE_INTEGER *due, LONG period,
                                     PTIMERAPCROUTINE routine, LPVOID arg, BOOL resume) {
    if (fault_mode == 2) {
        InterlockedIncrement(&fault_hits);
        return FALSE;
    }
    return SetWaitableTimer(timer, due, period, routine, arg, resume);
}
static DWORD WINAPI fixture_wait(DWORD n, const HANDLE *handles, BOOL all, DWORD timeout) {
    if (fault_mode == 3) {
        InterlockedIncrement(&fault_hits);
        return WAIT_FAILED;
    }
    return WaitForMultipleObjects(n, handles, all, timeout);
}
static void fixture_fallback(uint64_t ns) {
    InterlockedIncrement(&fallback_calls);
    xbox_HostSleepNs(ns);
}
#define CreateWaitableTimerExW fixture_create_timer
#define SetWaitableTimer fixture_arm_timer
#define WaitForMultipleObjects fixture_wait
#define xbox_HostSleepNs fixture_fallback
#include "gamepkg_test_kick_helper.inc"
#undef CreateWaitableTimerExW
#undef SetWaitableTimer
#undef WaitForMultipleObjects
#undef xbox_HostSleepNs
static HANDLE done;
static volatile LONG stop, submitted, completed, payload, violation;
static DWORD WINAPI worker(void *unused) {
    (void)unused;
    InterlockedExchange(&g_gamepkg_ack_tid, (LONG)GetCurrentThreadId());
    volatile LONG *bit = (volatile LONG *)((char *)g_nv2a_memory + NV2A_KICK_REG);
    while (!InterlockedCompareExchange(&stop, 0, 0)) {
        LONG s = InterlockedCompareExchange(&submitted, 0, 0),
             c = InterlockedCompareExchange(&completed, 0, 0);
        if (s > c) {
            if (InterlockedCompareExchange(&payload, 0, 0) != s)
                InterlockedExchange(&violation, 1);
            InterlockedExchange(&completed, s);
        }
        if (InterlockedCompareExchange(bit, 0, 0) & NV2A_KICK_BIT) {
            if (InterlockedCompareExchange(&completed, 0, 0) <
                InterlockedCompareExchange(&submitted, 0, 0))
                continue;
            InterlockedAnd(bit, ~NV2A_KICK_BIT);
            SetEvent(done);
        }
        kick_nap_us(50);
    }
    return 0;
}
static LARGE_INTEGER freq;
static double us(void) {
    LARGE_INTEGER t;
    QueryPerformanceCounter(&t);
    return (double)t.QuadPart * 1e6 / (double)freq.QuadPart;
}
static int cmp(const void *a, const void *b) {
    double x = *(const double *)a, y = *(const double *)b;
    return (x > y) - (x < y);
}
static void phase(const char *name, int wake, int n) {
    HANDLE saved = g_gamepkg_kick_ready;
    if (!wake)
        g_gamepkg_kick_ready = NULL;
    else
        ResetEvent(saved);
    InterlockedExchange(&stop, 0);
    InterlockedExchange(&submitted, 0);
    InterlockedExchange(&completed, 0);
    InterlockedExchange(&payload, 0);
    InterlockedExchange(&violation, 0);
    ResetEvent(done);
    volatile LONG *bit = (volatile LONG *)((char *)g_nv2a_memory + NV2A_KICK_REG);
    InterlockedExchange(bit, NV2A_KICK_BIT);
    /* An event posted before the worker waits must be retained. */
    xbox_Nv2aKickWake();
    HANDLE th = CreateThread(NULL, 0, worker, NULL, 0, NULL);
    if (!th)
        exit(10);
    if (WaitForSingleObject(done, 2000) != WAIT_OBJECT_0)
        exit(11);
    double *v = calloc(n, sizeof(double)), sum = 0;
    if (!v)
        exit(12);
    for (int i = 1; i <= n; i++) {
        double t = us();
        InterlockedExchange(bit, NV2A_KICK_BIT);
        /* Repeated notifications must coalesce without losing future requests. */
        xbox_Nv2aKickWake();
        xbox_Nv2aKickWake();
        xbox_Nv2aKickWake();
        if (WaitForSingleObject(done, 2000) != WAIT_OBJECT_0)
            exit(13);
        if (InterlockedCompareExchange(bit, 0, 0) & NV2A_KICK_BIT)
            exit(14);
        if (InterlockedCompareExchange(&completed, 0, 0) < i - 1)
            exit(15);
        v[i - 1] = us() - t;
        sum += v[i - 1];
        InterlockedExchange(&payload, i);
        InterlockedExchange(&submitted, i);
    }
    InterlockedExchange(&stop, 1);
    if (saved)
        SetEvent(saved);
    if (WaitForSingleObject(th, 2000) != WAIT_OBJECT_0)
        exit(16);
    CloseHandle(th);
    if (InterlockedCompareExchange(&violation, 0, 0))
        exit(17);
    qsort(v, n, sizeof(double), cmp);
    printf("%s,%d,%.3f,%.3f,%.3f,%.3f,0\n", name, n, sum / n, v[(n - 1) / 2], v[(int)(n * .99) - 1],
           v[n - 1]);
    fflush(stdout);
    free(v);
    g_gamepkg_kick_ready = saved;
}
int main(int argc, char **argv) {
    if (argc > 2 || (argc == 2 && (strlen(argv[1]) != 1 || argv[1][0] < '0' || argv[1][0] > '3')))
        return 2;
    fault_mode = argc == 2 ? argv[1][0] - '0' : 0;
    QueryPerformanceFrequency(&freq);
    g_nv2a_memory = calloc(1, NV2A_KICK_REG + 4);
    done = CreateEventW(NULL, FALSE, FALSE, NULL);
    g_gamepkg_kick_ready = CreateEventW(NULL, FALSE, FALSE, NULL);
    if (!g_nv2a_memory || !done || !g_gamepkg_kick_ready)
        return 3;
    unsigned ret = timeBeginPeriod(1);
    puts("phase,kicks,mean_us,median_us,p99_us,max_us,ordering_violations");
    if (fault_mode) {
        phase("forced_timer_failure_fallback", 1, 500);
        if (!InterlockedCompareExchange(&fault_hits, 0, 0) ||
            !InterlockedCompareExchange(&fallback_calls, 0, 0))
            return 18;
        printf("#fault_mode=%d hits=%ld fallback_calls=%ld\n", fault_mode, (long)fault_hits,
               (long)fallback_calls);
    } else {
        phase("fallback_without_event_A", 0, 1000);
        phase("event_early_and_repeated", 1, 10000);
        phase("fallback_without_event_B", 0, 1000);
        phase("event_repeat", 1, 10000);
    }
    if (ret == TIMERR_NOERROR) {
        timeEndPeriod(1);
    }
    CloseHandle(done);
    CloseHandle(g_gamepkg_kick_ready);
    free(g_nv2a_memory);
    return 0;
}
