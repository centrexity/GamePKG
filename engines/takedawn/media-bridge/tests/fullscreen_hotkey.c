#define SDL_MAIN_HANDLED
#include "fullscreen_hotkey.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
int main(void)
{
    unsigned checks = 0;
    for (unsigned key = 0; key != 3; ++key) {
        for (unsigned mod = 0; mod != 65536; ++mod) {
            SDL_Event event; memset(&event, 0, sizeof event);
            unsigned held = 0;
            event.type = SDL_KEYDOWN;
            event.key.keysym.sym = key == 0 ? SDLK_RETURN : key == 1 ? SDLK_KP_ENTER : SDLK_SPACE;
            event.key.keysym.mod = (SDL_Keymod)mod;
            int toggle = key != 2 && (mod & KMOD_ALT) && !(mod & (KMOD_CTRL | KMOD_GUI));
            assert(gamepkg_fullscreen_hotkey(&held, &event) == (toggle ? 2 : 0));
            event.key.repeat = 1;
            event.key.keysym.mod = 0; /* Alt released before Enter. */
            assert(gamepkg_fullscreen_hotkey(&held, &event) == (toggle ? 1 : 0));
            event.type = SDL_KEYUP;
            assert(gamepkg_fullscreen_hotkey(&held, &event) == (toggle ? 1 : 0));
            assert(held == 0);
            assert(gamepkg_fullscreen_hotkey(&held, &event) == 0);
            checks += 5;
        }
    }
    for (unsigned reason = 0; reason != 3; ++reason) {
        unsigned held = 3;
        SDL_Event event; memset(&event, 0, sizeof event); event.type = SDL_WINDOWEVENT;
        event.window.event = reason == 0 ? SDL_WINDOWEVENT_FOCUS_LOST : reason == 1 ? SDL_WINDOWEVENT_MINIMIZED : SDL_WINDOWEVENT_HIDDEN;
        assert(gamepkg_fullscreen_hotkey(&held, &event) == 0 && held == 0);
        checks++;
    }
    /* Return and keypad Enter have independent edges. */
    unsigned held = 0;
    SDL_Event event; memset(&event, 0, sizeof event); event.type = SDL_KEYDOWN;
    event.key.keysym.mod = KMOD_RALT; event.key.keysym.sym = SDLK_RETURN;
    assert(gamepkg_fullscreen_hotkey(&held, &event) == 2);
    event.key.keysym.sym = SDLK_KP_ENTER;
    assert(gamepkg_fullscreen_hotkey(&held, &event) == 2 && held == 3);
    event.type = SDL_KEYUP; event.key.keysym.sym = SDLK_RETURN;
    assert(gamepkg_fullscreen_hotkey(&held, &event) == 1 && held == 2);
    event.key.keysym.sym = SDLK_KP_ENTER;
    assert(gamepkg_fullscreen_hotkey(&held, &event) == 1 && held == 0);
    printf("fullscreen shortcut: %u checks passed\n", checks + 4);
    return 0;
}
