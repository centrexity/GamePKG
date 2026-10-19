// Apache-2.0. Restore reviewed input snapshots when the original engine dispatches them.
#ifndef GAMEPKG_MODIFIERS_H
#define GAMEPKG_MODIFIERS_H
#include <stdio.h>
#include <stdlib.h>
static const Uint32 GAMEPKG_MODIFIERS_MARKER = 0x47504d31u;
static void gamepkg_set_modifiers(Uint32 mods)
{
    if(mods & ~Uint32(0x3fc3)) exit(2);
    // Keep the queue producer's state, but also restore it in dispatch order.
    SDL_SetModState(SDL_Keymod(mods));
    SDL_Event marker = {};
    marker.type = SDL_USEREVENT;
    marker.user.windowID = GAMEPKG_MODIFIERS_MARKER;
    marker.user.code = int(mods);
    if(SDL_PushEvent(&marker) != 1)
    {
        fprintf(stderr,"Unable to queue the game's input modifier snapshot.\n");
        exit(2);
    }
}
static bool gamepkg_apply_modifiers(const SDL_Event &event)
{
    if(event.type != SDL_USEREVENT || event.user.windowID != GAMEPKG_MODIFIERS_MARKER) return false;
    if(event.user.code < 0 || (Uint32(event.user.code) & ~Uint32(0x3fc3)) || event.user.data1 || event.user.data2) exit(2);
    SDL_SetModState(SDL_Keymod(event.user.code));
    return true;
}
#endif
