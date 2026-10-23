#ifndef GAMEPKG_HOST_H
#define GAMEPKG_HOST_H
/*
 * Stable C ABI for a native engine. The operations and their numbers are the
 * host imports in wit/game.wit. Core Wasm wrappers in the runtime use the
 * same success and failure values:
 *   draw, audio, save write: 0 success, -1 failure
 *   achievement: 1 recorded, 0 unknown index, -1 unbound
 *   window control: 1 accepted, 0 rejected or unbound
 *   file, input, rom, asset, save read: byte count, or -1
 *   network: payload byte count, -2 busy, -3 failed, -1 rejected
 * Frame time is the sample taken by gamepkg_host_begin_frame. The engine
 * does not sample the clock itself.
 * gamepkg_host_log is the one addition. Guests had no log import.
 */
#include <stdint.h>

#define GAMEPKG_HOST_ABI 1

#define GAMEPKG_FILE_IMPORT 0u
#define GAMEPKG_FILE_EXPORT 1u

#define GAMEPKG_WINDOW_VISIBLE 0u
#define GAMEPKG_WINDOW_CONFINED 1u
#define GAMEPKG_WINDOW_FULLSCREEN 2u
#define GAMEPKG_WINDOW_CLOSE 3u
#define GAMEPKG_WINDOW_HOMEPAGE 4u

#ifdef __cplusplus
extern "C" {
#endif

int32_t gamepkg_effect_call(uint32_t operation, const char *path, uint32_t value);
uint64_t gamepkg_frame_time_us(void);
uint32_t gamepkg_window_state(void);
int32_t gamepkg_window_control(uint32_t operation, uint32_t value);
int32_t gamepkg_file_call(uint32_t operation, const uint8_t *request, uint32_t request_len,
    uint8_t *out, uint32_t capacity);
int32_t gamepkg_input_events(uint8_t *out, uint32_t capacity);
uint64_t gamepkg_random_u64(void);
int32_t gamepkg_network_call(uint32_t operation, const uint8_t *request, uint32_t request_len,
    uint8_t *out, uint32_t capacity);
uint32_t gamepkg_rom_count(void);
int32_t gamepkg_rom_size_at(uint32_t index);
int32_t gamepkg_rom_read_at(uint32_t index, uint32_t offset, uint8_t *out, uint32_t length);
int32_t gamepkg_achievement_unlock(uint32_t index);
int32_t gamepkg_asset_read(const char *path, uint32_t offset, uint8_t *out, uint32_t length);
int32_t gamepkg_rom_size(void);
int32_t gamepkg_rom_read(uint32_t offset, uint8_t *out, uint32_t length);
int32_t gamepkg_save_size(void);
int32_t gamepkg_save_read(uint32_t offset, uint8_t *out, uint32_t length);
int32_t gamepkg_save_write(const uint8_t *bytes, uint32_t length);
int32_t gamepkg_draw_clear(uint32_t rgba);
int32_t gamepkg_draw_clip(float x, float y, float width, float height);
int32_t gamepkg_draw_clip_reset(void);
int32_t gamepkg_draw_rect(float x, float y, float width, float height, uint32_t rgba);
int32_t gamepkg_draw_circle(float x, float y, float radius, uint32_t rgba);
/* RGBA8 pixels, width * height * 4 bytes. Same checks as the Wasm draw-image imports. */
int32_t gamepkg_draw_image(float x, float y, float width, float height, uint32_t pixel_width,
    uint32_t pixel_height, const uint8_t *pixels, uint32_t length);
int32_t gamepkg_draw_image_linear(float x, float y, float width, float height,
    uint32_t pixel_width, uint32_t pixel_height, const uint8_t *pixels, uint32_t length);
int32_t gamepkg_audio_submit(const int16_t *samples, uint32_t count);
int32_t gamepkg_audio_submit_stereo(const int16_t *samples, uint32_t count);

/* Bounded UTF-8 line, at most 1024 bytes, no NUL inside the length. */
int32_t gamepkg_host_log(const char *text, uint32_t length);
/* Host tick hook. Not for the engine. */
void gamepkg_host_begin_frame(void);

#ifdef __cplusplus
}
#endif

#endif
