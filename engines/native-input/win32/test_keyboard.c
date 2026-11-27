/* Real shared Rust service and Windows message types; only this pump's
 * Windows-logo modifier query is fake. No physical key injection occurs. */
#include <stdlib.h>
#include <stdio.h>
#include <windows.h>
#include <stdint.h>
#define CHECK(expression) do { if (!(expression)) { fprintf(stderr, "FAIL line %d: %s\n", __LINE__, #expression); exit(17); } } while (0)
static int logo_held;
static MSG next_keyboard_message;
static BOOL next_keyboard_available;
static BOOL fake_peek(MSG *out, HWND owner, UINT first, UINT last, UINT remove) {
    CHECK(out && owner == (HWND)(uintptr_t)1 && first == WM_KEYFIRST &&
          last == WM_KEYLAST && remove == PM_NOREMOVE);
    if (!next_keyboard_available) return FALSE;
    *out = next_keyboard_message;
    return TRUE;
}
static SHORT fake_key_state(int vk) {
    CHECK(vk == VK_LWIN || vk == VK_RWIN);
    return logo_held ? (SHORT)0x8000 : 0;
}
#define GAMEPKG_WIN32_GET_KEY_STATE fake_key_state
#define GAMEPKG_WIN32_PEEK_MESSAGE fake_peek
#ifndef GAMEPKG_WIN32_PROVIDER_SOURCE
#define GAMEPKG_WIN32_PROVIDER_SOURCE "gamepkg_win32_keyboard.c"
#endif
#include GAMEPKG_WIN32_PROVIDER_SOURCE
#include <xinput.h>
static XINPUT_STATE physical_pad;
static uint64_t pad_time = 1;
static unsigned foreground_queries;
static DWORD fake_pad_get(DWORD port, XINPUT_STATE *state) {
    if (port != 3) return ERROR_DEVICE_NOT_CONNECTED;
    *state = physical_pad;
    return ERROR_SUCCESS;
}
static DWORD fake_pad_set(DWORD port, XINPUT_VIBRATION *motors) {
    CHECK(port == 3 && motors);
    return ERROR_SUCCESS;
}
static uint64_t fake_pad_time(void) { return ++pad_time; }
static HWND fake_foreground(void) { foreground_queries++; return (HWND)(uintptr_t)1; }
static DWORD fake_foreground_process(HWND window, LPDWORD process) {
    CHECK(window == (HWND)(uintptr_t)1 && process);
    *process = GetCurrentProcessId();
    return GetCurrentThreadId();
}
#define GAMEPKG_XINPUT_GET_STATE fake_pad_get
#define GAMEPKG_XINPUT_SET_STATE fake_pad_set
#define GAMEPKG_XINPUT_TIME_NS fake_pad_time
#define GetForegroundWindow fake_foreground
#define GetWindowThreadProcessId fake_foreground_process
#include "gamepkg_xinput.h"
#undef GetForegroundWindow
#undef GetWindowThreadProcessId

static LPARAM scan(unsigned code, int extended) {
    return (LPARAM)((code << 16) | (extended ? 1u << 24 : 0u));
}
static int key(UINT m, WPARAM vk, unsigned code, int extended) {
    return gamepkg_win32_keyboard_message(m, vk, scan(code, extended));
}
static uint64_t keyboard_id(void) {
    gamepkg_input_device devices[GAMEPKG_INPUT_MAX_DEVICES];
    int count = gamepkg_input_devices(devices, GAMEPKG_INPUT_MAX_DEVICES);
    CHECK(count >= 0);
    for (int i = 0; i < count; ++i)
        if (devices[i].backend == 4 && devices[i].kind == GAMEPKG_INPUT_KEYBOARD)
            return devices[i].device;
    return 0;
}
static gamepkg_input_event events[GAMEPKG_INPUT_MAX_EVENTS];
static unsigned drain(void) {
    int n = gamepkg_input_read(events, GAMEPKG_INPUT_MAX_EVENTS);
    CHECK(n >= 0);
    return (unsigned)n;
}
static unsigned count_kind(uint32_t kind) {
    unsigned n = drain(), result = 0;
    for (unsigned i = 0; i < n; i++) result += events[i].kind == kind;
    return result;
}
static void expect_text(const uint32_t *codes, unsigned count) {
    unsigned n = drain(), found = 0;
    for (unsigned i = 0; i < n; i++) if (events[i].kind == GAMEPKG_INPUT_TEXT) {
        CHECK(found < count && events[i].code == codes[found]);
        found++;
    }
    CHECK(found == count);
}
static volatile LONG stop_readers;
static DWORD WINAPI reader(void *argument) {
    (void)argument;
    while (!InterlockedCompareExchange(&stop_readers, 0, 0)) {
        int held = gamepkg_win32_keyboard_held('A');
        CHECK(held == 0 || held == 1);
        CHECK(gamepkg_win32_keyboard_label('A', (uint32_t)scan(0x1e, 0)) == 'A');
    }
    return 0;
}
static DWORD WINAPI changing_window(void *argument) {
    (void)argument;
    for (unsigned i = 0; i < 200; i++) {
        gamepkg_win32_keyboard_focus(0);
        gamepkg_win32_keyboard_shutdown();
        gamepkg_win32_keyboard_focus(1);
        key(WM_KEYUP, 'A', 0x1e, 0);
    }
    return 0;
}
static void concurrent_lifetime(void) {
    HANDLE readers[4];
    gamepkg_win32_keyboard_shutdown();
    gamepkg_win32_keyboard_focus(1);
    stop_readers = 0;
    for (unsigned i = 0; i < 4; i++) {
        readers[i] = CreateThread(NULL, 0, reader, NULL, 0, NULL);
        CHECK(readers[i]);
    }
    HANDLE window = CreateThread(NULL, 0, changing_window, NULL, 0, NULL);
    CHECK(window);
    for (unsigned i = 0; i < 2000; i++) {
        key(WM_KEYDOWN, 'A', 0x1e, 0);
        key(WM_KEYUP, 'A', 0x1e, 0);
    }
    CHECK(WaitForSingleObject(window, 10000) == WAIT_OBJECT_0);
    CHECK(CloseHandle(window));
    InterlockedExchange(&stop_readers, 1);
    CHECK(WaitForMultipleObjects(4, readers, TRUE, 10000) == WAIT_OBJECT_0);
    for (unsigned i = 0; i < 4; i++) CHECK(CloseHandle(readers[i]));
    gamepkg_win32_keyboard_focus(1);
    key(WM_KEYUP, 'A', 0x1e, 0);
    CHECK(key(WM_KEYDOWN, 'A', 0x1e, 0) == GAMEPKG_WIN32_KEY_ACCEPT);
    CHECK(gamepkg_win32_keyboard_held('A'));
    gamepkg_win32_keyboard_shutdown();
    CHECK(!keyboard_id());
}
static void window_focus_authority(void) {
    XINPUT_STATE state;
    XINPUT_VIBRATION vibration = {1, 2};
    gamepkg_win32_keyboard_shutdown();
    /* With no window provider, retain the existing headless fallback. */
    CHECK(gamepkg_xinput_get_state(0, &state) == ERROR_SUCCESS);
    physical_pad.Gamepad.wButtons = XINPUT_GAMEPAD_A;
    physical_pad.Gamepad.sThumbLX = 12345;
    CHECK(gamepkg_xinput_get_state(0, &state) == ERROR_SUCCESS);
    CHECK(state.Gamepad.wButtons == XINPUT_GAMEPAD_A && state.Gamepad.sThumbLX == 12345);
    CHECK(gamepkg_xinput_set_state(0, &vibration) == ERROR_SUCCESS);
    /* The fake foreground still belongs to this process. The game window
     * has lost focus; repeated controller polls must not undo that event. */
    gamepkg_win32_keyboard_focus(0);
    unsigned queries = foreground_queries;
    for (unsigned i = 0; i < 100; i++) {
        CHECK(gamepkg_xinput_get_state(0, &state) == ERROR_SUCCESS);
        CHECK(!state.Gamepad.wButtons && !state.Gamepad.sThumbLX);
    }
    CHECK(foreground_queries == queries);
    gamepkg_win32_keyboard_focus(1);
    CHECK(gamepkg_xinput_get_state(0, &state) == ERROR_SUCCESS);
    CHECK(!state.Gamepad.wButtons); /* A held across focus needs release. */
    physical_pad.Gamepad.wButtons = 0;
    gamepkg_xinput_get_state(0, &state);
    physical_pad.Gamepad.wButtons = XINPUT_GAMEPAD_A;
    CHECK(gamepkg_xinput_get_state(0, &state) == ERROR_SUCCESS);
    CHECK(state.Gamepad.wButtons == XINPUT_GAMEPAD_A);
    gamepkg_win32_keyboard_shutdown();
    gamepkg_xinput_shutdown();
}
static void altgr_queue(void) {
    MSG ctrl = {0};
    ctrl.hwnd = (HWND)(uintptr_t)1;
    ctrl.message = WM_KEYDOWN;
    ctrl.wParam = VK_CONTROL;
    ctrl.lParam = scan(0x1d, 0);
    ctrl.time = 123;
    next_keyboard_message = ctrl;
    next_keyboard_message.message = WM_SYSKEYDOWN;
    next_keyboard_message.wParam = VK_MENU;
    next_keyboard_message.lParam = scan(0x38, 1);
    next_keyboard_available = TRUE;
    CHECK(gamepkg_win32_keyboard_skip_altgr(&ctrl));
    next_keyboard_message.time++;
    CHECK(!gamepkg_win32_keyboard_skip_altgr(&ctrl));
    next_keyboard_message.time--;
    next_keyboard_message.hwnd = (HWND)(uintptr_t)2;
    CHECK(!gamepkg_win32_keyboard_skip_altgr(&ctrl));
    next_keyboard_message.hwnd = ctrl.hwnd;
    next_keyboard_message.lParam = scan(0x38, 0);
    CHECK(!gamepkg_win32_keyboard_skip_altgr(&ctrl));
    next_keyboard_message.lParam = scan(0x38, 1);
    ctrl.lParam = scan(0x1d, 1);
    CHECK(!gamepkg_win32_keyboard_skip_altgr(&ctrl));
    ctrl.lParam = scan(0x1d, 0);
    ctrl.message = WM_KEYUP;
    CHECK(!gamepkg_win32_keyboard_skip_altgr(&ctrl));
    next_keyboard_message.message = WM_SYSKEYUP;
    CHECK(!gamepkg_win32_keyboard_skip_altgr(&ctrl));
    next_keyboard_available = FALSE;
    CHECK(!gamepkg_win32_keyboard_skip_altgr(&ctrl));
    CHECK(!gamepkg_win32_keyboard_skip_altgr(NULL));
}
int main(void) {
    gamepkg_win32_keyboard_shutdown();
    drain();
    CHECK(key(WM_KEYDOWN, 'W', 0x11, 0) == GAMEPKG_WIN32_KEY_DROP);
    uint64_t first = keyboard_id();
    CHECK(first && !gamepkg_win32_keyboard_held('W'));
    gamepkg_win32_keyboard_focus(1);
    CHECK(key(WM_KEYDOWN, 'W', 0x11, 0) == GAMEPKG_WIN32_KEY_DROP);
    key(WM_KEYUP, 'W', 0x11, 0);
    CHECK(key(WM_KEYDOWN, 'W', 0x11, 0) == GAMEPKG_WIN32_KEY_ACCEPT);
    CHECK(gamepkg_input_key_state(first, 26) == 1);
    CHECK(gamepkg_win32_keyboard_held('W'));
    drain();
    for (unsigned i = 0; i < 100; i++) {
        CHECK(key(WM_KEYDOWN, 'W', 0x11, 0) == GAMEPKG_WIN32_KEY_DROP);
        CHECK(gamepkg_win32_keyboard_message(WM_KEYDOWN, 'W', scan(0x11, 0) | (LPARAM)(1u << 30)) == GAMEPKG_WIN32_KEY_DROP);
    }
    CHECK(count_kind(GAMEPKG_INPUT_KEY_DOWN) == 0);

    key(WM_KEYDOWN, VK_LSHIFT, 0x2a, 0);
    key(WM_KEYDOWN, VK_RSHIFT, 0x36, 0);
    CHECK(gamepkg_input_key_state(first, 225) == 1);
    CHECK(gamepkg_input_key_state(first, 229) == 1);
    CHECK(gamepkg_win32_keyboard_held(VK_SHIFT));
    CHECK(gamepkg_win32_keyboard_label(VK_LSHIFT, (uint32_t)scan(0x2a, 0)) == VK_SHIFT);
    CHECK(gamepkg_win32_keyboard_label(VK_RSHIFT, (uint32_t)scan(0x36, 0)) == VK_SHIFT);
    key(WM_KEYUP, VK_LSHIFT, 0x2a, 0);
    CHECK(gamepkg_win32_keyboard_held(VK_SHIFT));
    key(WM_KEYUP, VK_RSHIFT, 0x36, 0);
    CHECK(!gamepkg_win32_keyboard_held(VK_SHIFT));
    key(WM_KEYDOWN, VK_CONTROL, 0x1d, 0);
    key(WM_KEYDOWN, VK_CONTROL, 0x1d, 1);
    CHECK(gamepkg_input_key_state(first, 224) == 1);
    CHECK(gamepkg_input_key_state(first, 228) == 1);
    key(WM_KEYUP, VK_CONTROL, 0x1d, 0);
    CHECK(gamepkg_win32_keyboard_held(VK_CONTROL));
    key(WM_KEYUP, VK_CONTROL, 0x1d, 1);
    CHECK(!gamepkg_win32_keyboard_held(VK_CONTROL));

    key(WM_KEYDOWN, VK_RETURN, 0x1c, 0);
    key(WM_KEYDOWN, VK_RETURN, 0x1c, 1);
    gamepkg_win32_keyboard_release(VK_RETURN, (uint32_t)scan(0x1c, 0));
    CHECK(gamepkg_win32_keyboard_held(VK_RETURN));
    CHECK(gamepkg_win32_keyboard_held('W')); /* Shortcut does not cancel gas. */
    gamepkg_win32_keyboard_release(VK_RETURN, (uint32_t)scan(0x1c, 1));
    CHECK(!gamepkg_win32_keyboard_held(VK_RETURN));
    key(WM_KEYUP, VK_RETURN, 0x1c, 0);
    key(WM_KEYUP, VK_RETURN, 0x1c, 1);
    key(WM_KEYDOWN, VK_RETURN, 0x1c, 0);
    CHECK(gamepkg_win32_keyboard_held(VK_RETURN));
    key(WM_KEYUP, VK_RETURN, 0x1c, 0);

    key(WM_KEYDOWN, 'A', 0x1e, 0);
    CHECK(key(WM_KEYDOWN, 'Q', 0x1e, 0) == GAMEPKG_WIN32_KEY_DROP);
    CHECK(gamepkg_win32_keyboard_label('Q', (uint32_t)scan(0x1e, 0)) == 'A');
    CHECK(gamepkg_win32_keyboard_held('A') && !gamepkg_win32_keyboard_held('Q'));
    key(WM_KEYUP, 'Q', 0x1e, 0); /* Layout changed while physical A was held. */
    CHECK(!gamepkg_win32_keyboard_held('A'));
    key(WM_KEYDOWN, 'Q', 0x1e, 0);
    CHECK(gamepkg_win32_keyboard_held('Q') && !gamepkg_win32_keyboard_held('A'));
    key(WM_KEYUP, 'Q', 0x1e, 0);
    key(WM_KEYDOWN, 'A', 0x9e, 0); /* Windows on-screen break-bit quirk. */
    CHECK(gamepkg_input_key_state(first, 4) == 1);
    key(WM_KEYUP, 'A', 0x1e, 0);

    CHECK(gamepkg_win32_key_usage(VK_NUMLOCK, (uint32_t)scan(0x45, 1)) == 83);
    CHECK(gamepkg_win32_key_usage(VK_PAUSE, (uint32_t)scan(0x45, 0)) == 72);
    CHECK(gamepkg_win32_key_usage(VK_F24, (uint32_t)scan(0x76, 0)) == 115);
    CHECK(gamepkg_win32_key_usage(VK_F24, (uint32_t)scan(0x6f, 0)) == 0);
    CHECK(gamepkg_win32_key_usage(VK_PACKET, (uint32_t)scan(0x1e, 0)) == 0);
    CHECK(gamepkg_win32_key_usage('A', 0) == 0);
    for (unsigned code = 0; code < 256; code++) for (int ext = 0; ext < 2; ext++)
        CHECK(gamepkg_win32_key_usage('A', (uint32_t)scan(code, ext)) < 512);

    drain();
    gamepkg_win32_keyboard_message(WM_CHAR, 'A', 0);
    gamepkg_win32_keyboard_message(WM_CHAR, 0xd83d, 0);
    gamepkg_win32_keyboard_message(WM_CHAR, 0xde00, 0);
    gamepkg_win32_keyboard_message(WM_CHAR, 0xdc00, 0); /* Isolated low surrogate. */
    gamepkg_win32_keyboard_message(WM_CHAR, 0xd800, 0);
    gamepkg_win32_keyboard_message(WM_CHAR, 'B', 0); /* Abandon invalid pair. */
    gamepkg_win32_keyboard_message(WM_CHAR, 0x10041, 0);
    gamepkg_win32_keyboard_message(WM_CHAR, 0xdc00, 0);
    gamepkg_win32_keyboard_message(WM_UNICHAR, UNICODE_NOCHAR, 0);
    gamepkg_win32_keyboard_message(WM_UNICHAR, 0x10ffff, 0);
    gamepkg_win32_keyboard_message(WM_UNICHAR, 0xd800, 0);
    gamepkg_win32_keyboard_message(WM_UNICHAR, 0x110000, 0);
    const uint32_t text_codes[] = {'A', 0x1f600, 'B', 0x10ffff};
    expect_text(text_codes, 4);
    gamepkg_win32_keyboard_message(WM_CHAR, 0xd800, 0);
    gamepkg_win32_keyboard_focus(0);
    CHECK(!gamepkg_win32_keyboard_held('W'));
    gamepkg_win32_keyboard_focus(1);
    CHECK(!gamepkg_win32_keyboard_held('W'));
    gamepkg_win32_keyboard_message(WM_CHAR, 0xdc00, 0);
    expect_text(NULL, 0);
    CHECK(key(WM_KEYDOWN, 'W', 0x11, 0) == GAMEPKG_WIN32_KEY_DROP);
    key(WM_KEYUP, 'W', 0x11, 0);
    key(WM_KEYDOWN, 'W', 0x11, 0);

    uint64_t pad = gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD, 77, UINT64_C(0x100000001));
    gamepkg_input_gamepad report = {0}, state = {0};
    report.device = pad;
    gamepkg_input_update_gamepad(&report);
    report.buttons = GAMEPKG_INPUT_PAD_SOUTH;
    gamepkg_input_update_gamepad(&report);
    CHECK(gamepkg_input_gamepad_state(0, &state) == 1 && state.buttons == report.buttons);
    CHECK(key(WM_KEYDOWN, VK_LWIN, 0x5b, 1) == GAMEPKG_WIN32_KEY_RESET);
    CHECK(!gamepkg_win32_keyboard_held('W'));
    CHECK(gamepkg_input_gamepad_state(0, &state) == 1 && state.buttons == report.buttons);
    logo_held = 1;
    CHECK(key(WM_KEYDOWN, 'A', 0x1e, 0) == GAMEPKG_WIN32_KEY_DROP);
    CHECK(!gamepkg_win32_keyboard_held('A'));
    logo_held = 0;
    key(WM_KEYUP, VK_LWIN, 0x5b, 1);
    key(WM_KEYUP, 'W', 0x11, 0);
    key(WM_KEYDOWN, 'W', 0x11, 0);
    for (unsigned i = 0; i < 3000; i++) {
        key(WM_KEYDOWN, 'A', 0x1e, 0);
        key(WM_KEYUP, 'A', 0x1e, 0);
    }
    CHECK(gamepkg_win32_keyboard_held('W'));
    CHECK(count_kind(GAMEPKG_INPUT_RESET) == 1);
    CHECK(gamepkg_win32_keyboard_held('W'));
    gamepkg_win32_keyboard_shutdown();
    CHECK(!keyboard_id());
    CHECK(gamepkg_input_gamepad_state(0, &state) == 1 && !state.buttons);
    gamepkg_input_disconnect(pad);
    gamepkg_win32_keyboard_focus(1);
    key(WM_KEYDOWN, 'A', 0x1e, 0);
    CHECK(keyboard_id() > first && gamepkg_win32_keyboard_held('A'));
    gamepkg_win32_keyboard_shutdown();
    concurrent_lifetime();
    window_focus_authority();
    altgr_queue();
    puts("PASS native Windows keyboard with real Rust service: physical/label state, paired modifiers/Enter, shortcut/focus, Unicode, queue overflow, device generation, concurrent readers/focus/shutdown and XInput window-focus authority");
    return 0;
}
