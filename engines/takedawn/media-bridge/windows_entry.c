/* Host-owned native Windows entry. The original game main owns the window,
 * renderer, audio and game lifetime after read-only disc admission. */
#include "windows_file.h"
#include "media.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern int gamepkg_original_main(int, char **);
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
    const WCHAR *iso = _wgetenv(L"GAMEPKG_TAKEDAWN_DISC");
    if (!iso || !*iso) {
        fputs("GamePKG: select the supported Xbox ISO before starting Takedawn.\n", stderr); return 2;
    }
    int bytes = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, iso, -1, NULL, 0, NULL, NULL);
    char *path = bytes > 0 ? (char *)malloc((size_t)bytes) : NULL;
    if (!path || !WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, iso, -1, path, bytes, NULL, NULL)) {
        free(path); fputs("GamePKG: the selected ISO path is invalid or unavailable.\n", stderr); return 2;
    }
    int32_t result = gp_media_init((const uint8_t *)path, (size_t)bytes - 1, 64u * 1024u * 1024u);
    free(path);
    if (result) { fprintf(stderr, "GamePKG: disc admission failed (%d).\n", result); return 3; }
    fputs("GamePKG: verified disc admitted in place; 716 files, read-only.\n", stderr);
    return gamepkg_original_main(argc, argv);
}
