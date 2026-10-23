#ifndef GAMEPKG_WINDOWS_DISC_STREAM_H
#define GAMEPKG_WINDOWS_DISC_STREAM_H
/* Host-owned native startup/SDL transport. No CRT FILE object crosses DLLs. */
#include <stddef.h>
#include <stdint.h>
#include <SDL.h>
#ifdef __cplusplus
extern "C" {
#endif
int gpw_load_xbe(const char *, void **, size_t *);
int64_t gpw_file_size(const char *);
SDL_RWops *gpw_SDL_RWFromFile(const char *, const char *);
#ifdef __cplusplus
}
#endif
#endif
