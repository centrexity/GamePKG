// Exercise the production queue markers independently of SDL/window backends.
#include <cassert>
#include <cstdint>
#include <vector>
#include <cstring>
typedef uint32_t Uint32;
typedef int SDL_Keymod;
static const Uint32 SDL_USEREVENT=0x8000, SDL_KEYDOWN=0x300, SDL_MOUSEBUTTONDOWN=0x401, SDL_MOUSEWHEEL=0x403;
struct SDL_Event
{
    Uint32 type;
    struct { Uint32 windowID; int code; void *data1, *data2; } user;
};
static int current=0, push_result=1;
static std::vector<SDL_Event> queue;
static void SDL_SetModState(SDL_Keymod mods) { current=mods; }
static int SDL_PushEvent(SDL_Event *event) { if(push_result!=1) return push_result; queue.push_back(*event); return 1; }
#include "../engines/cube2/adapters/modifiers.h"
int main(int argc, char **argv)
{
    if(argc==2)
    {
        if(!strcmp(argv[1],"--queue-full")) { push_result=-1;gamepkg_set_modifiers(128); }
        else if(!strcmp(argv[1],"--filtered")) { push_result=0;gamepkg_set_modifiers(128); }
        else if(!strcmp(argv[1],"--unknown-modifier")) gamepkg_set_modifiers(0x4000);
        else
        {
            SDL_Event marker={};marker.type=SDL_USEREVENT;marker.user.windowID=GAMEPKG_MODIFIERS_MARKER;
            if(!strcmp(argv[1],"--bad-marker")) marker.user.code=0x4000;
            else if(!strcmp(argv[1],"--negative-marker")) marker.user.code=-1;
            else if(!strcmp(argv[1],"--pointer-marker")) marker.user.data1=&marker;
            else return 3;
            gamepkg_apply_modifiers(marker);
        }
        return 3; // Every invalid case must stop inside the production helper.
    }
    std::vector<int> expected;
    for(unsigned batch=0;batch<2;batch++)
    {
        queue.clear();expected.clear();
        for(Uint32 mods : {0u,128u,0x3000u,0x3080u,0u})
        {
            gamepkg_set_modifiers(mods);
            for(Uint32 type : {SDL_KEYDOWN,SDL_MOUSEBUTTONDOWN,SDL_MOUSEWHEEL})
            {
                SDL_Event event={};event.type=type;SDL_PushEvent(&event);expected.push_back(mods);
            }
        }
        assert(current==0); // Producer finished before the consumer starts.
        std::vector<int> observed;
        for(const SDL_Event &event : queue)
        {
            if(gamepkg_apply_modifiers(event)) continue;
            observed.push_back(current);
        }
        assert(observed==expected);
    }
    SDL_Event foreign={};foreign.type=SDL_USEREVENT;foreign.user.windowID=17;foreign.user.code=128;
    assert(!gamepkg_apply_modifiers(foreign));assert(current==0);
    // A standalone snapshot restores state even without a following key event.
    queue.clear();gamepkg_set_modifiers(0x3000);current=0;
    assert(gamepkg_apply_modifiers(queue[0]));assert(current==0x3000);
}
