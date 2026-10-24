#ifndef GAMEPKG_TAKEDAWN_MEDIA_H
#define GAMEPKG_TAKEDAWN_MEDIA_H
#include <stdint.h>
#include <stddef.h>

/* Host-only API, not part of the guest SDK. Every pointer must address live,
 * correctly sized memory for the duration of the call. Strings are UTF-8 with
 * explicit byte lengths (no trailing NUL). Results are zero or negative POSIX
 * errno numbers, defined independently of the compiling host. No exceptions or
 * Rust panics cross this boundary. Paths are relative to the admitted disc.
 * Initialization hashes the entire pinned disc; no contents are extracted.
 * Shutdown refuses live handles. Calls are synchronized, handles never recycle
 * within this process, and no raw pointers to internal objects are exposed.
 * budget_bytes must be charged against the owning game's host memory budget.
 */
#ifdef __cplusplus
extern "C" {
#endif
typedef struct gp_media_info {
    uint64_t size;
    uint32_t is_directory;
    uint32_t reserved;
} gp_media_info;
int32_t gp_media_init(const uint8_t *iso, size_t length, uint64_t budget_bytes);
#if defined(_WIN32)
/* Borrows a launcher-supplied file handle in this process. The reader creates
 * its own read-only noninheritable duplicate; it never reopens the pathname or
 * closes the supplied original. The same full pinned manifest is verified. */
int32_t gp_media_init_handle(uintptr_t handle, uint64_t budget_bytes);
#endif
int32_t gp_media_shutdown(void);
int32_t gp_media_open(const uint8_t *path, size_t length, uint64_t *handle);
int32_t gp_media_close(uint64_t handle);
int32_t gp_media_stat(const uint8_t *path, size_t length, gp_media_info *info);
int32_t gp_media_fstat(uint64_t handle, gp_media_info *info);
/* Reads may be short. At most 64 KiB is returned per call. Output and position
 * remain unchanged on integrity failure. A zero-byte read also validates the
 * handle and rejects directory handles. */
int32_t gp_media_read(uint64_t handle, uint8_t *output, size_t capacity, size_t *read);
int32_t gp_media_seek(uint64_t handle, int64_t offset, int32_t whence, uint64_t *position);
/* Independent directory cursor. EOF gives name_length=0. No '.'/'..' entries.
 * An insufficient name buffer gives -ENOBUFS without advancing the cursor.
 * Names are UTF-8 bytes, not NUL-terminated. */
int32_t gp_media_readdir(uint64_t handle, uint8_t *name, size_t capacity,
                         size_t *name_length, gp_media_info *info);
#ifdef __cplusplus
}
#endif
#endif
