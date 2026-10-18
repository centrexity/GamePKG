/* Maintainer oracle. Every signed 16-bit input, all legal SDL volume levels,
 * and seven destination values spanning both clipping boundaries. */
#include <SDL.h>
#include <stdio.h>
int main(void) {
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
