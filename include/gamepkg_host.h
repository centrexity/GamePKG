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
 * Frame time is the sample taken by gamepkg_host_begin_frame. It is not a
 * free-running clock.
 * gamepkg_host_log, the free-running clocks, the thread calls, and the
 * directory calls are host additions. Guests had no log import, no
 * free-running clock, no thread import, and no directory import.
 * gamepkg_file_call stays the player-picked import and export. The directory
 * calls read a host path the process already has. The engine keeps its own
 * mutexes and completion handles.
 */
#include <stdint.h>

#define GAMEPKG_HOST_ABI 3

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
/* Nanoseconds from a monotonic clock. Zero if the clock cannot be read.
   Independent of gamepkg_frame_time_us. */
uint64_t gamepkg_monotonic_time_ns(void);
/* Unix time. Null pointers are ignored. Both outputs are zero if the clock
   cannot be read. */
void gamepkg_wall_time(uint64_t *seconds, uint32_t *nanoseconds);
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

/* Detached thread. start is called on the new thread with context.
   Returns 1, or 0 when start is null or the thread cannot be created.
   stack_bytes 0 selects the host default, which is at least 1 MiB. */
typedef void (*gamepkg_thread_start)(void *context);
int32_t gamepkg_thread_spawn(gamepkg_thread_start start, void *context, uint32_t stack_bytes);
/* 0 yields. GAMEPKG_THREAD_SLEEP_INFINITE does not return. */
#define GAMEPKG_THREAD_SLEEP_INFINITE 0xffffffffu
void gamepkg_thread_sleep_ms(uint32_t milliseconds);
void gamepkg_thread_yield(void);

/* Host-path metadata. Not gamepkg_file_call. kind is GAMEPKG_PATH_NONE when
   the path does not exist. flags uses GAMEPKG_PATH_READ_ONLY. Times are Unix
   seconds and nanoseconds; a clock the host cannot read stays zero. The
   struct is 64 bytes on every ABI. Returns 0, or -1 when path or info is null. */
#define GAMEPKG_PATH_NONE 0u
#define GAMEPKG_PATH_FILE 1u
#define GAMEPKG_PATH_DIRECTORY 2u
#define GAMEPKG_PATH_READ_ONLY 1u
typedef struct gamepkg_path_info {
	uint32_t kind;
	uint32_t flags;
	uint64_t size;
	uint64_t modification_seconds;
	uint32_t modification_nanoseconds;
	uint32_t reserved0;
	uint64_t access_seconds;
	uint32_t access_nanoseconds;
	uint32_t reserved1;
	uint64_t creation_seconds;
	uint32_t creation_nanoseconds;
	uint32_t reserved2;
} gamepkg_path_info;
int32_t gamepkg_path_stat(const char *path, gamepkg_path_info *info);
/* Bytes from a host path at offset. Returns the count read, 0 at the end,
   or -1 when the path cannot be read. length 0 returns 0. */
int32_t gamepkg_path_read(const char *path, uint64_t offset, uint8_t *out, uint32_t length);
/* Small integer handle, never a pointer. Returns 1, or 0 when the directory
   cannot be opened or the table is full. "." and ".." are not returned.
   A name that does not fit in capacity, including its trailing NUL, is skipped.
   gamepkg_dir_read returns the length excluding that NUL, 0 at the end, or -1
   for an unknown handle or a null buffer. */
int32_t gamepkg_dir_open(const char *path, uint32_t *handle);
int32_t gamepkg_dir_read(uint32_t handle, uint8_t *name, uint32_t capacity);
void gamepkg_dir_close(uint32_t handle);

/* Bounded UTF-8 line, at most 1024 bytes, no NUL inside the length. */
int32_t gamepkg_host_log(const char *text, uint32_t length);
/* Host tick hook. Not for the engine. */
void gamepkg_host_begin_frame(void);

#ifdef __cplusplus
}
#endif

#endif
