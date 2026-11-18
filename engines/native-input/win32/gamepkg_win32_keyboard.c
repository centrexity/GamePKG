/* SPDX-License-Identifier: Apache-2.0 */
#include "gamepkg_win32_keyboard.h"
#include "../../../include/gamepkg_input_backend.h"
#include <string.h>
#ifndef GAMEPKG_WIN32_GET_KEY_STATE
#define GAMEPKG_WIN32_GET_KEY_STATE GetKeyState
#endif
#ifndef GAMEPKG_WIN32_PEEK_MESSAGE
#define GAMEPKG_WIN32_PEEK_MESSAGE PeekMessageW
#endif
/* Backend 4 is the native Windows window provider; XInput uses backend 3. */
static uint64_t keyboard;
static uint32_t labels[512];
static uint32_t high_surrogate;
static int focus_managed;
static SRWLOCK keyboard_lock = SRWLOCK_INIT;

int gamepkg_win32_keyboard_skip_altgr(const MSG *message) {
    if (!message || message->wParam != VK_CONTROL ||
        ((uint32_t)message->lParam & (1u << 24)) ||
        (((uint32_t)message->lParam >> 16) & 127u) != 0x1d) return 0;
    /* Never suppress releases: an actual Ctrl must not remain held when
     * the user releases Ctrl and Alt together. */
    if (message->message != WM_KEYDOWN && message->message != WM_SYSKEYDOWN) return 0;
    MSG next = {0};
    /* No adapter lock is held: PeekMessage can dispatch pending sent messages. */
    if (!GAMEPKG_WIN32_PEEK_MESSAGE(&next, message->hwnd, WM_KEYFIRST, WM_KEYLAST, PM_NOREMOVE))
        return 0;
    int next_down = next.message == WM_KEYDOWN || next.message == WM_SYSKEYDOWN;
    return next.hwnd == message->hwnd && next.time == message->time &&
        next.wParam == VK_MENU && next_down &&
        ((uint32_t)next.lParam & (1u << 24)) &&
        (((uint32_t)next.lParam >> 16) & 127u) == 0x38;
}

uint32_t gamepkg_win32_key_usage(uint32_t vk, uint32_t flags) {
    /* Windows set-1 scan codes to keyboard-page HID usages. Physical codes
     * remain independent of the current keyboard layout. References:
     * learn.microsoft.com/windows/win32/inputdev/about-keyboard-input
     * github.com/libsdl-org/SDL/blob/main/src/events/scancodes_windows.h */
    static const uint8_t base[128] = {
        0,41,30,31,32,33,34,35,36,37,38,39,45,46,42,43, /* 0x00..0x0f */
        20,26,8,21,23,28,24,12,18,19,47,48,40,224,4,22, /* 0x10..0x1f */
        7,9,10,11,13,14,15,51,52,53,225,49,29,27,6,25, /* 0x20..0x2f */
        5,17,16,54,55,56,229,85,226,44,57,58,59,60,61,62, /* 0x30..0x3f */
        63,64,65,66,67,83,71,95,96,97,86,92,93,94,87,89, /* 0x40..0x4f */
        90,91,98,99,0,0,100,68,69,103,0,0,140,0,0,0, /* 0x50..0x5f */
        0,0,0,0,104,105,106,107,108,109,110,111,112,113,114,0, /* 0x60..0x6f */
        136,145,144,135,0,0,115,147,146,138,0,139,0,137,133,0, /* 0x70..0x7f */
    };
    if (vk > 255 || vk == VK_PACKET) return 0;
    if (vk == VK_PAUSE) return 72; /* Pause has an E1 sequence, not E0 NumLock. */
    /* Windows' on-screen keyboard can put the set-1 break bit here. The
     * message kind determines up/down; E0 is already a separate flag. */
    unsigned scan = (flags >> 16) & 127u;
    if (flags & (1u << 24)) {
        switch (scan) {
        case 0x1c:return 88; case 0x1d:return 228; case 0x35:return 84;
        case 0x37:return 70; case 0x38:return 230; case 0x45:return 83;
        case 0x46:return 72; case 0x47:return 74;
        case 0x48:return 82; case 0x49:return 75; case 0x4b:return 80;
        case 0x4d:return 79; case 0x4f:return 77; case 0x50:return 81;
        case 0x51:return 78; case 0x52:return 73; case 0x53:return 76;
        case 0x5b:return 227; case 0x5c:return 231; case 0x5d:return 101;
        default:return 0;
        }
    }
    return scan < sizeof base ? base[scan] : 0;
}
static uint64_t device(void) {
    if (!keyboard) keyboard = gamepkg_input_connect(GAMEPKG_INPUT_KEYBOARD, 4, 0);
    return keyboard;
}
static int submit(uint32_t kind, uint32_t code) {
    gamepkg_input_event e = {0};
    e.device = device();
    if (!e.device) return 0;
    e.timestamp_ns = gamepkg_input_time_ns();
    e.kind = kind;
    e.code = code;
    return gamepkg_input_submit(&e);
}
static uint32_t group(uint32_t vk) {
    switch (vk) {
    case VK_LSHIFT:case VK_RSHIFT:return VK_SHIFT;
    case VK_LCONTROL:case VK_RCONTROL:return VK_CONTROL;
    case VK_LMENU:case VK_RMENU:return VK_MENU;
    default:return vk;
    }
}
uint32_t gamepkg_win32_keyboard_label(uint32_t vk, uint32_t flags) {
    AcquireSRWLockShared(&keyboard_lock);
    unsigned usage = gamepkg_win32_key_usage(vk, flags);
    uint32_t label = usage && labels[usage] ? labels[usage] : vk;
    ReleaseSRWLockShared(&keyboard_lock);
    return group(label);
}
static int held_locked(uint32_t vk) {
    if (!keyboard || vk > 255) return 0;
    vk = group(vk);
    for (unsigned usage = 1; usage < 512; usage++)
        if (labels[usage] && group(labels[usage]) == vk &&
            gamepkg_input_key_state(keyboard, usage) == 1) return 1;
    return 0;
}
int gamepkg_win32_keyboard_held(uint32_t vk) {
    AcquireSRWLockShared(&keyboard_lock);
    int held = held_locked(vk);
    ReleaseSRWLockShared(&keyboard_lock);
    return held;
}
void gamepkg_win32_keyboard_release(uint32_t vk, uint32_t flags) {
    AcquireSRWLockExclusive(&keyboard_lock);
    unsigned usage = gamepkg_win32_key_usage(vk, flags);
    if (usage && keyboard) submit(GAMEPKG_INPUT_KEY_UP, usage);
    ReleaseSRWLockExclusive(&keyboard_lock);
}
static void text(uint32_t code) {
    if (code && code <= 0x10ffff && !(code >= 0xd800 && code <= 0xdfff))
        submit(GAMEPKG_INPUT_TEXT, code);
}
static int message_locked(UINT m, WPARAM w, LPARAM l) {
    if (m == WM_CHAR) {
        uint32_t c = (uint32_t)w;
        if ((uint64_t)w > 0xffff) { high_surrogate = 0; return GAMEPKG_WIN32_KEY_DROP; }
        if (c >= 0xd800 && c <= 0xdbff) high_surrogate = c;
        else if (c >= 0xdc00 && c <= 0xdfff) {
            if (high_surrogate) text(0x10000 + ((high_surrogate-0xd800) << 10) + c-0xdc00);
            high_surrogate = 0;
        } else { high_surrogate = 0; text(c); }
        return GAMEPKG_WIN32_KEY_ACCEPT;
    }
    if (m == WM_UNICHAR) {
        high_surrogate = 0;
        if (w != UNICODE_NOCHAR && (uint64_t)w <= 0x10ffff) text((uint32_t)w);
        return GAMEPKG_WIN32_KEY_ACCEPT;
    }
    int down = m == WM_KEYDOWN || m == WM_SYSKEYDOWN;
    if (!down && m != WM_KEYUP && m != WM_SYSKEYUP) return GAMEPKG_WIN32_KEY_DROP;
    if ((uint64_t)w > 255) return GAMEPKG_WIN32_KEY_DROP;
    unsigned usage = gamepkg_win32_key_usage((uint32_t)w, (uint32_t)l);
    if (!usage) return GAMEPKG_WIN32_KEY_DROP;
    if (down) {
        if ((uint32_t)l & (1u << 30)) return GAMEPKG_WIN32_KEY_DROP;
        if (!device()) return GAMEPKG_WIN32_KEY_DROP;
        if (gamepkg_input_key_state(keyboard, usage) == 1) return GAMEPKG_WIN32_KEY_DROP;
        if (w == VK_LWIN || w == VK_RWIN) {
            gamepkg_input_reset_device(keyboard);
            high_surrogate = 0;
            return GAMEPKG_WIN32_KEY_RESET;
        }
        /* GetKeyState reads this pump's queued modifier state, not an extra
         * physical keyboard poll. Windows shortcuts must not drive a game. */
        if ((GAMEPKG_WIN32_GET_KEY_STATE(VK_LWIN) | GAMEPKG_WIN32_GET_KEY_STATE(VK_RWIN)) & 0x8000)
            return GAMEPKG_WIN32_KEY_DROP;
        labels[usage] = (uint32_t)w;
    }
    if (submit(down ? GAMEPKG_INPUT_KEY_DOWN : GAMEPKG_INPUT_KEY_UP, usage) != 1)
        return GAMEPKG_WIN32_KEY_DROP;
    return !down || gamepkg_input_key_state(keyboard, usage) == 1
        ? GAMEPKG_WIN32_KEY_ACCEPT : GAMEPKG_WIN32_KEY_DROP;
}
int gamepkg_win32_keyboard_message(UINT m, WPARAM w, LPARAM l) {
    AcquireSRWLockExclusive(&keyboard_lock);
    int accepted = message_locked(m, w, l);
    ReleaseSRWLockExclusive(&keyboard_lock);
    return accepted;
}
static void focus_locked(int focused) {
    focus_managed = 1;
    gamepkg_input_focus(focused ? 1u : 0u);
    if (!focused) high_surrogate = 0;
}
int gamepkg_win32_keyboard_manages_focus(void) {
    AcquireSRWLockShared(&keyboard_lock);
    int managed = focus_managed;
    ReleaseSRWLockShared(&keyboard_lock);
    return managed;
}
void gamepkg_win32_keyboard_focus(int focused) {
    AcquireSRWLockExclusive(&keyboard_lock);
    focus_locked(focused);
    ReleaseSRWLockExclusive(&keyboard_lock);
}
void gamepkg_win32_keyboard_shutdown(void) {
    AcquireSRWLockExclusive(&keyboard_lock);
    focus_locked(0);
    if (keyboard) gamepkg_input_disconnect(keyboard);
    keyboard = 0;
    high_surrogate = 0;
    focus_managed = 0;
    memset(labels, 0, sizeof labels);
    ReleaseSRWLockExclusive(&keyboard_lock);
}
