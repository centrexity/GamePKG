/* Called by the pinned upstream LAST exit handler, after normal save/audio/
 * renderer teardown. The Rust session owns the next process and retained ISO. */
#ifndef GAMEPKG_WINDOWS_RESTART_H
#define GAMEPKG_WINDOWS_RESTART_H
#include <windows.h>
#include <stdio.h>
#define GAMEPKG_RESTART_EXIT 0x47505253u
static inline void gpw_restart_at_exit(void) {
    WCHAR mode[2] = {0};
    if (GetEnvironmentVariableW(L"GAMEPKG_TAKEDAWN_MANAGED_RESTART", mode, 2) == 1 && mode[0] == L'1') {
        fputs("GamePKG: normal shutdown complete; requesting managed restart.\n", stderr);
        fflush(NULL);
        ExitProcess(GAMEPKG_RESTART_EXIT);
    }
}
#endif
