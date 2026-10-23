/* Asset-free test of the actual native entry/admission boundary. */
#define main gpw_entry_main
#include "../windows_entry.c"
#undef main
#include <assert.h>
static unsigned admitted, entered;
static int32_t admission_result;
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
    puts("PASS: original Windows entry runs only after disc admission; Unicode ISO paths, failure recovery and extraction overrides checked");
    return 0;
}
