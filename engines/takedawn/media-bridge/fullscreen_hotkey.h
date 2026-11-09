#ifndef GAMEPKG_FULLSCREEN_HOTKEY_H
#define GAMEPKG_FULLSCREEN_HOTKEY_H
#include <SDL.h>
/* 0: ordinary input, 1: consume, 2: toggle once. Track each Enter key until
 * release even when Alt is released first. Focus loss also releases ownership. */
static int gamepkg_fullscreen_hotkey(unsigned *held, const SDL_Event *event)
{
    if (event->type == SDL_WINDOWEVENT) {
        if (event->window.event == SDL_WINDOWEVENT_FOCUS_LOST
            || event->window.event == SDL_WINDOWEVENT_HIDDEN
            || event->window.event == SDL_WINDOWEVENT_MINIMIZED)
            *held = 0;
        return 0;
    }
    if (event->type != SDL_KEYDOWN && event->type != SDL_KEYUP)
        return 0;
    const unsigned bit = event->key.keysym.sym == SDLK_RETURN ? 1u
        : event->key.keysym.sym == SDLK_KP_ENTER ? 2u : 0u;
    if (!bit)
        return 0;
    if (event->type == SDL_KEYUP) {
        const int consumed = (*held & bit) != 0;
        *held &= ~bit;
        return consumed;
    }
    if (*held & bit)
        return 1;
    if (!event->key.repeat && (event->key.keysym.mod & KMOD_ALT)
        && !(event->key.keysym.mod & (KMOD_CTRL | KMOD_GUI))) {
        *held |= bit;
        return 2;
    }
    return 0;
}
/* Implemented beside the pinned settings reader. A launch option selects the
 * initial mode; it must not lock out subsequent desktop shortcuts. */
void gamepkg_SetFullscreen(int fullscreen);
void gamepkg_ToggleFullscreen(void);
/* Run on the window/event thread. Do not depend on OverlayPoll: the overlay
 * may be disabled or unavailable. Commit settings only after SDL succeeds. */
static int gamepkg_apply_fullscreen_shortcut(SDL_Window *window)
{
    const int fullscreen = !(SDL_GetWindowFlags(window) & SDL_WINDOW_FULLSCREEN);
    if (SDL_SetWindowFullscreen(window,
            fullscreen ? SDL_WINDOW_FULLSCREEN_DESKTOP : 0) != 0)
        return -1;
    gamepkg_SetFullscreen(fullscreen);
    return 0;
}
#endif
