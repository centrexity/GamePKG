/* SPDX-License-Identifier: Apache-2.0 */
#ifndef GAMEPKG_NATIVE_SDL2_EVENTS_H
#define GAMEPKG_NATIVE_SDL2_EVENTS_H
#include <SDL.h>
#include <stdint.h>
/* Called once by the engine's existing SDL event dispatcher. No OS pump,
 * remapping, text-input activation, or game action synthesis occurs here.
 * DROP suppresses a rejected legacy key/touch event; RESET clears the
 * legacy keyboard/touch action view after losing focus. */
enum { GAMEPKG_SDL2_EVENT_DROP, GAMEPKG_SDL2_EVENT_ACCEPT, GAMEPKG_SDL2_EVENT_RESET };
int gamepkg_sdl2_window_event(const SDL_Event *event);
void gamepkg_sdl2_window_shutdown(void);
/* Legacy layout has one integer ID. Use a non-reused proxy for the full
 * (touch device, finger) identity of the current accepted event. */
int64_t gamepkg_sdl2_layout_finger(const SDL_TouchFingerEvent *event);
/* Main-pump-only helpers for the legacy label-based keyboard view. */
int gamepkg_sdl2_label_held(SDL_Keycode label);
SDL_Keycode gamepkg_sdl2_release_label(const SDL_KeyboardEvent *event);
#endif
