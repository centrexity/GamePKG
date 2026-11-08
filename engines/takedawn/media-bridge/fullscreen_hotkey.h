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
/* Implemented beside the pinned settings reader; updates the overlay as well
 * as the window, including when the launch mode is host-controlled. */
void gamepkg_ToggleFullscreen(void);
#endif
