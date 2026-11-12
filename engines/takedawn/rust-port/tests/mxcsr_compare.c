/* Asset-free native SSE control/rounding/FTZ/thread-local qualification. */
#define _GNU_SOURCE
#include <assert.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>
#include <xmmintrin.h>
extern uint32_t gamepkg_takedawn_stmxcsr(void);
extern void gamepkg_takedawn_ldmxcsr(uint32_t);
static float multiply(float a, float b) {
    __asm__ volatile("mulss %1, %0" : "+x"(a) : "x"(b));
    return a;
}
static int convert(float value) {
    int result;
    __asm__ volatile("cvtss2si %1, %0" : "=r"(result) : "x"(value));
    return result;
}
static uint32_t bits(float value) {
    uint32_t out;
    memcpy(&out, &value, sizeof out);
    return out;
}
static pthread_barrier_t barrier;
static void *worker(void *argument) {
    uint32_t original = _mm_getcsr(), word = (uint32_t)(uintptr_t)argument;
    gamepkg_takedawn_ldmxcsr(word);
    pthread_barrier_wait(&barrier);
    for (unsigned i = 0; i < 4096; i++) {
        assert(gamepkg_takedawn_stmxcsr() == word);
        assert(_mm_getcsr() == word);
    }
    pthread_barrier_wait(&barrier);
    gamepkg_takedawn_ldmxcsr(original);
    return NULL;
}
static void reserved_bit_fault(int use_rust) {
    pid_t child = fork();
    assert(child >= 0);
    if (!child) {
        struct rlimit no_core = {0, 0};
        assert(setrlimit(RLIMIT_CORE, &no_core) == 0);
        if (use_rust) gamepkg_takedawn_ldmxcsr(0x80001F80u);
        else _mm_setcsr(0x80001F80u);
        _exit(0);
    }
    int status;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGSEGV);
}
int main(void) {
    uint32_t original = _mm_getcsr();
    unsigned cases = 0;
    const int positive[] = {2, 1, 2, 1}, negative[] = {-2, -2, -1, -1};
    for (unsigned round = 0; round < 4; round++)
        for (unsigned ftz = 0; ftz < 2; ftz++)
            for (unsigned flags = 0; flags < 64; flags++) {
                uint32_t word = 0x1F80u | round << 13 | ftz << 15 | flags;
                _mm_setcsr(word);
                assert(gamepkg_takedawn_stmxcsr() == word);
                assert(_mm_getcsr() == word); /* read leaves native state intact */
                gamepkg_takedawn_ldmxcsr(word ^ 0x8000u);
                assert(_mm_getcsr() == (word ^ 0x8000u));
                gamepkg_takedawn_ldmxcsr(word);
                assert(_mm_getcsr() == word);
                assert(convert(1.5f) == positive[round]);
                assert(convert(-1.5f) == negative[round]);
                gamepkg_takedawn_ldmxcsr(word);
                float a = multiply(0x1p-126f, 0.5f);
                uint32_t rust_status = _mm_getcsr();
                _mm_setcsr(word);
                float b = multiply(0x1p-126f, 0.5f);
                assert(bits(a) == bits(b) && _mm_getcsr() == rust_status);
                assert(bits(a) == (ftz ? 0u : 0x00400000u));
                /* The exact original game setup: read, OR FTZ, write. */
                _mm_setcsr(word);
                gamepkg_takedawn_ldmxcsr(gamepkg_takedawn_stmxcsr() | 0x8000u);
                assert(_mm_getcsr() == (word | 0x8000u));
                cases++;
            }
    _mm_setcsr(original);
    assert(pthread_barrier_init(&barrier, NULL, 3) == 0);
    pthread_t threads[2];
    assert(pthread_create(&threads[0], NULL, worker, (void *)(uintptr_t)0x3F80u) == 0);
    assert(pthread_create(&threads[1], NULL, worker, (void *)(uintptr_t)0xDF95u) == 0);
    pthread_barrier_wait(&barrier);
    assert(gamepkg_takedawn_stmxcsr() == original);
    pthread_barrier_wait(&barrier);
    assert(gamepkg_takedawn_stmxcsr() == original);
    for (unsigned i = 0; i < 2; i++) assert(pthread_join(threads[i], NULL) == 0);
    assert(pthread_barrier_destroy(&barrier) == 0);
    reserved_bit_fault(0);
    reserved_bit_fault(1);
    assert(_mm_getcsr() == original);
    printf("PASS: %u native MXCSR cases; all rounding modes, sticky status, FTZ/subnormal output, original setup, 8192 thread-local reads and matching reserved-bit faults.\n", cases);
}
