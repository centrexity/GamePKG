/* Actual upstream lifecycle fixture; no game source/assets are needed. */
#include <windows.h>
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "recomp_restart.h"
static const char *marker;
static HANDLE save_lock;
static void teardown(void) {
    assert(CloseHandle(save_lock));
    FILE *file = fopen(marker, "a"); assert(file);
    assert(fputs("teardown complete\n", file) >= 0);
    assert(!fclose(file));
    puts("FIXTURE: normal save/audio/window teardown completed");
    fflush(NULL);
}
int main(int argc, char **argv) {
    assert(argc == 3);
    recomp_restart_init(argc, argv); /* registers original last exit handler */
    marker = argv[2];
    save_lock = CreateFileA("save-lock.bin", GENERIC_READ | GENERIC_WRITE, 0, NULL,
        OPEN_ALWAYS, FILE_ATTRIBUTE_TEMPORARY, NULL);
    assert(save_lock != INVALID_HANDLE_VALUE);
    assert(!atexit(teardown));
    assert(recomp_restart_available());
    if (!strcmp(argv[1], "once")) {
        FILE *file = fopen(marker, "r");
        if (file) {
            char line[64]; assert(fgets(line, sizeof line, file));
            assert(!strcmp(line, "teardown complete\n"));
            assert(!fclose(file));
            puts("FIXTURE: restarted after completed teardown");
            return 0;
        }
    }
    if (!strcmp(argv[1], "quit")) {
        recomp_quit_request("ordinary quit");
        assert(!recomp_restart_requested());
        return 0;
    }
    recomp_restart_request("original lifecycle fixture");
    assert(recomp_restart_requested());
    if (!strcmp(argv[1], "cancel")) {
        recomp_restart_cancel();
        assert(!recomp_restart_requested());
    } else {
        assert(!strcmp(argv[1], "request") || !strcmp(argv[1], "once"));
    }
    return 0; /* all normal atexit handlers run before managed restart */
}
