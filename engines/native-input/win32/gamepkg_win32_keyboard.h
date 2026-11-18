/* SPDX-License-Identifier: Apache-2.0 */
#ifndef GAMEPKG_WIN32_KEYBOARD_H
#define GAMEPKG_WIN32_KEYBOARD_H
#include <windows.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* One aggregate keyboard, fed by the existing game window's pump. No pump,
 * OS device poll, game action mapping or text-input activation is added.
 * CHAR messages must come from a Unicode window (UTF-16 WM_CHAR). */
enum { GAMEPKG_WIN32_KEY_DROP, GAMEPKG_WIN32_KEY_ACCEPT, GAMEPKG_WIN32_KEY_RESET };
uint32_t gamepkg_win32_key_usage(uint32_t virtual_key, uint32_t scan_flags);
int gamepkg_win32_keyboard_message(UINT message, WPARAM key, LPARAM flags);
/* Called by the existing pump before TranslateMessage/DispatchMessage.
 * Peeks without consuming: Windows' same-time synthetic left Ctrl followed
 * by right Alt is AltGr, rather than a second physical game control. */
int gamepkg_win32_keyboard_skip_altgr(const MSG *message);
void gamepkg_win32_keyboard_focus(int focused);
/* Once the game window reports focus, a device poll must not replace its
 * state with a process-wide foreground guess. False before a window attaches
 * and after shutdown, retaining the headless provider's existing fallback. */
int gamepkg_win32_keyboard_manages_focus(void);
void gamepkg_win32_keyboard_shutdown(void);
/* Legacy label-based mappings read accepted canonical state. A release uses
 * the label recorded at key-down, even if the layout changed while held.
 * Modifier labels group to VK_SHIFT/VK_CONTROL/VK_MENU; their physical sides
 * and the two Enter keys retain independent canonical state. */
uint32_t gamepkg_win32_keyboard_label(uint32_t virtual_key, uint32_t scan_flags);
int gamepkg_win32_keyboard_held(uint32_t virtual_key);
/* A window shortcut consumes this physical key, including repeats, until
 * its release. Release it here without resetting other held controls. */
void gamepkg_win32_keyboard_release(uint32_t virtual_key, uint32_t scan_flags);
#ifdef __cplusplus
}
#endif
#endif
