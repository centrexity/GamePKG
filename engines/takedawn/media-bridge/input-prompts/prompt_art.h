#ifndef GAMEPKG_PROMPT_ART_H
#define GAMEPKG_PROMPT_ART_H
#include <stdint.h>
struct gamepkg_prompt_rle { uint16_t width, height; uint32_t count; const uint32_t *runs; };
#ifdef __cplusplus
extern "C" {
#endif
extern const struct gamepkg_prompt_rle *const gamepkg_kenney_art[30][4][4];
/* Matches only an exact registered prompt image; returns 0 for other artwork. */
int gamepkg_prompt_scale(const uint32_t *src, uint32_t w, uint32_t h, unsigned factor, uint32_t *dst);
int gamepkg_prompt_sheet(int set, uint32_t *dst);
#ifdef __cplusplus
}
#endif
#endif
