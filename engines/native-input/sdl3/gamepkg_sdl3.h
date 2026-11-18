#ifndef GAMEPKG_SDL3_H
#define GAMEPKG_SDL3_H
#include <SDL3/SDL.h>
#include "../../../include/gamepkg_input_backend.h"
#ifdef __cplusplus
extern "C" {
#endif

/* Called on the engine's existing SDL owner thread, after its event pump.
 * Only observes controllers the engine already opened. Never opens/closes a
 * device, pumps events, applies a deadzone or compacts another player's slot.
 * The result is the highest occupied slot plus one, with NULL holes. */
int gamepkg_sdl3_gamepads(SDL_Gamepad *out[GAMEPKG_INPUT_MAX_PLAYERS]);
int gamepkg_sdl3_snapshot(SDL_Gamepad *pad, gamepkg_input_gamepad *out);
/* Pure conversion back to SDL coordinates for an engine's existing mapper. */
bool gamepkg_sdl3_button(const gamepkg_input_gamepad *state, SDL_GamepadButton button);
Sint16 gamepkg_sdl3_axis(const gamepkg_input_gamepad *state, SDL_GamepadAxis axis);
/* Focus belongs to the engine's actual window. Call disconnect before its
 * existing SDL_CloseGamepad; shutdown before SDL_Quit. These do not own SDL. */
void gamepkg_sdl3_focus(bool focused);
void gamepkg_sdl3_disconnect(SDL_JoystickID id);
void gamepkg_sdl3_shutdown(void);
/* Call once per event from the existing SDL pump, before legacy actions.
 * Zero rejects the event. Other results are PASS plus optional resampling
 * flags: the engine must rebuild only the indicated legacy views from the
 * shared snapshots. Text input activation remains the engine's decision. */
#define GAMEPKG_SDL3_PASS 1
#define GAMEPKG_SDL3_RESET_KEYS 2
#define GAMEPKG_SDL3_RESET_POINTER 4
#define GAMEPKG_SDL3_RESET_TOUCH 8
/* Before the existing event pump: bind the actual game window (zero after
 * destruction), with focus obtained by the engine from SDL. A new binding
 * releases input and returns reset flags for rebuilding legacy views. The
 * same binding is a no-op; normal focus events still own focus changes. */
int gamepkg_sdl3_bind_window(SDL_WindowID window, bool focused);
int gamepkg_sdl3_window_event(const SDL_Event *event, SDL_WindowID game_window);
bool gamepkg_sdl3_focused(void);
int gamepkg_sdl3_key_held(SDL_Scancode code);
int gamepkg_sdl3_mouse_held(Uint8 button);
/* For a legacy touch API that takes only one finger ID. Call immediately
 * after an accepted finger event, on its local copy. Canonical contacts keep
 * their original full surface/finger pair; the legacy view gets a non-reused
 * proxy and validated coordinates, including canceled/invalid-coordinate UP. */
int gamepkg_sdl3_legacy_touch(SDL_TouchFingerEvent *event);
#ifdef __cplusplus
}
#endif
#endif
