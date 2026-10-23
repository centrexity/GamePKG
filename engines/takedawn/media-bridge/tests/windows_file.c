/* Native Windows API oracle over synthetic data. No game assets are used. */
#include "../windows_file.h"
#include "../media.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
typedef struct { uint64_t id, position, size; int directory; } fake_stream;
static fake_stream streams[128];
static uint64_t next_id;
static unsigned live, stat_calls;
static int corrupt;
static LONG block_read;
static HANDLE read_started, read_resume;
static const uint64_t small_size = 131101, large_size = UINT64_C(0x200000007);
static uint8_t pattern(uint64_t offset) { return (uint8_t)((offset * 37) ^ (offset >> 5)); }
static fake_stream *stream(uint64_t id) {
    for (unsigned i = 0; i < 128; i++) if (streams[i].id == id) return streams + i;
    return NULL;
}
int32_t gp_media_stat(const uint8_t *name, size_t length, gp_media_info *info) {
    char path[512]; assert(length < sizeof path); memcpy(path, name, length); path[length] = 0;
    stat_calls++;
    memset(info, 0, sizeof *info);
    if (!*path || !_stricmp(path, "Data")) info->is_directory = 1;
    else if (!_stricmp(path, "Data/pattern.bin")) info->size = small_size;
    else if (!_stricmp(path, "Data/large.bin")) info->size = large_size;
    else return -2;
    return 0;
}
int32_t gp_media_open(const uint8_t *name, size_t length, uint64_t *output) {
    gp_media_info info;
    int32_t rc = gp_media_stat(name, length, &info); if (rc) return rc;
    for (unsigned i = 0; i < 128; i++) if (!streams[i].id) {
        streams[i] = (fake_stream){++next_id, 0, info.size, (int)info.is_directory};
        *output = next_id; live++; return 0;
    }
    return -24;
}
int32_t gp_media_close(uint64_t id) {
    fake_stream *s = stream(id); if (!s) return -9;
    memset(s, 0, sizeof *s); live--; return 0;
}
int32_t gp_media_fstat(uint64_t id, gp_media_info *info) {
    fake_stream *s = stream(id); if (!s) return -9;
    *info = (gp_media_info){s->size, (uint32_t)s->directory, 0}; return 0;
}
int32_t gp_media_seek(uint64_t id, int64_t offset, int32_t whence, uint64_t *position) {
    fake_stream *s = stream(id); if (!s) return -9;
    assert(whence == 0);
    if (offset < 0 || (uint64_t)offset > s->size) return -22;
    s->position = (uint64_t)offset; *position = s->position; return 0;
}
int32_t gp_media_read(uint64_t id, uint8_t *output, size_t capacity, size_t *read) {
    fake_stream *s = stream(id); if (!s) return -9;
    if (s->directory) return -21;
    if (corrupt) return -5;
    if (InterlockedExchange(&block_read, 0)) {
        assert(SetEvent(read_started));
        assert(WaitForSingleObject(read_resume, 5000) == WAIT_OBJECT_0);
    }
    size_t count = capacity > 65536 ? 65536 : capacity;
    if ((uint64_t)count > s->size - s->position) count = (size_t)(s->size - s->position);
    for (size_t i = 0; i < count; i++) output[i] = pattern(s->position + i);
    s->position += count; *read = count; return 0;
}
static HANDLE open_disc(void) {
    HANDLE h = gpw_CreateFileW(L"/gamepkg-disc/Data/pattern.bin", GENERIC_READ, FILE_SHARE_READ,
                               NULL, OPEN_EXISTING, 0, NULL);
    assert(h != INVALID_HANDLE_VALUE); return h;
}
static uint64_t position(HANDLE h, int virtual_file) {
    LARGE_INTEGER zero = {0}, result;
    assert(virtual_file ? gpw_SetFilePointerEx(h, zero, &result, FILE_CURRENT)
                        : SetFilePointerEx(h, zero, &result, FILE_CURRENT));
    return (uint64_t)result.QuadPart;
}
static void compare_read(HANDLE disc, HANDLE native, DWORD size, int explicit_offset, uint64_t offset) {
    uint8_t a[150000], b[150000]; assert(size <= sizeof a);
    memset(a, 0xA5, sizeof a); memset(b, 0xA5, sizeof b);
    OVERLAPPED oa = {0}, ob = {0};
    oa.Offset = ob.Offset = (DWORD)offset; oa.OffsetHigh = ob.OffsetHigh = (DWORD)(offset >> 32);
    DWORD ca = 999, cb = 999;
    BOOL reference = ReadFile(native, a, size, &ca, explicit_offset ? &oa : NULL);
    DWORD error = GetLastError();
    BOOL actual = gpw_ReadFile(disc, b, size, &cb, explicit_offset ? &ob : NULL);
    assert(actual == reference && ca == cb);
    if (!actual) assert(GetLastError() == error);
    assert(!memcmp(a, b, sizeof a));
    uint64_t disc_position = position(disc, 1), native_position = position(native, 0);
    if (disc_position != native_position) fprintf(stderr,
        "read size=%lu explicit=%d offset=%llu count=%lu result=%d native-position=%llu adapter-position=%llu\n",
        (unsigned long)size, explicit_offset, (unsigned long long)offset,
        (unsigned long)ca, reference, (unsigned long long)native_position,
        (unsigned long long)disc_position);
    assert(disc_position == native_position);
    if (explicit_offset) assert(oa.Internal == ob.Internal && oa.InternalHigh == ob.InternalHigh);
}
static void compare_seek(HANDLE disc, HANDLE native, int64_t delta, DWORD method) {
    LARGE_INTEGER distance, a, b; distance.QuadPart = delta;
    a.QuadPart = b.QuadPart = 123;
    BOOL reference = SetFilePointerEx(native, distance, &a, method); DWORD error = GetLastError();
    BOOL actual = gpw_SetFilePointerEx(disc, distance, &b, method);
    if (actual != reference || a.QuadPart != b.QuadPart) {
        fprintf(stderr, "seek delta=%lld method=%lu native=%d/%lld error=%lu adapter=%d/%lld error=%lu\n",
            (long long)delta, (unsigned long)method, reference, (long long)a.QuadPart,
            (unsigned long)error, actual, (long long)b.QuadPart, (unsigned long)GetLastError());
    }
    assert(actual == reference && a.QuadPart == b.QuadPart);
    if (!actual) assert(GetLastError() == error);
    assert(position(disc, 1) == position(native, 0));
}
static DWORD WINAPI blocking_read(LPVOID argument) {
    uint8_t data[64]; DWORD count;
    assert(gpw_ReadFile((HANDLE)argument, data, sizeof data, &count, NULL) && count == sizeof data);
    return 0;
}
int main(void) {
    HANDLE native = CreateFileW(L"native-file-oracle.bin", GENERIC_READ | GENERIC_WRITE, 0, NULL,
        CREATE_ALWAYS, FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_DELETE_ON_CLOSE, NULL);
    assert(native != INVALID_HANDLE_VALUE);
    uint8_t data[131101];
    for (size_t i = 0; i < sizeof data; i++) data[i] = pattern(i);
    DWORD count; assert(WriteFile(native, data, sizeof data, &count, NULL) && count == sizeof data);
    LARGE_INTEGER zero = {0}; assert(SetFilePointerEx(native, zero, NULL, FILE_BEGIN));
    HANDLE disc = open_disc();
    assert(WaitForSingleObject(disc, 0) == WAIT_OBJECT_0);
    compare_read(disc, native, 7, 0, 0);
    compare_read(disc, native, 70000, 1, 123);
    compare_read(disc, native, 140000, 1, 0);
    compare_read(disc, native, 1, 0, 0);
    compare_read(disc, native, 1, 1, small_size);
    compare_read(disc, native, 0, 1, 3);
    compare_read(disc, native, 13, 0, 0);
    compare_seek(disc, native, -1, FILE_BEGIN);
    compare_seek(disc, native, -7, FILE_CURRENT);
    compare_seek(disc, native, INT64_MIN, FILE_CURRENT);
    compare_seek(disc, native, 19, FILE_END);
    compare_read(disc, native, 19, 0, 0);
    compare_read(disc, native, 1, 1, UINT64_C(0x100000007));
    /* Overflow from a valid >4 GiB position is an API arithmetic case. An
     * absolute INT64_MAX seek instead tests the backing filesystem's own
     * maximum offset (Wine's host filesystem and NTFS may differ). */
    compare_seek(disc, native, INT64_MAX, FILE_CURRENT);
    compare_seek(disc, native, 1, FILE_CURRENT);
    compare_seek(disc, native, 0, FILE_BEGIN);
    compare_seek(disc, native, 0, 123);
    BY_HANDLE_FILE_INFORMATION info;
    assert(gpw_GetFileInformationByHandle(disc, &info));
    assert(info.nFileSizeLow == small_size && !info.nFileSizeHigh && info.nNumberOfLinks == 1);
    assert(info.dwFileAttributes == FILE_ATTRIBUTE_READONLY);
    WIN32_FILE_ATTRIBUTE_DATA attrs;
    assert(gpw_GetFileAttributesExW(L"C:\\gamepkg-disc\\Data\\pattern.bin", GetFileExInfoStandard, &attrs));
    assert(attrs.nFileSizeLow == small_size && attrs.dwFileAttributes == FILE_ATTRIBUTE_READONLY);
    assert(!gpw_WriteFile(disc, data, 1, &count, NULL) && GetLastError() == ERROR_WRITE_PROTECT && count == 0);
    assert(!gpw_SetEndOfFile(disc) && GetLastError() == ERROR_WRITE_PROTECT);
    assert(!gpw_FlushFileBuffers(disc) && GetLastError() == ERROR_ACCESS_DENIED);
    HANDLE other = gpw_CreateFileW(L"\\gamepkg-disc\\DATA\\PATTERN.BIN", GENERIC_READ, 0, NULL, OPEN_EXISTING, 0, NULL);
    assert(other == INVALID_HANDLE_VALUE && GetLastError() == ERROR_SHARING_VIOLATION);
    other = open_disc(); assert(position(other, 1) == 0); assert(gpw_CloseHandle(other));
    WCHAR path[512]; DWORD needed = gpw_GetFinalPathNameByHandleW(disc, NULL, 0, 0);
    assert(needed > 1 && gpw_GetFinalPathNameByHandleW(disc, path, needed, 0) == needed - 1);
    assert(!wcscmp(path, L"\\gamepkg-disc\\Data\\pattern.bin"));
    corrupt = 1; memset(data, 0xA5, sizeof data);
    assert(!gpw_ReadFile(disc, data, sizeof data, &count, NULL) && GetLastError() == ERROR_READ_FAULT && count == 0);
    for (size_t i = 0; i < sizeof data; i++) assert(data[i] == 0xA5);
    corrupt = 0;
    read_started = CreateEventW(NULL, TRUE, FALSE, NULL);
    read_resume = CreateEventW(NULL, TRUE, FALSE, NULL);
    assert(read_started && read_resume);
    InterlockedExchange(&block_read, 1);
    HANDLE thread = CreateThread(NULL, 0, blocking_read, disc, 0, NULL);
    assert(thread && WaitForSingleObject(read_started, 5000) == WAIT_OBJECT_0);
    assert(WaitForSingleObject(disc, 0) == WAIT_TIMEOUT);
    assert(SetEvent(read_resume) && WaitForSingleObject(thread, 5000) == WAIT_OBJECT_0);
    assert(WaitForSingleObject(disc, 0) == WAIT_OBJECT_0);
    assert(CloseHandle(thread) && CloseHandle(read_started) && CloseHandle(read_resume));
    assert(gpw_CloseHandle(disc)); assert(!gpw_CloseHandle(disc) && GetLastError() == ERROR_INVALID_HANDLE);
    assert(CloseHandle(native));
    HANDLE large = gpw_CreateFileW(L"/gamepkg-disc/Data/large.bin", GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL);
    assert(large != INVALID_HANDLE_VALUE);
    OVERLAPPED ov = {0}; ov.OffsetHigh = 1; ov.Offset = 5;
    assert(gpw_ReadFile(large, data, 17, &count, &ov) && count == 17);
    for (unsigned i = 0; i < count; i++) assert(data[i] == pattern(UINT64_C(0x100000005) + i));
    assert(gpw_CloseHandle(large));
    HANDLE directory = gpw_CreateFileW(L"/gamepkg-disc/Data", GENERIC_READ, FILE_SHARE_READ,
                                        NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, NULL);
    assert(directory != INVALID_HANDLE_VALUE);
    assert(gpw_GetFileInformationByHandle(directory, &info) && (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY));
    assert(!gpw_ReadFile(directory, data, 1, &count, NULL) && GetLastError() == ERROR_ACCESS_DENIED);
    assert(gpw_CloseHandle(directory));
    HANDLE metadata = gpw_CreateFileW(L"/gamepkg-disc/Data/pattern.bin", 0, FILE_SHARE_READ,
                                       NULL, OPEN_EXISTING, 0, NULL);
    assert(metadata != INVALID_HANDLE_VALUE);
    assert(!gpw_ReadFile(metadata, data, 1, &count, NULL) && GetLastError() == ERROR_ACCESS_DENIED);
    assert(gpw_GetFileInformationByHandle(metadata, &info));
    assert(gpw_CloseHandle(metadata));
    HANDLE existing = gpw_CreateFileW(L"/gamepkg-disc/Data/pattern.bin", GENERIC_READ, FILE_SHARE_READ,
                                       NULL, OPEN_ALWAYS, 0, NULL);
    assert(existing != INVALID_HANDLE_VALUE && GetLastError() == ERROR_ALREADY_EXISTS);
    assert(gpw_CloseHandle(existing));
    const DWORD mutations[] = {CREATE_NEW, CREATE_ALWAYS, TRUNCATE_EXISTING};
    for (unsigned i = 0; i < sizeof mutations / sizeof *mutations; i++) {
        assert(gpw_CreateFileW(L"/gamepkg-disc/Data/pattern.bin", GENERIC_READ, FILE_SHARE_READ,
                               NULL, mutations[i], 0, NULL) == INVALID_HANDLE_VALUE);
        assert(GetLastError() == ERROR_WRITE_PROTECT);
    }
    assert(gpw_CreateFileW(L"/gamepkg-disc/Data/pattern.bin", GENERIC_WRITE, FILE_SHARE_READ,
                           NULL, OPEN_EXISTING, 0, NULL) == INVALID_HANDLE_VALUE);
    assert(GetLastError() == ERROR_WRITE_PROTECT);
    const DWORD unsupported[] = {FILE_FLAG_OVERLAPPED, FILE_FLAG_NO_BUFFERING};
    for (unsigned i = 0; i < sizeof unsupported / sizeof *unsupported; i++) {
        assert(gpw_CreateFileW(L"/gamepkg-disc/Data/pattern.bin", GENERIC_READ, FILE_SHARE_READ,
                               NULL, OPEN_EXISTING, unsupported[i], NULL) == INVALID_HANDLE_VALUE);
        assert(GetLastError() == ERROR_NOT_SUPPORTED);
    }
    HANDLE handles[128]; for (unsigned i = 0; i < 128; i++) handles[i] = open_disc();
    assert(gpw_CreateFileW(L"/gamepkg-disc/Data/pattern.bin", GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL) == INVALID_HANDLE_VALUE);
    assert(GetLastError() == ERROR_TOO_MANY_OPEN_FILES);
    for (unsigned i = 0; i < 128; i++) assert(gpw_CloseHandle(handles[i]));
    const WCHAR *bad[] = {L"/gamepkg-disc/../escape.bin", L"Burnout 3 Takedown/../escape.bin",
        L"C:\\gamepkg-disc\\Data\\..\\escape.bin", L"\\\\?\\C:\\gamepkg-disc\\..\\escape.bin",
        L"/gamepkg-disc/Data//pattern.bin", L"/gamepkg-disc/Data/pattern.bin:stream",
        L"/gamepkg-disc/Data/./pattern.bin", L"/gamepkg-disc/Data/pattern.bin/",
        L"/gamepkg-disc/Data/\x007f.bin", L"/gamepkg-disc/Data/\x0085.bin"};
    for (unsigned i = 0; i < sizeof bad / sizeof *bad; i++) {
        unsigned before = stat_calls;
        assert(gpw_CreateFileW(bad[i], GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL) == INVALID_HANDLE_VALUE);
        assert(GetLastError() == ERROR_ACCESS_DENIED && before == stat_calls);
    }
    HANDLE outside = gpw_CreateFileW(L"native-pass-through.bin", GENERIC_READ | GENERIC_WRITE,
        0, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_DELETE_ON_CLOSE, NULL);
    assert(outside != INVALID_HANDLE_VALUE);
    assert(gpw_WriteFile(outside, "native", 6, &count, NULL) && count == 6);
    assert(gpw_SetFilePointerEx(outside, zero, NULL, FILE_BEGIN));
    assert(gpw_ReadFile(outside, data, 6, &count, NULL) && count == 6 && !memcmp(data, "native", 6));
    assert(gpw_CloseHandle(outside));
    assert(!live);
    puts("PASS: native Windows disc file adapter; native read/seek/EOF oracle, 64-bit offsets, read-only metadata, write/traversal rejection, sharing, waitable handles, integrity failure and handle cleanup");
    return 0;
}
