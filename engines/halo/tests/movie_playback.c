/* SPDX-License-Identifier: Apache-2.0
 * Generated black/white/silence fixtures only; no game assets. */
#define _GNU_SOURCE
#define _FILE_OFFSET_BITS 64
#define __HALO_LINUX_PLATFORM_H
#include <assert.h>
#include <fenv.h>
#include <fpu_control.h>
#include <inttypes.h>
#include <stdarg.h>
#include <stdio.h>
#include <limits.h>
#include <xmmintrin.h>
#if !defined(__i386__)
/* This 64-bit sanitizer harness exercises the bridge body, not the XDK ABI. */
#define __stdcall
#endif
typedef int BOOL;
typedef unsigned char BYTE;
typedef char CHAR;
void platform_log(const char *format, ...) {
    va_list ap; va_start(ap, format); vfprintf(stderr, format, ap); va_end(ap); fputc('\n', stderr);
}
#include "xiso.c"
#include "gamepkg_bink_ffmpeg.inc"
static int enabled = 1;
static double volume = 1;
int config_boolean(const char *name) { return !strcmp(name, "audio.enabled") && enabled; }
double config_real(const char *name) { (void)name; return volume; }
const char *config_string(const char *name) { (void)name; return "default"; }
SDL_AudioDeviceID platform_audio_device(int recording, const char *name) {
    (void)recording; (void)name; return SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK;
}
static unsigned saved_mxcsr;
static fpu_control_t saved_cw;
static int rounding;
static void fp_check(void) {
    fpu_control_t cw; _FPU_GETCW(cw);
    assert(cw == saved_cw && _mm_getcsr() == saved_mxcsr);
    assert(fegetround() == rounding && fetestexcept(FE_ALL_EXCEPT) == FE_DIVBYZERO);
}
static void fp_set(int mode) {
    rounding = mode; assert(!fesetround(mode)); feclearexcept(FE_ALL_EXCEPT); feraiseexcept(FE_DIVBYZERO);
    _FPU_GETCW(saved_cw); saved_cw = (saved_cw & ~_FPU_EXTENDED) | _FPU_SINGLE; _FPU_SETCW(saved_cw);
    _mm_setcsr(_mm_getcsr() | 0x8040); saved_mxcsr = _mm_getcsr();
}
static HBINK open_movie(void) { HBINK m = BinkOpen("d:\\bink\\intro.bik", 0); fp_check(); return m; }
static void close_movie(HBINK m) { BinkClose(m); fp_check(); }
static void pixels(HBINK m, unsigned frame) {
    enum { PITCH = 84, HEIGHT = 20, X = 2, Y = 1, GUARD = 32 };
    uint8_t buffer[GUARD + PITCH * HEIGHT + GUARD]; memset(buffer, 0xa5, sizeof(buffer));
    assert(!BinkCopyToBuffer(m, buffer + GUARD, PITCH, HEIGHT, X, Y, 0x80000003)); fp_check();
    for (unsigned at = 0; at < sizeof(buffer); ++at) {
        unsigned rel = at - GUARD, row = rel / PITCH, col = rel % PITCH;
        uint8_t expected = 0xa5;
        if (at >= GUARD && row >= Y && row < Y + 16 && col >= X * 4 && col < (X + 16) * 4)
            expected = (col % 4 == 3 || frame % 3 == 1) ? 255 : 0;
        assert(buffer[at] == expected);
    }
}
int main(int argc, char **argv) {
    assert(argc == 3); const char *scenario = argv[2];
    setenv("HALO_DATA_ROOT", "/gamepkg-generated-movie", 1); setenv("SDL_AUDIO_DRIVER", "dummy", 1);
    char error[1024]; assert(xiso_view_mount(argv[1], "/gamepkg-generated-movie", error, sizeof(error)));
    assert(SDL_Init(SDL_INIT_AUDIO)); assert(!strcmp(SDL_GetCurrentAudioDriver(), "dummy"));
    BinkSetSoundSystem(BinkOpenDirectSound, 1);
    fp_set(FE_DOWNWARD);
    if (!strcmp(scenario, "invalid-media")) {
        HBINK m = open_movie();
        if (m) {
            for (unsigned i = 0; i < 4 && !BinkHasError(m); ++i) { BinkDoFrame(m); fp_check(); BinkNextFrame(m); }
            assert(BinkHasError(m)); close_movie(m);
        }
        puts("PASS invalid media rejected or failed promptly"); SDL_Quit(); return 0;
    }
    const char *invalid[] = { NULL, "", "https://example.invalid/movie", "d:/bink/../maps/ui.map",
        "d:/bink/intro.bik:stream", "d:/bink/./intro.bik", "d:/bink/intro.bik/child", "d:/maps/ui.map" };
    for (unsigned i = 0; i < sizeof(invalid)/sizeof(invalid[0]); ++i) { assert(!BinkOpen(invalid[i], 0)); fp_check(); }
    char unterminated[512]; memset(unterminated, 'x', sizeof(unterminated));
    assert(!BinkOpen(unterminated, 0)); fp_check();
    int modes[] = { FE_TONEAREST, FE_DOWNWARD, FE_UPWARD, FE_TOWARDZERO };
    double gains[] = { 1, 0, .5, -1, 2, NAN };
    for (unsigned test = 0; test < 8; ++test) {
        fp_set(modes[test % 4]); enabled = test != 6; volume = gains[test % 6];
        if (test == 7) SDL_QuitSubSystem(SDL_INIT_AUDIO);
        HBINK m = open_movie(); assert(m && m->Width == 16 && m->Height == 16 && m->Frames == 3);
        assert(!m->started && (!m->output || SDL_AudioStreamDevicePaused(m->output)));
        unsigned loops = test == 0 ? 2 : 1;
        uint64_t began = SDL_GetTicksNS();
        for (unsigned frame = 0; frame < 3 * loops; ++frame) {
            while (BinkWait(m)) fp_check(); fp_check();
            assert(!BinkDoFrame(m)); fp_check(); assert(!BinkHasError(m));
            uint64_t count = m->decoded; assert(!BinkDoFrame(m) && m->decoded == count); fp_check();
            pixels(m, frame); BinkNextFrame(m); fp_check(); BinkPresented(m); fp_check();
            if (m->output) {
                double expected = isfinite(volume) ? fmin(1, fmax(0, volume)) : 0;
                assert(SDL_GetAudioStreamGain(m->output) == (float)expected);
            }
        }
        while (BinkWait(m)) fp_check(); fp_check();
        uint64_t elapsed = SDL_GetTicksNS() - began;
        uint64_t expected_ns = av_rescale((int64_t)3 * loops, INT64_C(1000000000) * m->fps.den, m->fps.num);
        assert(elapsed >= expected_ns && elapsed - expected_ns < 200000000);
        if (m->audio >= 0) assert(m->total_audio_samples == (uint64_t)av_rescale_rnd(3 * loops, (int64_t)m->rate * m->fps.den, m->fps.num, AV_ROUND_DOWN));
        if (!enabled || test == 7) assert(!m->output);
        assert(m->decoded == 3 * loops && m->presentations == 3 * loops);
        close_movie(m);
    }
    /* Failed copies must leave the entire destination untouched. */
    for (unsigned test = 0; test < 7; ++test) {
        HBINK m = open_movie(); assert(m && !BinkDoFrame(m)); fp_check();
        uint8_t buffer[4096]; memset(buffer, 0xa5, sizeof(buffer));
        void *dst = test == 0 ? NULL : buffer;
        long pitch = test == 1 ? -64 : test == 2 ? 63 : 64;
        unsigned long height = test == 3 ? 15 : test == 6 ? ULONG_MAX : 16;
        unsigned long x = test == 4 ? ULONG_MAX : 0, y = test == 5 ? ULONG_MAX : 0;
        assert(BinkCopyToBuffer(m, dst, pitch, height, x, y, 3)); fp_check(); assert(BinkHasError(m));
        for (unsigned i = 0; i < sizeof(buffer); ++i) assert(buffer[i] == 0xa5);
        close_movie(m);
    }
    /* Skip early, then reopen: no decoder/audio state may survive. */
    for (unsigned i = 0; i < 8; ++i) { HBINK m = open_movie(); assert(m && !BinkDoFrame(m)); fp_check(); close_movie(m); }
    puts("PASS synthetic pixels/stride/guards, loops/clock/audio, FP, paths, bad copies, skip/reopen");
    SDL_Quit(); return 0;
}
