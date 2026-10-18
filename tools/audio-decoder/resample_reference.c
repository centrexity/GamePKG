/* Maintainer-only independent SDL float conversion oracle. No audio device. */
#include <SDL.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    if (argc != 4 && argc != 5) return 1;
    FILE *input = fopen(argv[1], "rb");
    if (!input) return 2;
    Uint8 bytes[65536];
    size_t length = fread(bytes, 1, sizeof(bytes), input);
    if (!length || length % 8 || fgetc(input) != EOF || ferror(input)) return 3;
    fclose(input);
    if (argc == 5) {
        SDL_AudioStream *stream = SDL_NewAudioStream(AUDIO_F32LSB, 2, atoi(argv[2]), AUDIO_F32LSB, 2, atoi(argv[3]));
        if (!stream) return 9;
        Uint8 output[65536];
        size_t at = 0;
        for (;;) {
            if (at < length) {
                size_t n = length - at;
                if (n > 1024 * 8) n = 1024 * 8;
                if (SDL_AudioStreamPut(stream, bytes + at, (int)n)) return 10;
                at += n;
            } else if (SDL_AudioStreamFlush(stream)) return 11;
            int n;
            while ((n = SDL_AudioStreamGet(stream, output, sizeof(output))) > 0)
                if (fwrite(output, 1, (size_t)n, stdout) != (size_t)n) return 12;
            if (n < 0) return 13;
            if (at == length) {
                if (SDL_AudioStreamFlush(stream)) return 14;
                while ((n = SDL_AudioStreamGet(stream, output, sizeof(output))) > 0)
                    if (fwrite(output, 1, (size_t)n, stdout) != (size_t)n) return 15;
                if (n < 0) return 16;
                break;
            }
        }
        SDL_FreeAudioStream(stream);
        return 0;
    }
    SDL_AudioCVT cvt;
    if (SDL_BuildAudioCVT(&cvt, AUDIO_F32LSB, 2, atoi(argv[2]), AUDIO_F32LSB, 2, atoi(argv[3])) < 0) return 4;
    if (cvt.len_mult <= 0 || cvt.len_mult > 64) return 5;
    cvt.len = (int)length;
    cvt.buf = SDL_malloc(length * (size_t)cvt.len_mult);
    if (!cvt.buf) return 6;
    memcpy(cvt.buf, bytes, length);
    if (SDL_ConvertAudio(&cvt) || cvt.len_cvt < 0 || cvt.len_cvt % 8) return 7;
    int ok = fwrite(cvt.buf, 1, (size_t)cvt.len_cvt, stdout) == (size_t)cvt.len_cvt;
    SDL_free(cvt.buf);
    return ok ? 0 : 8;
}
