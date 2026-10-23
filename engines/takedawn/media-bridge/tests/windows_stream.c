/* Synthetic native SDL/startup test. Reuse the complete file API oracle and
 * deterministic reader provider, without an ISO or extracted game files. */
#define GPW_STREAM_TEST
#include "windows_file.c"
#include "../windows_stream.h"
#include <stdlib.h>

int main(void) {
    assert(gpw_test_file_oracle() == 0);
    assert(gpw_file_size("/gamepkg-disc/Data/pattern.bin") == (int64_t)small_size);
    assert(gpw_file_size("/gamepkg-disc/Data/large.bin") == (int64_t)large_size);
    assert(gpw_file_size("/gamepkg-disc/Data") == -1 && GetLastError() == ERROR_ACCESS_DENIED);
    void *loaded = NULL; size_t loaded_size = 0;
    assert(gpw_load_xbe("/gamepkg-disc/Data/pattern.bin", &loaded, &loaded_size));
    assert(loaded && loaded_size == small_size);
    for (size_t i = 0; i < loaded_size; i++) assert(((uint8_t *)loaded)[i] == pattern(i));
    free(loaded);
    loaded = (void *)(uintptr_t)123; loaded_size = 456;
    corrupt = 1;
    assert(!gpw_load_xbe("/gamepkg-disc/Data/pattern.bin", &loaded, &loaded_size));
    assert(GetLastError() == ERROR_READ_FAULT && loaded == (void *)(uintptr_t)123 && loaded_size == 456 && !live);
    corrupt = 0;
    assert(!gpw_load_xbe("/gamepkg-disc/../escape.bin", &loaded, &loaded_size) && GetLastError() == ERROR_ACCESS_DENIED);
    assert(!gpw_load_xbe("/gamepkg-disc/Data/pattern.bin", NULL, &loaded_size) && GetLastError() == ERROR_INVALID_PARAMETER);
#if SIZE_MAX == UINT32_MAX
    assert(!gpw_load_xbe("/gamepkg-disc/Data/large.bin", &loaded, &loaded_size) && GetLastError() == ERROR_FILE_TOO_LARGE);
#endif
    SDL_RWops *rw = gpw_SDL_RWFromFile("/gamepkg-disc/Data/pattern.bin", "rb");
    assert(rw && SDL_RWsize(rw) == (Sint64)small_size && SDL_RWtell(rw) == 0);
    uint8_t buffer[150000]; memset(buffer, 0xA5, sizeof buffer);
    assert(SDL_RWread(rw, buffer, 7, 2) == 2 && SDL_RWtell(rw) == 14);
    for (unsigned i = 0; i < 14; i++) assert(buffer[i] == pattern(i));
    assert(SDL_RWseek(rw, -4, RW_SEEK_END) == (Sint64)small_size - 4);
    assert(SDL_RWread(rw, buffer, 3, 2) == 1 && SDL_RWtell(rw) == (Sint64)small_size);
    for (unsigned i = 0; i < 4; i++) assert(buffer[i] == pattern(small_size - 4 + i));
    assert(SDL_RWread(rw, buffer, 1, 1) == 0);
    assert(SDL_RWseek(rw, 19, RW_SEEK_END) == (Sint64)small_size + 19);
    assert(SDL_RWread(rw, buffer, 1, 1) == 0 && SDL_RWtell(rw) == (Sint64)small_size + 19);
    assert(SDL_RWseek(rw, 0, RW_SEEK_SET) == 0);
    assert(SDL_RWread(rw, NULL, 0, SIZE_MAX) == 0 && SDL_RWtell(rw) == 0);
    assert(SDL_RWread(rw, buffer, 3, SIZE_MAX) == 0 && SDL_RWtell(rw) == 0);
    assert(SDL_RWread(rw, NULL, 1, 1) == 0 && SDL_RWtell(rw) == 0);
    assert(SDL_RWseek(rw, -1, RW_SEEK_SET) == -1 && SDL_RWtell(rw) == 0);
    assert(SDL_RWseek(rw, 1, 123) == -1 && SDL_RWtell(rw) == 0);
    assert(SDL_RWwrite(rw, buffer, 1, 1) == 0 && SDL_RWtell(rw) == 0);
    corrupt = 1; memset(buffer, 0xA5, sizeof buffer);
    assert(SDL_RWread(rw, buffer, 1, 10) == 0 && SDL_RWtell(rw) == 0);
    for (unsigned i = 0; i < 10; i++) assert(buffer[i] == 0xA5);
    corrupt = 0;
    assert(SDL_RWread(rw, buffer, 1, sizeof buffer) == small_size);
    for (size_t i = 0; i < small_size; i++) assert(buffer[i] == pattern(i));
    assert(SDL_RWclose(rw) == 0 && !live);
    rw = gpw_SDL_RWFromFile("/gamepkg-disc/Data/large.bin", "rb");
    assert(rw && SDL_RWsize(rw) == (Sint64)large_size);
    assert(SDL_RWseek(rw, INT64_C(0x100000005), RW_SEEK_SET) == INT64_C(0x100000005));
    assert(SDL_RWread(rw, buffer, 1, 17) == 17);
    for (unsigned i = 0; i < 17; i++) assert(buffer[i] == pattern(UINT64_C(0x100000005) + i));
    assert(SDL_RWclose(rw) == 0);
    assert(!gpw_SDL_RWFromFile("/gamepkg-disc/Data/pattern.bin", "rb+"));
    assert(!gpw_SDL_RWFromFile("/gamepkg-disc/Data/pattern.bin", "w"));
    assert(!gpw_SDL_RWFromFile("/gamepkg-disc/Data", "rb"));
    assert(!gpw_SDL_RWFromFile("/gamepkg-disc/../escape.bin", "rb"));
    assert(!gpw_SDL_RWFromFile(NULL, "rb"));
    assert(!gpw_SDL_RWFromFile("/gamepkg-disc/Data/pattern.bin", NULL));
    rw = gpw_SDL_RWFromFile("native-sdl-oracle.bin", "wb");
    assert(rw && SDL_RWwrite(rw, "native", 2, 3) == 3 && SDL_RWclose(rw) == 0);
    rw = gpw_SDL_RWFromFile("native-sdl-oracle.bin", "rb");
    assert(rw && SDL_RWsize(rw) == 6 && SDL_RWread(rw, buffer, 2, 3) == 3 && !memcmp(buffer, "native", 6));
    assert(SDL_RWclose(rw) == 0 && DeleteFileW(L"native-sdl-oracle.bin"));
    assert(!live);
    puts("PASS: native Windows startup and SDL ISO streams; full reads, partial objects, 64-bit seeks, EOF, overflow/write rejection, unchanged failure outputs and cleanup");
    return 0;
}
