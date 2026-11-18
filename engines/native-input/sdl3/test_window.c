/* Real shared Rust service, SDL3 event records; no OS input or display. */
#include "gamepkg_sdl3.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
enum { WINDOW=77 };
static int send(SDL_Event *event){return gamepkg_sdl3_window_event(event,WINDOW);}
static void flush(void){gamepkg_input_event events[64];while(gamepkg_input_read(events,64)>0){}}
static uint64_t device(uint32_t kind,uint64_t token){
    gamepkg_input_device list[16];int count=gamepkg_input_devices(list,16);
    for(int i=0;i<count;i++)if(list[i].backend==3&&list[i].kind==kind&&list[i].backend_token==token)return list[i].device;
    return 0;
}
static int window_event(SDL_EventType type,SDL_WindowID window){SDL_Event e;SDL_zero(e);e.type=type;e.window.windowID=window;return send(&e);}
static int key(SDL_EventType type,SDL_KeyboardID which,SDL_Scancode scan,SDL_Keycode label,SDL_Keymod mod,bool repeat){
    SDL_Event e;SDL_zero(e);e.type=type;e.key.windowID=WINDOW;e.key.which=which;e.key.scancode=scan;e.key.key=label;e.key.mod=mod;e.key.repeat=repeat;e.key.down=type==SDL_EVENT_KEY_DOWN;return send(&e);
}
static int mouse(SDL_EventType type,SDL_MouseID which,Uint8 button){SDL_Event e;SDL_zero(e);e.type=type;e.button.windowID=WINDOW;e.button.which=which;e.button.button=button;e.button.down=type==SDL_EVENT_MOUSE_BUTTON_DOWN;return send(&e);}
static int text(const char *value){SDL_Event e;SDL_zero(e);e.type=SDL_EVENT_TEXT_INPUT;e.text.windowID=WINDOW;e.text.text=value;return send(&e);}
static SDL_Event finger(SDL_EventType type,Uint64 surface,Uint64 id,float x,float y){SDL_Event e;SDL_zero(e);e.type=type;e.tfinger.windowID=WINDOW;e.tfinger.touchID=surface;e.tfinger.fingerID=id;e.tfinger.x=x;e.tfinger.y=y;return e;}
int main(void){
    assert(SDL_Init(SDL_INIT_EVENTS));
    assert(!gamepkg_sdl3_focused());assert(window_event(SDL_EVENT_WINDOW_FOCUS_GAINED,WINDOW)==GAMEPKG_SDL3_PASS);
    assert(gamepkg_sdl3_focused());
    int foreign_focus=window_event(SDL_EVENT_WINDOW_FOCUS_LOST,WINDOW+1);assert(foreign_focus==0);
    assert(window_event(SDL_EVENT_WINDOW_FOCUS_LOST,0)==0&&gamepkg_sdl3_focused());
    assert(key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_A,SDLK_Q,0,false)==GAMEPKG_SDL3_PASS);
    assert(gamepkg_sdl3_key_held(SDL_SCANCODE_A)&&!gamepkg_sdl3_key_held(SDL_SCANCODE_Q));
    uint64_t keyboard=device(GAMEPKG_INPUT_KEYBOARD,5);assert(keyboard&&gamepkg_input_key_state(keyboard,SDL_SCANCODE_A)==1);
    flush();int repeated_down=key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_A,SDLK_Q,0,false);assert(repeated_down==0);
    assert(key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_A,SDLK_Q,0,true)==0);
    assert(key(SDL_EVENT_KEY_DOWN,6,SDL_SCANCODE_A,SDLK_A,0,false)==GAMEPKG_SDL3_PASS);
    assert(key(SDL_EVENT_KEY_UP,5,SDL_SCANCODE_A,SDLK_A,0,false)==GAMEPKG_SDL3_PASS&&gamepkg_sdl3_key_held(SDL_SCANCODE_A));
    assert(key(SDL_EVENT_KEY_UP,6,SDL_SCANCODE_A,SDLK_Q,0,false)==GAMEPKG_SDL3_PASS&&!gamepkg_sdl3_key_held(SDL_SCANCODE_A));
    uint64_t pad=gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD,9,99);gamepkg_input_gamepad p={0};p.device=pad;p.buttons=GAMEPKG_INPUT_PAD_SOUTH;
    assert(gamepkg_input_update_gamepad(&p)==1);
    key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_W,SDLK_W,0,false);
    key(SDL_EVENT_KEY_DOWN,6,SDL_SCANCODE_W,SDLK_W,0,false);
    assert(key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_LGUI,SDLK_LGUI,SDL_KMOD_GUI,false)&GAMEPKG_SDL3_RESET_KEYS);
    assert(gamepkg_input_key_state(keyboard,SDL_SCANCODE_W)==0&&gamepkg_sdl3_key_held(SDL_SCANCODE_W));
    assert(gamepkg_input_gamepad_state(0,&p)==1&&p.buttons==GAMEPKG_INPUT_PAD_SOUTH);
    assert(key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_Q,SDLK_Q,SDL_KMOD_GUI,false)==0);
    key(SDL_EVENT_KEY_UP,5,SDL_SCANCODE_LGUI,SDLK_LGUI,0,false);
    key(SDL_EVENT_KEY_UP,5,SDL_SCANCODE_W,SDLK_W,0,false);
    key(SDL_EVENT_KEY_UP,6,SDL_SCANCODE_W,SDLK_W,0,false);
    /* Committed Unicode is independent of physical usages and key labels. */
    flush();assert(text("A\xc3\xa9\xf0\x9f\x8e\xae")==GAMEPKG_SDL3_PASS);
    gamepkg_input_event events[64];int count=gamepkg_input_read(events,64),n=0;uint32_t expected[]={0x41,0xe9,0x1f3ae};
    for(int i=0;i<count;i++)if(events[i].kind==GAMEPKG_INPUT_TEXT){assert(n<3&&events[i].code==expected[n++]);}
    assert(n==3);
    const char *invalid[]={"A\xff","\xc0\xaf","\xed\xa0\x80","\xf4\x90\x80\x80","\xf0\x9f"};
    for(unsigned i=0;i<sizeof invalid/sizeof *invalid;i++){flush();assert(text(invalid[i])==0);int invalid_events=gamepkg_input_read(events,64);assert(invalid_events==0);}
    for(unsigned position=0;position<32;position++)for(unsigned value=0;value<256;value++){
        char bytes[33];memset(bytes,'A',32);bytes[32]=0;bytes[position]=(char)value;text(bytes);flush();
    }
    assert(text(NULL)==0);
    /* Each real mouse is independent; a synthetic touch mouse is only
     * passed to the old UI, not duplicated into the canonical mouse stream. */
    assert(mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,1,SDL_BUTTON_LEFT)==GAMEPKG_SDL3_PASS);
    assert(mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,2,SDL_BUTTON_LEFT)==GAMEPKG_SDL3_PASS);
    mouse(SDL_EVENT_MOUSE_BUTTON_UP,1,SDL_BUTTON_LEFT);assert(gamepkg_sdl3_mouse_held(SDL_BUTTON_LEFT));
    mouse(SDL_EVENT_MOUSE_BUTTON_UP,2,SDL_BUTTON_LEFT);assert(!gamepkg_sdl3_mouse_held(SDL_BUTTON_LEFT));
    for(Uint8 button=1;button<=5;button++){assert(mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,1,button)==GAMEPKG_SDL3_PASS&&gamepkg_sdl3_mouse_held(button));mouse(SDL_EVENT_MOUSE_BUTTON_UP,1,button);}
    SDL_Event e;SDL_zero(e);e.type=SDL_EVENT_MOUSE_MOTION;e.motion.windowID=WINDOW;e.motion.which=1;e.motion.x=31.25f;e.motion.y=62.5f;e.motion.xrel=-.125f;e.motion.yrel=.75f;
    flush();assert(send(&e)==GAMEPKG_SDL3_PASS);count=gamepkg_input_read(events,64);assert(count==2);
    assert(events[0].kind==GAMEPKG_INPUT_POINTER&&events[0].x==31.25f&&events[1].kind==GAMEPKG_INPUT_RELATIVE&&events[1].x==-.125f&&events[1].y==.75f);
    e.motion.xrel=NAN;assert(send(&e)==0);assert(gamepkg_input_read(events,64)==0);
    SDL_zero(e);e.type=SDL_EVENT_MOUSE_WHEEL;e.wheel.windowID=WINDOW;e.wheel.which=1;e.wheel.x=1.25f;e.wheel.y=-2.5f;e.wheel.direction=SDL_MOUSEWHEEL_FLIPPED;
    assert(send(&e)==GAMEPKG_SDL3_PASS);assert(gamepkg_input_read(events,64)==1&&events[0].kind==GAMEPKG_INPUT_WHEEL&&events[0].x==-1.25f&&events[0].y==2.5f);
    flush();assert(mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,SDL_TOUCH_MOUSEID,SDL_BUTTON_LEFT)==GAMEPKG_SDL3_PASS);
    assert(!device(GAMEPKG_INPUT_MOUSE,SDL_TOUCH_MOUSEID)&&gamepkg_input_read(events,64)==0);
    /* Identical full-width finger IDs on different full-width surfaces
     * remain separate in the service and in legacy single-ID touch APIs. */
    Uint64 a=UINT64_C(0x100000001),b=UINT64_C(0x200000001),id=UINT64_C(0xf000000000000007);
    e=finger(SDL_EVENT_FINGER_DOWN,a,id,.25f,.5f);assert(send(&e)==GAMEPKG_SDL3_PASS);
    assert(gamepkg_sdl3_legacy_touch(&e.tfinger));Uint64 first=e.tfinger.fingerID;
    e=finger(SDL_EVENT_FINGER_DOWN,b,id,.75f,.5f);assert(send(&e)==GAMEPKG_SDL3_PASS);
    assert(gamepkg_sdl3_legacy_touch(&e.tfinger));Uint64 second=e.tfinger.fingerID;
    assert(first&&second&&first!=second&&gamepkg_input_contacts(NULL,0)==2);
    e=finger(SDL_EVENT_FINGER_MOTION,a,id,.3f,.6f);assert(send(&e)==GAMEPKG_SDL3_PASS&&gamepkg_sdl3_legacy_touch(&e.tfinger)&&e.tfinger.fingerID==first);
    e=finger(SDL_EVENT_FINGER_CANCELED,a,id,NAN,NAN);assert(send(&e)==GAMEPKG_SDL3_PASS&&gamepkg_sdl3_legacy_touch(&e.tfinger)&&e.tfinger.fingerID==first&&e.tfinger.x==.3f&&e.tfinger.y==.6f);
    assert(gamepkg_input_contacts(NULL,0)==1);
    e=finger(SDL_EVENT_FINGER_UP,b,id,2,-1);assert(send(&e)==GAMEPKG_SDL3_PASS&&gamepkg_sdl3_legacy_touch(&e.tfinger)&&e.tfinger.fingerID==second&&e.tfinger.x==.75f);
    assert(gamepkg_input_contacts(NULL,0)==0);
    e=finger(SDL_EVENT_FINGER_MOTION,a,id,.5f,.5f);assert(send(&e)==0&&!gamepkg_sdl3_legacy_touch(&e.tfinger));
    /* Background edges are tracked for release latches but never delivered
     * to the original action handler. A foreground notification alone does
     * not grant the window input focus. */
    key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_W,SDLK_W,0,false);mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,1,SDL_BUTTON_RIGHT);
    assert(window_event(SDL_EVENT_WINDOW_FOCUS_LOST,WINDOW)&GAMEPKG_SDL3_RESET_TOUCH);
    assert(!gamepkg_sdl3_focused()&&!gamepkg_sdl3_key_held(SDL_SCANCODE_W)&&!gamepkg_sdl3_mouse_held(SDL_BUTTON_RIGHT));
    assert(key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_Q,SDLK_Q,0,false)==0);
    assert(mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,1,SDL_BUTTON_LEFT)==0&&text("background")==0);
    SDL_zero(e);e.type=SDL_EVENT_DID_ENTER_FOREGROUND;assert(send(&e)==GAMEPKG_SDL3_PASS&&!gamepkg_sdl3_focused());
    window_event(SDL_EVENT_WINDOW_FOCUS_GAINED,WINDOW);
    assert(key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_Q,SDLK_Q,0,false)==0);
    assert(mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,1,SDL_BUTTON_LEFT)==0);
    key(SDL_EVENT_KEY_UP,5,SDL_SCANCODE_Q,SDLK_Q,0,false);mouse(SDL_EVENT_MOUSE_BUTTON_UP,1,SDL_BUTTON_LEFT);
    assert(key(SDL_EVENT_KEY_DOWN,5,SDL_SCANCODE_Q,SDLK_Q,0,false)==GAMEPKG_SDL3_PASS);
    assert(mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,1,SDL_BUTTON_LEFT)==GAMEPKG_SDL3_PASS);
    SDL_zero(e);e.type=SDL_EVENT_KEYBOARD_REMOVED;e.kdevice.which=5;assert(send(&e)&GAMEPKG_SDL3_RESET_KEYS);assert(!gamepkg_sdl3_key_held(SDL_SCANCODE_Q));
    SDL_zero(e);e.type=SDL_EVENT_MOUSE_REMOVED;e.mdevice.which=1;assert(send(&e)&GAMEPKG_SDL3_RESET_POINTER);assert(!gamepkg_sdl3_mouse_held(SDL_BUTTON_LEFT));
    e=finger(SDL_EVENT_FINGER_DOWN,a,id,.2f,.3f);assert(send(&e)==GAMEPKG_SDL3_PASS&&gamepkg_sdl3_legacy_touch(&e.tfinger)&&e.tfinger.fingerID>second);
    /* Replacing an owned window releases all controls. Queued old-window
     * releases clear only blocked latches, never new-window visible keys. */
    assert(gamepkg_sdl3_bind_window(WINDOW,true)==14);
    assert(key(SDL_EVENT_KEY_DOWN,7,SDL_SCANCODE_C,SDLK_C,0,false)==GAMEPKG_SDL3_PASS);
    assert(mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,3,SDL_BUTTON_X1)==GAMEPKG_SDL3_PASS);
    e=finger(SDL_EVENT_FINGER_DOWN,a,id,.2f,.3f);assert(send(&e)==GAMEPKG_SDL3_PASS);
    assert(gamepkg_sdl3_bind_window(WINDOW,true)==0&&gamepkg_sdl3_key_held(SDL_SCANCODE_C));
    assert(gamepkg_sdl3_bind_window(WINDOW+2,true)==14);
    int replaced_neutral=!gamepkg_sdl3_key_held(SDL_SCANCODE_C)&&!gamepkg_sdl3_mouse_held(SDL_BUTTON_X1)&&gamepkg_input_contacts(NULL,0)==0;
    assert(replaced_neutral);
    SDL_zero(e);e.type=SDL_EVENT_KEY_UP;e.key.windowID=WINDOW;e.key.which=7;e.key.scancode=SDL_SCANCODE_C;
    assert(gamepkg_sdl3_window_event(&e,WINDOW+2)==0);
    e.type=SDL_EVENT_KEY_DOWN;e.key.windowID=WINDOW+2;
    int fresh_after_old_release=gamepkg_sdl3_window_event(&e,WINDOW+2);assert(fresh_after_old_release==GAMEPKG_SDL3_PASS);
    e.type=SDL_EVENT_KEY_UP;e.key.windowID=WINDOW;
    assert(gamepkg_sdl3_window_event(&e,WINDOW+2)==0&&gamepkg_sdl3_key_held(SDL_SCANCODE_C));
    e.key.windowID=WINDOW+2;assert(gamepkg_sdl3_window_event(&e,WINDOW+2)==GAMEPKG_SDL3_PASS&&!gamepkg_sdl3_key_held(SDL_SCANCODE_C));
    SDL_zero(e);e.type=SDL_EVENT_MOUSE_BUTTON_UP;e.button.windowID=WINDOW;e.button.which=3;e.button.button=SDL_BUTTON_X1;
    assert(gamepkg_sdl3_window_event(&e,WINDOW+2)==0);
    e.type=SDL_EVENT_MOUSE_BUTTON_DOWN;e.button.windowID=WINDOW+2;
    assert(gamepkg_sdl3_window_event(&e,WINDOW+2)==GAMEPKG_SDL3_PASS&&gamepkg_sdl3_mouse_held(SDL_BUTTON_X1));
    e.type=SDL_EVENT_MOUSE_BUTTON_UP;e.button.windowID=WINDOW;
    assert(gamepkg_sdl3_window_event(&e,WINDOW+2)==0&&gamepkg_sdl3_mouse_held(SDL_BUTTON_X1));
    SDL_zero(e);e.type=SDL_EVENT_KEY_DOWN;e.key.windowID=WINDOW+2;e.key.which=7;e.key.scancode=SDL_SCANCODE_C;
    assert(gamepkg_sdl3_window_event(&e,WINDOW+2)==GAMEPKG_SDL3_PASS);
    assert(gamepkg_sdl3_bind_window(0,true)==14&&!gamepkg_sdl3_focused());
    e.type=SDL_EVENT_KEY_UP;assert(gamepkg_sdl3_window_event(&e,0)==0);
    SDL_zero(e);e.type=SDL_EVENT_MOUSE_BUTTON_UP;e.button.windowID=WINDOW+2;e.button.which=3;e.button.button=SDL_BUTTON_X1;
    assert(gamepkg_sdl3_window_event(&e,0)==0);
    SDL_zero(e);e.type=SDL_EVENT_WINDOW_FOCUS_GAINED;e.window.windowID=WINDOW+2;
    assert(gamepkg_sdl3_window_event(&e,WINDOW+2)==0&&!gamepkg_sdl3_focused());
    assert(gamepkg_sdl3_bind_window(WINDOW,true)==14);
    assert(key(SDL_EVENT_KEY_DOWN,7,SDL_SCANCODE_C,SDLK_C,0,false)==GAMEPKG_SDL3_PASS);
    assert(mouse(SDL_EVENT_MOUSE_BUTTON_DOWN,3,SDL_BUTTON_X1)==GAMEPKG_SDL3_PASS);
    window_event(SDL_EVENT_WINDOW_HIDDEN,WINDOW);assert(!gamepkg_sdl3_focused()&&gamepkg_input_contacts(NULL,0)==0);
    gamepkg_sdl3_shutdown();gamepkg_input_device devices[16];count=gamepkg_input_devices(devices,16);
    assert(count==1&&devices[0].device==pad&&devices[0].backend==9);
    gamepkg_input_disconnect(pad);SDL_Quit();
    puts("PASS SDL3 window/Rust: HID keys/multiple keyboards, Unicode+8192 mutations, five mouse buttons/precision, full-width touch/proxies/cancel, focus latches, foreign-window rejection and device lifetime");return 0;
}
