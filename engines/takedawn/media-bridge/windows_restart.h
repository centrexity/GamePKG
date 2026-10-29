/* Signal the owning Rust session, then allow ALL remaining exit handlers and
 * CRT cleanup to finish normally. The owner waits for process exit to restart. */
#ifndef GAMEPKG_WINDOWS_RESTART_H
#define GAMEPKG_WINDOWS_RESTART_H
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
static inline int gpw_restart_at_exit(void) {
    WCHAR text[64] = {0};
    SetLastError(ERROR_SUCCESS);
    DWORD length = GetEnvironmentVariableW(L"GAMEPKG_TAKEDAWN_RESTART_HANDLE", text, 64);
    if (!length && GetLastError() == ERROR_ENVVAR_NOT_FOUND) return 0;
    uintptr_t value = 0;
    if (!length || length >= 64) goto invalid;
    for (DWORD i = 0; i < length; i++) {
        if (text[i] < L'0' || text[i] > L'9') goto invalid;
        uintptr_t digit = (uintptr_t)(text[i] - L'0');
        if (value > (UINTPTR_MAX - digit) / 10) goto invalid;
        value = value * 10 + digit;
    }
    if (!value || value == UINTPTR_MAX || !SetEvent((HANDLE)value)) goto invalid;
    fputs("GamePKG: managed restart requested; normal exit pending.\n", stderr);
    fflush(NULL);
    return 1;
invalid:
    /* A broken managed signal must not create an untracked child or reopen
     * the ISO by filename. Its diagnostic remains in the owner's output log. */
    fputs("GamePKG: unable to signal the managed restart; game will close.\n", stderr);
    fflush(NULL);
    return 1;
}
#endif
