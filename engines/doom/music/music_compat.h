/* GamePKG compatibility for the pinned GPL-2.0-or-later music sources. */
#ifndef GAMEPKG_MUSIC_COMPAT_H
#define GAMEPKG_MUSIC_COMPAT_H
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include "doomtype.h"
#include "memio.h"
#include "upstream/midifile.h"
#define PACKED_STRUCT(body) struct __attribute__((packed)) body
#if __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
#define SDL_SwapBE16(value) __builtin_bswap16((uint16_t)(value))
#define SDL_SwapBE32(value) __builtin_bswap32((uint32_t)(value))
#else
#define SDL_SwapBE16(value) ((uint16_t)(value))
#define SDL_SwapBE32(value) ((uint32_t)(value))
#endif
typedef enum { opl_doom1_1_666, opl_doom2_1_666, opl_doom_1_9 } opl_driver_ver_t;
static inline int gamepkg_midi_getc(MEMFILE *stream) {
    uint8_t value;
    return mem_fread(&value, 1, 1, stream) == 1 ? value : EOF;
}
midi_file_t *MIDI_LoadMemory(void *bytes, size_t length);
void gamepkg_music_render(int16_t *samples, unsigned frames);
#endif
