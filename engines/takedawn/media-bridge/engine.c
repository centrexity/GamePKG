/* Takedawn's native entry with GamePKG's read-only, in-place disc transport.
 * Process isolation belongs to the launcher, not this file adapter. */
#include "media.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <SDL.h>
extern int __real_main(int, char **);
extern FILE *__wrap_fopen(const char *, const char *);
int __wrap_main(int argc, char **argv) {
    if (argc == 2 && !strcmp(argv[1], "--gamepkg-runtime-info")) {
        puts("{\"abi\":1,\"implementation\":\"takedawn\",\"game_commit\":\"5a00fc10f9548a189e9043c5a55975863ad49175\",\"toolkit_commit\":\"5ae8ad90d984425e834dee759592c15592679880\",\"disc_access\":\"verified-in-place\"}");
        return 0;
    }
    /* The upstream installer extracts media. GamePKG admits only the selected
     * image; it never runs that installer or accepts a substitute data folder. */
    for (int i = 1; i < argc; ++i) {
        if (!strcmp(argv[i], "--install") || !strcmp(argv[i], "--data") ||
            !strcmp(argv[i], "--data-status")) {
            fputs("GamePKG reads the selected ISO in place; installation and data-folder overrides are unavailable.\n", stderr);
            return 2;
        }
    }
    const char *iso = getenv("GAMEPKG_TAKEDAWN_DISC");
    if (!iso || !*iso) {
        fprintf(stderr, "GamePKG: select the supported Xbox ISO before starting Takedawn.\n");
        return 2;
    }
    int32_t rc = gp_media_init((const uint8_t *)iso, strlen(iso), 64u * 1024u * 1024u);
    if (rc) { fprintf(stderr, "GamePKG: disc admission failed (%d).\n", rc); return 3; }
    fprintf(stderr, "GamePKG: verified disc admitted in place; 716 files, read-only.\n");
    /* The original main owns process lifetime and may exit from its game thread.
     * OS teardown drops media resources; do not close handles still owned by it. */
    return __real_main(argc, argv);
}
extern SDL_RWops *__real_SDL_RWFromFile(const char *, const char *);
SDL_RWops *__wrap_SDL_RWFromFile(const char *path, const char *mode) {
    const char *roots[] = {"/gamepkg-disc", "Burnout 3 Takedown"};
    for (unsigned i = 0; i < sizeof roots / sizeof *roots; i++) {
        size_t n = strlen(roots[i]);
        if (!strncmp(path, roots[i], n) && (!path[n] || path[n] == '/')) {
            FILE *file = __wrap_fopen(path, mode);
            if (!file) return NULL;
            SDL_RWops *rw = SDL_RWFromFP(file, SDL_TRUE);
            if (!rw) fclose(file);
            return rw;
        }
    }
    return __real_SDL_RWFromFile(path, mode);
}
