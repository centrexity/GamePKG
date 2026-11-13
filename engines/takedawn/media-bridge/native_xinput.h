/* SPDX-License-Identifier: Apache-2.0
 * Windows XInput adapter for the shared GamePKG service. The original Duke
 * mapping, keyboard/touch merge and rumble curve remain in the pinned engine. */
#ifndef GAMEPKG_NATIVE_XINPUT_H
#define GAMEPKG_NATIVE_XINPUT_H
#include <windows.h>
#include <xinput.h>
#include <string.h>
#include "../../../include/gamepkg_input_backend.h"
#ifndef GAMEPKG_XINPUT_GET_STATE
#define GAMEPKG_XINPUT_GET_STATE XInputGetState
#endif
#ifndef GAMEPKG_XINPUT_SET_STATE
#define GAMEPKG_XINPUT_SET_STATE XInputSetState
#endif
#ifndef GAMEPKG_XINPUT_TIME_NS
#define GAMEPKG_XINPUT_TIME_NS gamepkg_input_time_ns
#endif
#ifndef GAMEPKG_XINPUT_FOCUSED
static int gamepkg_xi_window_focused(void) {
    HWND window = GetForegroundWindow();
    DWORD process = 0;
    if (window) GetWindowThreadProcessId(window, &process);
    return process != 0 && process == GetCurrentProcessId();
}
#define GAMEPKG_XINPUT_FOCUSED gamepkg_xi_window_focused
#endif
#define GAMEPKG_XI_BACKEND 3u
#define GAMEPKG_XI_REPROBE_NS UINT64_C(250000000)
#define GAMEPKG_XI_ROUND_NS UINT64_C(1000000)
struct gamepkg_xi_slot {
    uint64_t device, probed_at;
    int probed, connected;
    XINPUT_STATE raw;
};
struct gamepkg_xi_logical {
    uint64_t device;
    DWORD source_packet, packet;
    XINPUT_GAMEPAD last;
    int valid;
};
static struct gamepkg_xi_slot gamepkg_xi_slots[4];
static struct gamepkg_xi_logical gamepkg_xi_logical[4];
static SRWLOCK gamepkg_xi_lock = SRWLOCK_INIT;
static uint64_t gamepkg_xi_polled_at;
static DWORD gamepkg_xi_last_port = 4;

/* Caller holds the adapter lock. A new/non-increasing logical read starts a
 * new polling round, so a repeated read of one player is never rate limited.
 * Increasing reads share the round only while it is under 1 ms old. Connected
 * pads keep the title's polling cadence; empty physical slots use backoff. */
static void gamepkg_xi_refresh(DWORD requested_port) {
    uint64_t now = GAMEPKG_XINPUT_TIME_NS();
    gamepkg_input_focus(GAMEPKG_XINPUT_FOCUSED() ? 1u : 0u);
    if (gamepkg_xi_polled_at && requested_port > gamepkg_xi_last_port &&
        now >= gamepkg_xi_polled_at && now - gamepkg_xi_polled_at < GAMEPKG_XI_ROUND_NS) {
        gamepkg_xi_last_port = requested_port;
        return;
    }
    gamepkg_xi_last_port = requested_port;
    gamepkg_xi_polled_at = now;
    for (DWORD physical = 0; physical < 4; physical++) {
        struct gamepkg_xi_slot *slot = &gamepkg_xi_slots[physical];
        if (slot->probed && !slot->connected && now >= slot->probed_at &&
            now - slot->probed_at < GAMEPKG_XI_REPROBE_NS) continue;
        XINPUT_STATE raw;
        memset(&raw, 0, sizeof raw);
        DWORD rc = GAMEPKG_XINPUT_GET_STATE(physical, &raw);
        slot->probed = 1;
        slot->probed_at = now;
        if (rc != ERROR_SUCCESS) {
            if (slot->device) gamepkg_input_disconnect(slot->device);
            slot->device = 0;
            slot->connected = 0;
            memset(&slot->raw, 0, sizeof slot->raw);
            continue;
        }
        slot->connected = 1;
        slot->raw = raw;
        if (!slot->device)
            slot->device = gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD, GAMEPKG_XI_BACKEND, physical);
        if (!slot->device) continue;
        gamepkg_input_gamepad report;
        memset(&report, 0, sizeof report);
        report.device = slot->device;
        report.timestamp_ns = now;
        report.buttons = raw.Gamepad.wButtons;
        report.left_x = raw.Gamepad.sThumbLX;
        report.left_y = raw.Gamepad.sThumbLY;
        report.right_x = raw.Gamepad.sThumbRX;
        report.right_y = raw.Gamepad.sThumbRY;
        report.left_trigger = (uint16_t)(raw.Gamepad.bLeftTrigger * 257u);
        report.right_trigger = (uint16_t)(raw.Gamepad.bRightTrigger * 257u);
        gamepkg_input_update_gamepad(&report);
    }
}
static int gamepkg_xi_physical(uint64_t device) {
    for (int physical = 0; physical < 4; physical++)
        if (gamepkg_xi_slots[physical].connected && gamepkg_xi_slots[physical].device == device)
            return physical;
    return -1;
}
/* The legacy engine probes disconnected logical ports only once a second.
 * Keep unclaimed physical controllers present as neutral slots so its normal
 * poll can observe the first button. Never invent additional controllers or
 * assign ownership by hardware slot order. Existing assignments stay put. */
static int gamepkg_xi_pending(DWORD requested_port) {
    uint64_t assigned[4] = {0, 0, 0, 0};
    unsigned pending = 0;
    for (DWORD player = 0; player < 4; player++) {
        gamepkg_input_gamepad report;
        if (gamepkg_input_gamepad_state(player, &report) == 1) assigned[player] = report.device;
    }
    for (DWORD physical = 0; physical < 4; physical++) {
        uint64_t device = gamepkg_xi_slots[physical].device;
        if (!device || !gamepkg_xi_slots[physical].connected) continue;
        int claimed = 0;
        for (DWORD player = 0; player < 4; player++) if (assigned[player] == device) claimed = 1;
        if (!claimed) ++pending;
    }
    for (DWORD player = 0; player < 4 && pending; player++) {
        if (assigned[player]) continue;
        if (player == requested_port) return 1;
        --pending;
    }
    return 0;
}
static DWORD gamepkg_xinput_get_state(DWORD port, XINPUT_STATE *out) {
    if (port >= 4) return ERROR_DEVICE_NOT_CONNECTED;
    if (!out) return ERROR_BAD_ARGUMENTS;
    AcquireSRWLockExclusive(&gamepkg_xi_lock);
    gamepkg_xi_refresh(port);
    gamepkg_input_gamepad report;
    int physical = -1;
    if (gamepkg_input_gamepad_state(port, &report) == 1)
        physical = gamepkg_xi_physical(report.device);
    if (physical < 0) {
        if (gamepkg_xi_pending(port)) {
            struct gamepkg_xi_logical *logical = &gamepkg_xi_logical[port];
            if (!logical->valid || logical->device != 0) ++logical->packet;
            logical->valid = 1;
            logical->device = 0;
            memset(out, 0, sizeof *out);
            out->dwPacketNumber = logical->packet;
            ReleaseSRWLockExclusive(&gamepkg_xi_lock);
            return ERROR_SUCCESS;
        }
        gamepkg_xi_logical[port].valid = 0;
        ReleaseSRWLockExclusive(&gamepkg_xi_lock);
        return ERROR_DEVICE_NOT_CONNECTED;
    }
    XINPUT_GAMEPAD pad;
    memset(&pad, 0, sizeof pad);
    pad.wButtons = (WORD)report.buttons;
    pad.bLeftTrigger = (BYTE)(report.left_trigger / 257u);
    pad.bRightTrigger = (BYTE)(report.right_trigger / 257u);
    pad.sThumbLX = report.left_x;
    pad.sThumbLY = report.left_y;
    pad.sThumbRX = report.right_x;
    pad.sThumbRY = report.right_y;
    struct gamepkg_xi_logical *logical = &gamepkg_xi_logical[port];
    DWORD source_packet = gamepkg_xi_slots[physical].raw.dwPacketNumber;
    /* Preserve packet-change semantics, including focus-neutral transitions
     * that change controls without changing the hardware packet number. */
    if (!logical->valid || logical->device != report.device ||
        logical->source_packet != source_packet || memcmp(&logical->last, &pad, sizeof pad)) {
        ++logical->packet;
        logical->valid = 1;
        logical->device = report.device;
        logical->source_packet = source_packet;
        logical->last = pad;
    }
    out->dwPacketNumber = logical->packet;
    out->Gamepad = pad;
    ReleaseSRWLockExclusive(&gamepkg_xi_lock);
    return ERROR_SUCCESS;
}
static DWORD gamepkg_xinput_set_state(DWORD port, const XINPUT_VIBRATION *vibration) {
    if (port >= 4) return ERROR_DEVICE_NOT_CONNECTED;
    if (!vibration) return ERROR_BAD_ARGUMENTS;
    AcquireSRWLockExclusive(&gamepkg_xi_lock);
    gamepkg_input_gamepad report;
    int physical = -1;
    if (gamepkg_input_gamepad_state(port, &report) == 1)
        physical = gamepkg_xi_physical(report.device);
    DWORD rc = physical < 0 ? ERROR_DEVICE_NOT_CONNECTED :
        GAMEPKG_XINPUT_SET_STATE((DWORD)physical, (XINPUT_VIBRATION *)vibration);
    ReleaseSRWLockExclusive(&gamepkg_xi_lock);
    return rc;
}
static void gamepkg_xinput_shutdown(void) {
    AcquireSRWLockExclusive(&gamepkg_xi_lock);
    XINPUT_VIBRATION stop = {0, 0};
    for (DWORD physical = 0; physical < 4; physical++) {
        if (gamepkg_xi_slots[physical].connected)
            GAMEPKG_XINPUT_SET_STATE(physical, &stop);
        if (gamepkg_xi_slots[physical].device)
            gamepkg_input_disconnect(gamepkg_xi_slots[physical].device);
    }
    memset(gamepkg_xi_slots, 0, sizeof gamepkg_xi_slots);
    for (DWORD port = 0; port < 4; port++) gamepkg_xi_logical[port].valid = 0;
    gamepkg_xi_polled_at = 0;
    gamepkg_xi_last_port = 4;
    ReleaseSRWLockExclusive(&gamepkg_xi_lock);
}
#endif
