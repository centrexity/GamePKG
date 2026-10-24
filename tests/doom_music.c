/* Production music path tests: generated instruments/MIDI, no game media. */
#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <setjmp.h>
#include "../engines/doom/music/music_compat.h"
#include "i_sound.h"
#include "../engines/doom/music/upstream/opl.h"
#ifdef GAMEPKG_OPL_TEST
unsigned gamepkg_opl_pending_callbacks(void);
#endif
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*) == 4, "actual 32-bit process required");
#endif
static unsigned char instruments[11908];
static jmp_buf expected_failure;
static int expect_failure;
void *Z_Malloc(int size, int tag, void *owner) {
    (void)tag; (void)owner; assert(size > 0 && size <= 1024 * 1024);
    void *result = malloc((size_t)size); assert(result); return result;
}
void Z_Free(void *ptr) { free(ptr); }
void I_Error(char *message, ...) {
    (void)message;
    if (expect_failure) longjmp(expected_failure, 1);
    abort();
}
int W_CheckNumForName(char *name) { return strcmp(name, "GENMIDI") ? -1 : 0; }
int W_LumpLength(unsigned index) { assert(index == 0); return sizeof(instruments); }
void *W_CacheLumpNum(int index, int tag) { (void)tag; assert(index == 0); return instruments; }
void W_ReleaseLumpName(char *name) { (void)name; }
static uint64_t render(unsigned ticks) {
    uint64_t energy = 0; int16_t pcm[1600];
    for (unsigned i = 0; i < ticks; i++) {
        gamepkg_music_render(pcm, 800);
        for (unsigned s = 0; s < 1600; s++) energy += (uint64_t)((int64_t)pcm[s] * pcm[s]);
    }
    return energy;
}
static unsigned callbacks;
static void timer(void *data) { assert(data == &callbacks); callbacks++; }
int main(void) {
    assert(render(2)==0); // Disabled/uninitialized backend must be safe silence.
    memcpy(instruments, "#OPL_II#", 8);
    const unsigned char voice[16] = {1,0xf2,0x73,0,0,32,0,1,0xf2,0x73,0,0,0,0,0,0};
    for (unsigned i = 0; i < 175; i++) {
        unsigned char *instrument = instruments + 8 + i * 36;
        instrument[2] = 128;
        memcpy(instrument + 4, voice, 16); memcpy(instrument + 20, voice, 16);
    }
    unsigned char midi[] = {
        'M','T','h','d',0,0,0,6,0,0,0,1,0,96,
        'M','T','r','k',0,0,0,15,
        0,0xc0,0,0,0x90,60,100,96,0x80,60,0,0,0xff,0x2f,0
    };
    assert(DG_music_module.Init());
    DG_music_module.SetMusicVolume(127);
    void *song = DG_music_module.RegisterSong(midi, sizeof(midi)); assert(song);
    DG_music_module.PlaySong(song, true);
    uint64_t loud = render(30); assert(loud > 0);
    DG_music_module.SetMusicVolume(0);
    render(60); assert(render(10) * 100 < loud);
    DG_music_module.SetMusicVolume(127);
    assert(render(90));
    DG_music_module.PauseMusic();
    OPL_SetCallback(10000, timer, &callbacks);
    render(10); assert(callbacks == 0);
    DG_music_module.ResumeMusic();
    render(2); assert(callbacks == 1);
    DG_music_module.StopSong();
    assert(!DG_music_module.MusicIsPlaying());
#ifdef GAMEPKG_OPL_TEST
    assert(gamepkg_opl_pending_callbacks() == 0);
    DG_music_module.PlaySong(song, false);
    unsigned completed_ticks = 0;
    uint64_t complete_energy = 0;
    while (gamepkg_opl_pending_callbacks()) {
        assert(completed_ticks++ < 180);
        complete_energy += render(1);
    }
    assert(completed_ticks > 0 && complete_energy > 0);
    DG_music_module.StopSong();
    DG_music_module.PlaySong(song, true);
    assert(render(completed_ticks + 180) > 0);
    assert(gamepkg_opl_pending_callbacks() > 0);
    DG_music_module.StopSong();
    assert(gamepkg_opl_pending_callbacks() == 0);
#endif
    DG_music_module.UnRegisterSong(song);
#ifdef GAMEPKG_OPL_TEST
    /* Match the game's stop/unregister/replace order, including paused tracks. */
    for (unsigned replacement = 0; replacement < 32; replacement++) {
        midi[27] = midi[31] = (unsigned char)(40 + replacement);
        song = DG_music_module.RegisterSong(midi, sizeof(midi)); assert(song);
        DG_music_module.PlaySong(song, true);
        assert(render(4) > 0);
        DG_music_module.PauseMusic();
        render(2);
        DG_music_module.StopSong();
        assert(gamepkg_opl_pending_callbacks() == 0);
        DG_music_module.UnRegisterSong(song);
        if (replacement % 8 == 7) {
            DG_music_module.Shutdown();
            assert(gamepkg_opl_pending_callbacks() == 0);
            assert(!DG_music_module.MusicIsPlaying());
            assert(DG_music_module.Init());
            DG_music_module.SetMusicVolume(127);
        }
    }
    midi[27] = midi[31] = 60;
#endif
    unsigned char mus[] = {
        'M','U','S',26,8,0,16,0,1,0,0,0,0,0,0,0,
        0x90,0xbc,100,70,0x80,60,1,0x60
    };
    song = DG_music_module.RegisterSong(mus, sizeof(mus)); assert(song);
    DG_music_module.PlaySong(song, true);
    assert(render(30));
    DG_music_module.StopSong(); DG_music_module.UnRegisterSong(song);
    for (size_t n = 0; n < sizeof(mus); n++) {
        assert(!DG_music_module.RegisterSong(mus, n));
    }
    /* Every truncated header and invalid channel payload must fail. */
    for (size_t n = 0; n < sizeof(midi); n++) assert(!MIDI_LoadMemory(midi, n));
    midi[12] = midi[13] = 0; assert(!MIDI_LoadMemory(midi, sizeof(midi)));
    midi[13] = 96; midi[28] = 255; assert(!MIDI_LoadMemory(midi, sizeof(midi)));
    midi[28] = 100; midi[21] = 250; assert(!MIDI_LoadMemory(midi, sizeof(midi)));
    midi[21] = 15;
    for (size_t byte = 0; byte < sizeof(midi); byte++) {
        unsigned char original = midi[byte];
        for (unsigned value = 0; value < 256; value++) {
            midi[byte] = value;
            midi_file_t *candidate = MIDI_LoadMemory(midi, sizeof(midi));
            if (candidate) MIDI_FreeFile(candidate);
        }
        midi[byte] = original;
    }
    unsigned char invalid_mus[16] = {'M','U','S',26};
    assert(!DG_music_module.RegisterSong(invalid_mus, sizeof(invalid_mus)));
    DG_music_module.Shutdown();
    assert(OPL_Init(0) == OPL_INIT_OPL3);
    callbacks = 0;
    OPL_SetCallback(20000, timer, &callbacks);
    OPL_AdjustCallbacks(2.0f);
    render(1); assert(callbacks == 1);
    expect_failure = 1;
    if (setjmp(expected_failure) == 0) {
        for (unsigned i = 0; i < 129; i++) OPL_SetCallback(1, timer, &callbacks);
        assert(!"callback limit was not enforced");
    }
    expect_failure = 0;
    OPL_Shutdown();
    assert(render(2)==0); // Shutdown must not render stale chip state.
    return 0;
}
