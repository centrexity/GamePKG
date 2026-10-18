/* Maintainer oracle. Every signed 16-bit input, all legal SDL volume levels,
 * and seven destination values spanning both clipping boundaries. */
#include <SDL.h>
#include <stdio.h>
static int stream_reference(void) {
    const int volumes[] = {0, 1, 63, 64, 127, 128};
    const int lengths[] = {1025, 2051, 4096, 777};
    Uint16 music[6000 * 2], effect[4096 * 2];
    for (unsigned v = 0; v < sizeof(volumes)/sizeof(volumes[0]); ++v) {
        for (int i = 0; i < 6000; ++i)
            for (int c = 0; c < 2; ++c)
                music[2*i+c] = SDL_SwapLE16((Uint16)((i*7919+c*31337)%65536-32768));
        for (int slot = 0; slot < 4; ++slot) {
            for (int i = 0; i < lengths[slot]; ++i)
                for (int c = 0; c < 2; ++c)
                    effect[2*i+c] = SDL_SwapLE16((Uint16)((i*(slot*1543+197)+c*12345+slot*313)%65536-32768));
            SDL_MixAudioFormat((Uint8 *)music, (const Uint8 *)effect,
                              AUDIO_S16LSB, lengths[slot]*4, volumes[v]);
        }
        SDL_AudioCVT cvt;
        if (SDL_BuildAudioCVT(&cvt, AUDIO_S16LSB, 2, 44100, AUDIO_S16LSB, 2, 48000) < 0) return 2;
        cvt.len = sizeof(music);
        cvt.buf = SDL_malloc((size_t)cvt.len * cvt.len_mult);
        if (!cvt.buf) return 3;
        SDL_memcpy(cvt.buf, music, sizeof(music));
        if (SDL_ConvertAudio(&cvt)) return 4;
        Uint32 length = SDL_SwapLE32((Uint32)cvt.len_cvt);
        if (fwrite(&length, 1, 4, stdout) != 4 ||
            fwrite(cvt.buf, 1, cvt.len_cvt, stdout) != (size_t)cvt.len_cvt) return 5;
        SDL_free(cvt.buf);
    }
    return 0;
}
int main(int argc, char **argv) {
    if (argc == 2 && SDL_strcmp(argv[1], "--stream") == 0) return stream_reference();
    if (argc != 1) return 6;
    const Sint16 destinations[] = {-32768, -30000, -1, 0, 1, 30000, 32767};
    Uint16 source[65536], output[65536];
    for (int sample = 0; sample < 65536; ++sample)
        source[sample] = SDL_SwapLE16((Uint16)(sample - 32768));
    for (int volume = 0; volume <= 128; ++volume) {
        for (unsigned d = 0; d < sizeof(destinations)/sizeof(destinations[0]); ++d) {
            for (int sample = 0; sample < 65536; ++sample)
                output[sample] = SDL_SwapLE16((Uint16)destinations[d]);
            SDL_MixAudioFormat((Uint8 *)output, (const Uint8 *)source,
                              AUDIO_S16LSB, sizeof(output), volume);
            if (fwrite(output, 1, sizeof(output), stdout) != sizeof(output)) return 1;
        }
    }
    return 0;
}
