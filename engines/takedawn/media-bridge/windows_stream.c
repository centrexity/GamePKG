/* Native Windows startup and SDL streams over the verified in-place reader.
 * Original rendering/audio/game functions are unchanged. Host paths remain
 * host paths; the launcher owns process confinement. */
#include "windows_file.h"
#include "windows_stream.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>

static int wide_path(const char *path, WCHAR output[32768]) {
    if (!path) { SetLastError(ERROR_INVALID_PARAMETER); return 0; }
    return MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, output, 32768);
}
static HANDLE open_read(const char *path) {
    WCHAR wide[32768];
    if (!wide_path(path, wide)) return INVALID_HANDLE_VALUE;
    return gpw_CreateFileW(wide, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL);
}
static int handle_size(HANDLE file, uint64_t *size) {
    BY_HANDLE_FILE_INFORMATION info;
    if (!gpw_GetFileInformationByHandle(file, &info)) return 0;
    if (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) { SetLastError(ERROR_ACCESS_DENIED); return 0; }
    *size = ((uint64_t)info.nFileSizeHigh << 32) | info.nFileSizeLow;
    return 1;
}
int64_t gpw_file_size(const char *path) {
    HANDLE file = open_read(path);
    if (file == INVALID_HANDLE_VALUE) return -1;
    uint64_t size = 0; int ok = handle_size(file, &size); DWORD error = GetLastError();
    if (!gpw_CloseHandle(file)) return -1;
    if (!ok) { SetLastError(error); return -1; }
    if (size > INT64_MAX) { SetLastError(ERROR_FILE_TOO_LARGE); return -1; }
    return (int64_t)size;
}
int gpw_load_xbe(const char *path, void **output, size_t *output_size) {
    if (!output || !output_size) { SetLastError(ERROR_INVALID_PARAMETER); return 0; }
    HANDLE file = open_read(path);
    if (file == INVALID_HANDLE_VALUE) return 0;
    uint64_t size = 0; DWORD error = ERROR_SUCCESS;
    if (!handle_size(file, &size)) error = GetLastError();
    else if (!size || size > SIZE_MAX) error = ERROR_FILE_TOO_LARGE;
    uint8_t *data = error ? NULL : (uint8_t *)malloc((size_t)size);
    if (!error && !data) error = ERROR_NOT_ENOUGH_MEMORY;
    size_t total = 0;
    while (!error && total < size) {
        size_t remaining = (size_t)size - total;
        DWORD chunk = remaining > 1048576 ? 1048576 : (DWORD)remaining, count = 0;
        if (!gpw_ReadFile(file, data + total, chunk, &count, NULL)) error = GetLastError();
        else if (!count) error = ERROR_HANDLE_EOF;
        else total += count;
    }
    if (!gpw_CloseHandle(file) && !error) error = GetLastError();
    if (error) { free(data); SetLastError(error); return 0; }
    *output = data; *output_size = total; return 1;
}
static int stream_error(const char *operation) {
    return SDL_SetError("GamePKG disc %s failed (Windows error %lu)", operation, (unsigned long)GetLastError());
}
static Sint64 SDLCALL stream_size(SDL_RWops *stream) {
    uint64_t size;
    if (!handle_size((HANDLE)stream->hidden.unknown.data1, &size)) return stream_error("size");
    if (size > INT64_MAX) return SDL_SetError("GamePKG disc stream is too large");
    return (Sint64)size;
}
static Sint64 SDLCALL stream_seek(SDL_RWops *stream, Sint64 offset, int origin) {
    DWORD method;
    switch (origin) {
        case RW_SEEK_SET: method = FILE_BEGIN; break;
        case RW_SEEK_CUR: method = FILE_CURRENT; break;
        case RW_SEEK_END: method = FILE_END; break;
        default: return SDL_SetError("GamePKG disc seek origin is invalid");
    }
    LARGE_INTEGER distance, position; distance.QuadPart = offset;
    if (!gpw_SetFilePointerEx((HANDLE)stream->hidden.unknown.data1, distance, &position, method))
        return stream_error("seek");
    return position.QuadPart;
}
static size_t SDLCALL stream_read(SDL_RWops *stream, void *output, size_t size, size_t maximum) {
    if (!size || !maximum) return 0;
    if (!output || maximum > SIZE_MAX / size) { SDL_SetError("GamePKG disc read size is invalid"); return 0; }
    size_t bytes = size * maximum, total = 0;
    while (total < bytes) {
        size_t remaining = bytes - total;
        DWORD chunk = remaining > 1048576 ? 1048576 : (DWORD)remaining, count = 0;
        if (!gpw_ReadFile((HANDLE)stream->hidden.unknown.data1, (uint8_t *)output + total, chunk, &count, NULL)) {
            stream_error("read"); break;
        }
        if (!count) break;
        total += count;
    }
    return total / size;
}
static size_t SDLCALL stream_write(SDL_RWops *stream, const void *data, size_t size, size_t count) {
    (void)stream; (void)data; (void)size; (void)count;
    SDL_SetError("GamePKG disc streams are read-only"); return 0;
}
static int SDLCALL stream_close(SDL_RWops *stream) {
    int result = gpw_CloseHandle((HANDLE)stream->hidden.unknown.data1) ? 0 : stream_error("close");
    SDL_FreeRW(stream); return result;
}
SDL_RWops *gpw_SDL_RWFromFile(const char *path, const char *mode) {
    if (!mode) { SDL_SetError("GamePKG disc stream mode is missing"); return NULL; }
    WCHAR wide[32768];
    if (!wide_path(path, wide)) { stream_error("path"); return NULL; }
    int kind = gpw_DiscPathW(wide);
    if (!kind) return SDL_RWFromFile(path, mode);
    if (kind < 0) { stream_error("path"); return NULL; }
    if (strcmp(mode, "r") && strcmp(mode, "rb")) { SDL_SetError("GamePKG disc streams are read-only"); return NULL; }
    HANDLE file = gpw_CreateFileW(wide, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL);
    if (file == INVALID_HANDLE_VALUE) { stream_error("open"); return NULL; }
    uint64_t size;
    if (!handle_size(file, &size)) {
        DWORD error = GetLastError(); gpw_CloseHandle(file); SetLastError(error); stream_error("open"); return NULL;
    }
    SDL_RWops *stream = SDL_AllocRW();
    if (!stream) { gpw_CloseHandle(file); return NULL; }
    stream->size = stream_size; stream->seek = stream_seek; stream->read = stream_read;
    stream->write = stream_write; stream->close = stream_close;
    stream->type = SDL_RWOPS_UNKNOWN; stream->hidden.unknown.data1 = file;
    stream->hidden.unknown.data2 = NULL;
    return stream;
}
