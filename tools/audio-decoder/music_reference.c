/* Maintainer oracle for the original streamed Music API, not Mix_LoadWAV.
 * The disk driver runs callbacks without real-time delays; no audio device is used.
 * Output includes SDL's final callback padding. Never publish captured recordings.
 */
#include <SDL.h>
#include <SDL_mixer.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>

static atomic_int finished, captured_end, failed;
static size_t written;
static void ended(void) { atomic_store(&finished, 1); }
static void capture(void *unused, Uint8 *bytes, int length) {
    (void)unused;
    if (atomic_load(&captured_end)) return;
    if (length < 0 || (size_t)length > 64 * 1024 * 1024 - written) {
        atomic_store(&failed, 1);
        return;
    }
    if (fwrite(bytes, 1, (size_t)length, stdout) != (size_t)length)
        atomic_store(&failed, 1);
    written += (size_t)length;
    if (atomic_load(&finished)) atomic_store(&captured_end, 1);
}
int main(int argc, char **argv) {
    if (argc != 4) return 1;
    int rate = atoi(argv[2]);
    if (rate != 44100 && rate != 48000) return 2;
    SDL_setenv("SDL_AUDIODRIVER", "disk", 1);
    SDL_setenv("SDL_DISKAUDIOFILE", argv[3], 1);
    SDL_setenv("SDL_DISKAUDIODELAY", "0", 1);
    if (SDL_Init(SDL_INIT_AUDIO) || Mix_OpenAudio(rate, AUDIO_S16LSB, 2, 1024)) {
        fprintf(stderr, "SDL music reference: %s\n", SDL_GetError());
        return 3;
    }
    Mix_PauseAudio(1);
    int actual, channels;
    Uint16 format;
    if (!Mix_QuerySpec(&actual, &format, &channels) || actual != rate || channels != 2 || format != AUDIO_S16LSB) return 4;
    Mix_Music *music = Mix_LoadMUS(argv[1]);
    if (!music) return 5;
    Mix_VolumeMusic(MIX_MAX_VOLUME);
    Mix_SetPostMix(capture, NULL);
    Mix_HookMusicFinished(ended);
    /* Match the original game's play(1) argument, rather than its comment. */
    if (Mix_PlayMusic(music, 1)) return 6;
    Uint64 start = SDL_GetTicks64();
    Mix_PauseAudio(0);
    while (!atomic_load(&captured_end) && !atomic_load(&failed) && SDL_GetTicks64() - start < 30000)
        SDL_Delay(1);
    Mix_PauseAudio(1);
    int ok = atomic_load(&captured_end) && !atomic_load(&failed) && written > 0;
    Mix_SetPostMix(NULL, NULL);
    Mix_HookMusicFinished(NULL);
    Mix_HaltMusic();
    Mix_FreeMusic(music);
    Mix_CloseAudio();
    SDL_Quit();
    return ok ? 0 : 7;
}
