/* Host-owned native Windows entry. The original game main owns the window,
 * renderer, audio and game lifetime after read-only disc admission. */
#include "windows_file.h"
#include "media.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern int gamepkg_original_main(int, char **);
static int parse_handle(const WCHAR *text, uintptr_t *output) {
    if (!*text) return 0;
    uintptr_t value = 0;
    for (const WCHAR *p = text; *p; p++) {
        if (*p < L'0' || *p > L'9') return 0;
        uintptr_t digit = (uintptr_t)(*p - L'0');
        if (value > (UINTPTR_MAX - digit) / 10) return 0;
        value = value * 10 + digit;
    }
    if (!value || value == UINTPTR_MAX) return 0;
    *output = value; return 1;
}
int main(int argc, char **argv) {
    if (argc == 2 && !strcmp(argv[1], "--gamepkg-runtime-info")) {
        puts("{\"abi\":1,\"implementation\":\"takedawn\",\"game_commit\":\"5a00fc10f9548a189e9043c5a55975863ad49175\",\"toolkit_commit\":\"5ae8ad90d984425e834dee759592c15592679880\",\"disc_access\":\"verified-in-place\"}");
        return 0;
    }
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--install") || !strcmp(argv[i], "--data") || !strcmp(argv[i], "--data-status")) {
            fputs("GamePKG reads the selected ISO in place; installation and data-folder overrides are unavailable.\n", stderr);
            return 2;
        }
    }
    WCHAR handle_text[64] = {0};
    SetLastError(ERROR_SUCCESS);
    DWORD handle_length = GetEnvironmentVariableW(L"GAMEPKG_TAKEDAWN_DISC_HANDLE", handle_text, 64);
    BOOL handle_present = handle_length != 0 || GetLastError() != ERROR_ENVVAR_NOT_FOUND;
    int32_t result;
    if (handle_present) {
        uintptr_t handle;
        if (!handle_length || handle_length >= 64 || !parse_handle(handle_text, &handle)) {
            fputs("GamePKG: the selected ISO handle is invalid.\n", stderr); return 2;
        }
        result = gp_media_init_handle(handle, 64u * 1024u * 1024u);
        if (!result && !CloseHandle((HANDLE)handle)) {
            fputs("GamePKG: unable to release the inherited ISO handle.\n", stderr); return 3;
        }
    } else {
        /* Path admission remains available to private maintainer launchers. The
         * app supplies a handle; a present but invalid handle never falls back. */
        const WCHAR *iso = _wgetenv(L"GAMEPKG_TAKEDAWN_DISC");
        if (!iso || !*iso) {
            fputs("GamePKG: select the supported Xbox ISO before starting Takedawn.\n", stderr); return 2;
        }
        int bytes = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, iso, -1, NULL, 0, NULL, NULL);
        char *path = bytes > 0 ? (char *)malloc((size_t)bytes) : NULL;
        if (!path || !WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, iso, -1, path, bytes, NULL, NULL)) {
            free(path); fputs("GamePKG: the selected ISO path is invalid or unavailable.\n", stderr); return 2;
        }
        result = gp_media_init((const uint8_t *)path, (size_t)bytes - 1, 64u * 1024u * 1024u);
        free(path);
    }
    if (result) { fprintf(stderr, "GamePKG: disc admission failed (%d).\n", result); return 3; }
    fputs("GamePKG: verified disc admitted in place; 716 files, read-only.\n", stderr);
    return gamepkg_original_main(argc, argv);
}
