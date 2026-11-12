/* Asset-free test of the actual native entry/admission boundary. */
#define main gpw_entry_main
#include "../windows_entry.c"
#undef main
#include <assert.h>
static unsigned admitted, entered;
static int32_t admission_result;
static unsigned handle_admitted;
static uintptr_t expected_handle;
int32_t gp_media_init_handle(uintptr_t handle, uint64_t budget) {
    assert(handle == expected_handle && budget == 64u * 1024u * 1024u);
    handle_admitted++; return admission_result;
}
int32_t gp_media_init(const uint8_t *path, size_t length, uint64_t budget) {
    const char expected[] = "C:\\caf\xc3\xa9\\disc.iso";
    assert(length == sizeof expected - 1 && !memcmp(path, expected, length));
    assert(budget == 64u * 1024u * 1024u);
    admitted++; return admission_result;
}
int gamepkg_original_main(int argc, char **argv) {
    assert(argc == 2 && !strcmp(argv[1], "/gamepkg-disc/default.xbe"));
    entered++; return 37;
}
int main(void) {
    char name[] = "game.exe", xbe[] = "/gamepkg-disc/default.xbe";
    char *args[] = {name, xbe, NULL};
    assert(SetEnvironmentVariableW(L"GAMEPKG_TAKEDAWN_DISC_HANDLE", NULL));
    assert(!_wputenv_s(L"GAMEPKG_TAKEDAWN_DISC", L""));
    assert(gpw_entry_main(2, args) == 2 && !admitted && !entered);
    char info[] = "--gamepkg-runtime-info"; char *info_args[] = {name, info, NULL};
    assert(gpw_entry_main(2, info_args) == 0 && !admitted && !entered);
    const char *blocked[] = {"--install", "--data", "--data-status"};
    for (unsigned i = 0; i < sizeof blocked / sizeof *blocked; i++) {
        char option[32]; strcpy(option, blocked[i]); char *options[] = {name, option, NULL};
        assert(gpw_entry_main(2, options) == 2 && !admitted && !entered);
    }
    assert(!_wputenv_s(L"GAMEPKG_TAKEDAWN_DISC", L"C:\\caf\x00e9\\disc.iso"));
    admission_result = -5;
    assert(gpw_entry_main(2, args) == 3 && admitted == 1 && !entered);
    admission_result = 0;
    assert(gpw_entry_main(2, args) == 37 && admitted == 2 && entered == 1);
    const WCHAR invalid[] = {L'C', L':', L'\\', 0xD800, 0};
    assert(!_wputenv_s(L"GAMEPKG_TAKEDAWN_DISC", invalid));
    assert(gpw_entry_main(2, args) == 2 && admitted == 2 && entered == 1);
    assert(!_wputenv_s(L"GAMEPKG_TAKEDAWN_DISC", L""));
    /* A malformed capability must never silently reopen a valid legacy path. */
    assert(!_wputenv_s(L"GAMEPKG_TAKEDAWN_DISC", L"C:\\caf\x00e9\\disc.iso"));
    const WCHAR *bad[] = {L"", L"0", L"-1", L"+12", L" 12", L"12 ", L"0x12", L"12x",
        L"18446744073709551615", L"18446744073709551616",
        L"99999999999999999999999999999999999999999999999999999999999999999999999"};
    for (unsigned i = 0; i < sizeof bad / sizeof *bad; i++) {
        assert(SetEnvironmentVariableW(L"GAMEPKG_TAKEDAWN_DISC_HANDLE", bad[i]));
        assert(gpw_entry_main(2, args) == 2 && admitted == 2 && !handle_admitted && entered == 1);
    }
    HANDLE file = CreateFileW(L"entry-handle-oracle.bin", GENERIC_READ | GENERIC_WRITE,
        0, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_DELETE_ON_CLOSE, NULL);
    assert(file != INVALID_HANDLE_VALUE);
    WCHAR handle_text[64]; expected_handle = (uintptr_t)file;
    swprintf(handle_text, 64, L"%llu", (unsigned long long)expected_handle);
    assert(SetEnvironmentVariableW(L"GAMEPKG_TAKEDAWN_DISC_HANDLE", handle_text));
    admission_result = -5;
    assert(gpw_entry_main(2, args) == 3 && admitted == 2 && handle_admitted == 1 && entered == 1);
    DWORD flags;
    assert(GetHandleInformation(file, &flags));
    admission_result = 0;
    assert(gpw_entry_main(2, args) == 37 && admitted == 2 && handle_admitted == 2 && entered == 2);
    assert(!GetHandleInformation(file, &flags) && GetLastError() == ERROR_INVALID_HANDLE);
    assert(SetEnvironmentVariableW(L"GAMEPKG_TAKEDAWN_DISC_HANDLE", NULL));
    assert(!_wputenv_s(L"GAMEPKG_TAKEDAWN_DISC", L""));
    puts("PASS: original Windows entry runs only after disc admission; Unicode ISO paths, failure recovery and extraction overrides checked");
    return 0;
}
