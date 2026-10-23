/* Windows host transport, not a replacement game/kernel or process sandbox.
 * Real waitable event handles represent the synchronous read-only DVD files.
 * File bytes come exclusively from gp_media_*; nothing is extracted to disk.
 */
#include "windows_file.h"
#include "media.h"
#include <stdint.h>
#include <string.h>
#include <wchar.h>
#define GPW_SLOTS 128
#define GPW_PATH 512
typedef struct {
    HANDLE event;
    uint64_t media, position;
    DWORD access, share;
    char path[GPW_PATH];
} gpw_file;
static gpw_file files[GPW_SLOTS];
static SRWLOCK files_lock = SRWLOCK_INIT;
static BOOL fail(DWORD error) { SetLastError(error); return FALSE; }
static DWORD media_error(int32_t error) {
    switch (error) {
        case -2: return ERROR_FILE_NOT_FOUND;
        case -9: return ERROR_INVALID_HANDLE;
        case -12: case -24: return ERROR_NOT_ENOUGH_MEMORY;
        case -20: return ERROR_PATH_NOT_FOUND;
        case -21: return ERROR_ACCESS_DENIED;
        case -22: return ERROR_INVALID_PARAMETER;
        default: return ERROR_READ_FAULT;
    }
}
static int sep(WCHAR c) { return c == L'/' || c == L'\\'; }
/* 0: ordinary host path; 1: valid virtual disc path; -1: rejected disc path.
 * A rejected virtual path must never fall through to the host filesystem. */
static int relative_path(LPCWSTR path, char output[GPW_PATH]) {
    if (!path) { SetLastError(ERROR_INVALID_PARAMETER); return -1; }
    LPCWSTR candidate = path;
    if (!wcsncmp(candidate, L"\\\\?\\", 4)) candidate += 4;
    if (candidate[0] && candidate[1] == L':' && sep(candidate[2])) candidate += 2;
    const WCHAR *roots[] = {L"/gamepkg-disc", L"Burnout 3 Takedown"};
    size_t used = 0;
    for (unsigned root = 0; root < sizeof roots / sizeof *roots; root++) {
        size_t n = wcslen(roots[root]), i = 0;
        while (i < n && candidate[i] &&
               ((sep(roots[root][i]) && sep(candidate[i])) ||
                CompareStringOrdinal(candidate + i, 1, roots[root] + i, 1, TRUE) == CSTR_EQUAL)) i++;
        if (i == n && (!candidate[n] || sep(candidate[n]))) { used = n; break; }
    }
    if (!used) return 0;
    LPCWSTR tail = candidate + used;
    if (sep(*tail)) tail++;
    for (LPCWSTR p = tail; *p; p++) if (*p < 32 || (*p >= 127 && *p <= 159)) {
        SetLastError(ERROR_ACCESS_DENIED); return -1;
    }
    int count = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, tail, -1,
                                    output, GPW_PATH, NULL, NULL);
    if (!count) { SetLastError(ERROR_INVALID_NAME); return -1; }
    for (char *p = output; *p; p++) {
        if (*p == '\\') *p = '/';
        if ((unsigned char)*p < 32 || *p == ':' || *p == '*' || *p == '?') {
            SetLastError(ERROR_ACCESS_DENIED); return -1;
        }
    }
    if (*output) {
        const char *part = output;
        for (const char *p = output;; p++) {
            if (*p == '/' || !*p) {
                size_t n = (size_t)(p - part);
                if (!n || (n == 1 && part[0] == '.') ||
                    (n == 2 && part[0] == '.' && part[1] == '.')) {
                    SetLastError(ERROR_ACCESS_DENIED); return -1;
                }
                if (!*p) break;
                part = p + 1;
            }
        }
    }
    return 1;
}
static gpw_file *lookup(HANDLE handle) {
    for (unsigned i = 0; i < GPW_SLOTS; i++) if (files[i].event == handle && handle) return files + i;
    return NULL;
}
static int shares_read(DWORD access) {
    return (access & (GENERIC_READ | GENERIC_EXECUTE | FILE_READ_DATA | FILE_EXECUTE)) != 0;
}
static int can_read(DWORD access) { return (access & (GENERIC_READ | FILE_READ_DATA)) != 0; }
static DWORD attributes(gp_media_info info) {
    return FILE_ATTRIBUTE_READONLY | (info.is_directory ? FILE_ATTRIBUTE_DIRECTORY : 0);
}
HANDLE WINAPI gpw_CreateFileW(LPCWSTR path, DWORD access, DWORD share,
    LPSECURITY_ATTRIBUTES security, DWORD disposition, DWORD flags, HANDLE template_file) {
    char relative[GPW_PATH];
    int kind = relative_path(path, relative);
    if (!kind) return CreateFileW(path, access, share, security, disposition, flags, template_file);
    if (kind < 0) return INVALID_HANDLE_VALUE;
    DWORD writes = GENERIC_WRITE | GENERIC_ALL | FILE_WRITE_DATA | FILE_APPEND_DATA |
        FILE_WRITE_EA | FILE_WRITE_ATTRIBUTES | DELETE | WRITE_DAC | WRITE_OWNER;
    if ((access & writes) || (flags & FILE_FLAG_DELETE_ON_CLOSE) ||
        (disposition != OPEN_EXISTING && disposition != OPEN_ALWAYS)) {
        SetLastError(ERROR_WRITE_PROTECT); return INVALID_HANDLE_VALUE;
    }
    /* The reviewed kernel backend deliberately opens buffered synchronous
     * files. Do not silently claim asynchronous or sector-aligned I/O here. */
    if (flags & (FILE_FLAG_OVERLAPPED | FILE_FLAG_NO_BUFFERING)) {
        SetLastError(ERROR_NOT_SUPPORTED); return INVALID_HANDLE_VALUE;
    }
    if (share & ~(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)) {
        SetLastError(ERROR_INVALID_PARAMETER); return INVALID_HANDLE_VALUE;
    }
    gp_media_info info = {0};
    int32_t rc = gp_media_stat((const uint8_t *)relative, strlen(relative), &info);
    if (rc) { SetLastError(disposition == OPEN_ALWAYS ? ERROR_WRITE_PROTECT : media_error(rc)); return INVALID_HANDLE_VALUE; }
    if (info.is_directory && !(flags & FILE_FLAG_BACKUP_SEMANTICS)) {
        SetLastError(ERROR_ACCESS_DENIED); return INVALID_HANDLE_VALUE;
    }
    AcquireSRWLockExclusive(&files_lock);
    gpw_file *free_slot = NULL;
    DWORD error = ERROR_SUCCESS;
    for (unsigned i = 0; i < GPW_SLOTS; i++) {
        gpw_file *entry = files + i;
        if (!entry->event) { if (!free_slot) free_slot = entry; continue; }
        if (_stricmp(entry->path, relative)) continue;
        if ((shares_read(access) && !(entry->share & FILE_SHARE_READ)) ||
            (shares_read(entry->access) && !(share & FILE_SHARE_READ))) error = ERROR_SHARING_VIOLATION;
    }
    if (!error && !free_slot) error = ERROR_TOO_MANY_OPEN_FILES;
    if (error) { ReleaseSRWLockExclusive(&files_lock); SetLastError(error); return INVALID_HANDLE_VALUE; }
    uint64_t media;
    rc = gp_media_open((const uint8_t *)relative, strlen(relative), &media);
    if (rc) { ReleaseSRWLockExclusive(&files_lock); SetLastError(media_error(rc)); return INVALID_HANDLE_VALUE; }
    HANDLE event = CreateEventW(security, TRUE, TRUE, NULL);
    if (!event) {
        error = GetLastError(); gp_media_close(media); ReleaseSRWLockExclusive(&files_lock);
        SetLastError(error); return INVALID_HANDLE_VALUE;
    }
    free_slot->event = event; free_slot->media = media; free_slot->position = 0;
    free_slot->access = access; free_slot->share = share;
    memcpy(free_slot->path, relative, strlen(relative) + 1);
    ReleaseSRWLockExclusive(&files_lock);
    if (disposition == OPEN_ALWAYS) SetLastError(ERROR_ALREADY_EXISTS);
    return event;
}
BOOL WINAPI gpw_CloseHandle(HANDLE handle) {
    AcquireSRWLockExclusive(&files_lock);
    gpw_file *entry = lookup(handle);
    if (!entry) { ReleaseSRWLockExclusive(&files_lock); return CloseHandle(handle); }
    int32_t rc = gp_media_close(entry->media);
    if (rc) { ReleaseSRWLockExclusive(&files_lock); return fail(media_error(rc)); }
    memset(entry, 0, sizeof *entry);
    BOOL result = CloseHandle(handle);
    DWORD error = GetLastError();
    ReleaseSRWLockExclusive(&files_lock); SetLastError(error); return result;
}
BOOL WINAPI gpw_ReadFile(HANDLE handle, LPVOID buffer, DWORD length, LPDWORD output, LPOVERLAPPED ov) {
    AcquireSRWLockExclusive(&files_lock);
    gpw_file *entry = lookup(handle);
    if (!entry) { ReleaseSRWLockExclusive(&files_lock); return ReadFile(handle, buffer, length, output, ov); }
    if (output) *output = 0;
    DWORD error = ERROR_SUCCESS, total = 0;
    gp_media_info info;
    int32_t rc = gp_media_fstat(entry->media, &info);
    if (rc) error = media_error(rc);
    else if (!can_read(entry->access) || info.is_directory) error = ERROR_ACCESS_DENIED;
    else if ((!output && !ov) || (length && !buffer)) error = ERROR_INVALID_PARAMETER;
    uint64_t position = ov ? ((uint64_t)ov->OffsetHigh << 32) | ov->Offset : entry->position;
    if (!error && position > INT64_MAX) error = ERROR_INVALID_PARAMETER;
    BOOL started = !error;
    if (!error) {
        ResetEvent(entry->event);
        if (ov && ov->hEvent) ResetEvent(ov->hEvent);
        /* Native Windows preserves the cursor on a zero-byte read and on an
         * explicit-offset read failing at EOF. Wine moves it in those cases;
         * the native oracle is decisive. */
        if (length && position < info.size) {
            entry->position = position;
            uint64_t actual;
            rc = gp_media_seek(entry->media, (int64_t)(position < info.size ? position : info.size), 0, &actual);
            if (rc) error = media_error(rc);
            while (!error && total < length && entry->position < info.size) {
                size_t count = 0;
                rc = gp_media_read(entry->media, (uint8_t *)buffer + total, length - total, &count);
                if (rc) { error = media_error(rc); break; }
                if (!count) break;
                total += (DWORD)count; entry->position += count;
            }
        }
        if (!error && !total && length && ov) error = ERROR_HANDLE_EOF;
    }
    if (output) *output = total;
    if (ov) {
        ov->Internal = error == ERROR_HANDLE_EOF ? 0xC0000011u : error ? 0xC0000001u : 0;
        ov->InternalHigh = total;
        if (started && ov->hEvent) SetEvent(ov->hEvent);
    }
    SetEvent(entry->event);
    ReleaseSRWLockExclusive(&files_lock);
    return error ? fail(error) : TRUE;
}
BOOL WINAPI gpw_WriteFile(HANDLE handle, LPCVOID buffer, DWORD length, LPDWORD output, LPOVERLAPPED ov) {
    AcquireSRWLockShared(&files_lock); BOOL disc = lookup(handle) != NULL;
    ReleaseSRWLockShared(&files_lock);
    if (!disc) return WriteFile(handle, buffer, length, output, ov);
    if (output) *output = 0;
    return fail(ERROR_WRITE_PROTECT);
}
BOOL WINAPI gpw_SetFilePointerEx(HANDLE handle, LARGE_INTEGER distance, PLARGE_INTEGER output, DWORD method) {
    AcquireSRWLockExclusive(&files_lock);
    gpw_file *entry = lookup(handle);
    if (!entry) { ReleaseSRWLockExclusive(&files_lock); return SetFilePointerEx(handle, distance, output, method); }
    gp_media_info info = {0};
    int32_t rc = gp_media_fstat(entry->media, &info);
    DWORD error = rc ? media_error(rc) : (info.is_directory || !can_read(entry->access)) ? ERROR_ACCESS_DENIED : ERROR_SUCCESS;
    uint64_t base = method == FILE_BEGIN ? 0 : method == FILE_CURRENT ? entry->position : info.size;
    if (method != FILE_BEGIN && method != FILE_CURRENT && method != FILE_END) error = ERROR_INVALID_PARAMETER;
    uint64_t delta = distance.QuadPart < 0 ? (uint64_t)(-(distance.QuadPart + 1)) + 1 : (uint64_t)distance.QuadPart;
    if (!error && (distance.QuadPart < 0 ? delta > base : delta > (uint64_t)INT64_MAX - base)) error = ERROR_NEGATIVE_SEEK;
    if (!error) {
        entry->position = distance.QuadPart < 0 ? base - delta : base + delta;
        if (output) output->QuadPart = (LONGLONG)entry->position;
    }
    ReleaseSRWLockExclusive(&files_lock); return error ? fail(error) : TRUE;
}
BOOL WINAPI gpw_GetFileInformationByHandle(HANDLE handle, LPBY_HANDLE_FILE_INFORMATION output) {
    AcquireSRWLockShared(&files_lock);
    gpw_file *entry = lookup(handle);
    if (!entry) { ReleaseSRWLockShared(&files_lock); return GetFileInformationByHandle(handle, output); }
    if (!output) { ReleaseSRWLockShared(&files_lock); return fail(ERROR_INVALID_PARAMETER); }
    gp_media_info info;
    int32_t rc = gp_media_fstat(entry->media, &info);
    if (!rc) {
        memset(output, 0, sizeof *output);
        output->dwFileAttributes = attributes(info); output->nNumberOfLinks = 1;
        output->nFileSizeLow = (DWORD)info.size; output->nFileSizeHigh = (DWORD)(info.size >> 32);
    }
    ReleaseSRWLockShared(&files_lock); return rc ? fail(media_error(rc)) : TRUE;
}
BOOL WINAPI gpw_GetFileAttributesExW(LPCWSTR path, GET_FILEEX_INFO_LEVELS level, LPVOID output) {
    char relative[GPW_PATH]; int kind = relative_path(path, relative);
    if (!kind) return GetFileAttributesExW(path, level, output);
    if (kind < 0) return FALSE;
    if (level != GetFileExInfoStandard || !output) return fail(ERROR_INVALID_PARAMETER);
    gp_media_info info;
    int32_t rc = gp_media_stat((const uint8_t *)relative, strlen(relative), &info);
    if (rc) return fail(media_error(rc));
    WIN32_FILE_ATTRIBUTE_DATA *data = output;
    memset(data, 0, sizeof *data); data->dwFileAttributes = attributes(info);
    data->nFileSizeLow = (DWORD)info.size; data->nFileSizeHigh = (DWORD)(info.size >> 32);
    return TRUE;
}
DWORD WINAPI gpw_GetFileAttributesW(LPCWSTR path) {
    WIN32_FILE_ATTRIBUTE_DATA data;
    return gpw_GetFileAttributesExW(path, GetFileExInfoStandard, &data) ? data.dwFileAttributes : INVALID_FILE_ATTRIBUTES;
}
DWORD WINAPI gpw_GetFinalPathNameByHandleW(HANDLE handle, LPWSTR output, DWORD capacity, DWORD flags) {
    AcquireSRWLockShared(&files_lock); gpw_file *entry = lookup(handle);
    if (!entry) { ReleaseSRWLockShared(&files_lock); return GetFinalPathNameByHandleW(handle, output, capacity, flags); }
    if (flags & ~FILE_NAME_OPENED) { ReleaseSRWLockShared(&files_lock); SetLastError(ERROR_INVALID_PARAMETER); return 0; }
    WCHAR path[GPW_PATH + 32];
    const WCHAR root[] = L"\\gamepkg-disc";
    memcpy(path, root, sizeof root);
    size_t used = wcslen(root);
    if (*entry->path) {
        path[used++] = L'\\';
        int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, entry->path, -1,
                                       path + used, GPW_PATH);
        if (!count) { ReleaseSRWLockShared(&files_lock); return 0; }
        used += (size_t)count - 1;
    }
    for (size_t i = 0; i < used; i++) if (path[i] == L'/') path[i] = L'\\';
    DWORD needed = (DWORD)used + 1;
    if (capacity >= needed && !output) {
        ReleaseSRWLockShared(&files_lock); SetLastError(ERROR_INVALID_PARAMETER); return 0;
    }
    if (capacity >= needed) memcpy(output, path, needed * sizeof(WCHAR));
    ReleaseSRWLockShared(&files_lock);
    return capacity < needed ? needed : output ? (DWORD)used : 0;
}
BOOL WINAPI gpw_SetEndOfFile(HANDLE handle) {
    AcquireSRWLockShared(&files_lock); BOOL disc = lookup(handle) != NULL;
    ReleaseSRWLockShared(&files_lock);
    return disc ? fail(ERROR_WRITE_PROTECT) : SetEndOfFile(handle);
}
BOOL WINAPI gpw_FlushFileBuffers(HANDLE handle) {
    AcquireSRWLockShared(&files_lock); BOOL disc = lookup(handle) != NULL;
    ReleaseSRWLockShared(&files_lock);
    return disc ? fail(ERROR_ACCESS_DENIED) : FlushFileBuffers(handle);
}
