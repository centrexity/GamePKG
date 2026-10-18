/* Maintainer-only independent SDL float conversion oracle. No audio device. */
#include <SDL.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    if (argc != 4) return 1;
    FILE *input = fopen(argv[1], "rb");
    if (!input) return 2;
    Uint8 bytes[65536];
    size_t length = fread(bytes, 1, sizeof(bytes), input);
    if (!length || length % 8 || fgetc(input) != EOF || ferror(input)) return 3;
    fclose(input);
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
