/* Maintainer-only Linux SDL interposition: bounded format/put metadata, no PCM. */
#define _GNU_SOURCE
#include <SDL.h>
#include <dlfcn.h>
#include <stdatomic.h>
#include <stdio.h>
static atomic_uint traced_put_count;
SDL_AudioStream *SDL_NewAudioStream(SDL_AudioFormat source, Uint8 channels, int rate,
                                  SDL_AudioFormat target, Uint8 target_channels, int target_rate) {
    SDL_AudioStream *(*next)(SDL_AudioFormat, Uint8, int, SDL_AudioFormat, Uint8, int) =
        dlsym(RTLD_NEXT, "SDL_NewAudioStream");
    if (!next) return NULL;
    SDL_AudioStream *stream = next(source, channels, rate, target, target_channels, target_rate);
    fprintf(stderr, "TRACE_FORMAT %u %u %d %u %u %d\n", source, channels, rate,
            target, target_channels, target_rate);
    return stream;
}
int SDL_AudioStreamPut(SDL_AudioStream *stream, const void *bytes, int length) {
    int (*next)(SDL_AudioStream *, const void *, int) = dlsym(RTLD_NEXT, "SDL_AudioStreamPut");
    if (!next) return -1;
    if (atomic_fetch_add(&traced_put_count, 1) < 16) fprintf(stderr, "TRACE_PUT %d\n", length);
    return next(stream, bytes, length);
}
