#ifndef GAMEPKG_INPUT_BACKEND_H
#define GAMEPKG_INPUT_BACKEND_H
#include "gamepkg_input.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Trusted native window/device adapters only. They own the existing event
   pump and translate host codes; do not start a second pump or apply another
   deadzone. Backend IDs namespace OS tokens. Repeated connect for the same
   kind/backend/token is idempotent until disconnect. Zero means failure. */
uint64_t gamepkg_input_connect(uint32_t kind, uint32_t backend, uint64_t token);
int32_t gamepkg_input_disconnect(uint64_t device);
/* Initially unfocused. Use focus from the game's own window. Losing focus
   discards queued input, resets held controls and cancels touch contacts. */
int32_t gamepkg_input_focus(uint32_t focused);
/* Valid updates return 1; rejected data returns 0; null pointers return -1.
   Nonzero timestamps older than a device's last accepted timestamp are
   rejected. Update once from the existing host pump/poll; consumers read
   snapshots without polling OS devices again. Rumble backends route through
   the device handle in the selected player's state, not the player index. */
int32_t gamepkg_input_update_gamepad(const gamepkg_input_gamepad *state);
int32_t gamepkg_input_submit(const gamepkg_input_event *event);
#ifdef __cplusplus
}
#endif
#endif
