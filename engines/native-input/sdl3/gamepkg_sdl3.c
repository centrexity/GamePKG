#include "gamepkg_sdl3.h"
#include <string.h>
#include <math.h>

enum { BACKEND=3 };
static bool window_focused;
static bool window_bound;
static SDL_WindowID bound_window;
static struct { uint64_t device,finger,proxy; } touches[GAMEPKG_INPUT_MAX_DEVICES*GAMEPKG_INPUT_MAX_TOUCHES];
static uint64_t next_proxy,last_surface,last_finger,last_proxy;
static SDL_EventType last_touch_type;
static float last_touch_x,last_touch_y;
static struct { SDL_JoystickID id; uint64_t device; bool seen; gamepkg_input_gamepad raw; } pads[GAMEPKG_INPUT_MAX_DEVICES];
/* SDL's named paddle positions preserve the existing SDL2/common meanings:
 * right upper, left upper, right lower, left lower. */
static const unsigned char bits[SDL_GAMEPAD_BUTTON_COUNT] = {
    12,13,14,15,5,10,4,6,7,8,9,0,1,2,3,11,17,18,19,20,16,21,22,23,24,25
};
_Static_assert(SDL_GAMEPAD_BUTTON_COUNT==26,"Review new SDL gamepad buttons");
static bool on_owner(void) {
    return SDL_IsMainThread();
}
static int slot(SDL_JoystickID id) {
    for(unsigned i=0;i<GAMEPKG_INPUT_MAX_DEVICES;i++)if(pads[i].device&&pads[i].id==id)return (int)i;
    return -1;
}
static uint16_t trigger(Sint16 raw) {
    unsigned value=raw>0?(unsigned)raw:0;
    return (uint16_t)((value<<1)|(value>>14));
}
static void sample(unsigned index,SDL_Gamepad *pad) {
    gamepkg_input_gamepad state={0};
    state.device=pads[index].device;state.timestamp_ns=gamepkg_input_time_ns();
    for(unsigned button=0;button<SDL_GAMEPAD_BUTTON_COUNT;button++)
        if(SDL_GetGamepadButton(pad,(SDL_GamepadButton)button))state.buttons|=1u<<bits[button];
    state.left_x=SDL_GetGamepadAxis(pad,SDL_GAMEPAD_AXIS_LEFTX);
    state.left_y=(int16_t)(-1-(int)SDL_GetGamepadAxis(pad,SDL_GAMEPAD_AXIS_LEFTY));
    state.right_x=SDL_GetGamepadAxis(pad,SDL_GAMEPAD_AXIS_RIGHTX);
    state.right_y=(int16_t)(-1-(int)SDL_GetGamepadAxis(pad,SDL_GAMEPAD_AXIS_RIGHTY));
    state.left_trigger=trigger(SDL_GetGamepadAxis(pad,SDL_GAMEPAD_AXIS_LEFT_TRIGGER));
    state.right_trigger=trigger(SDL_GetGamepadAxis(pad,SDL_GAMEPAD_AXIS_RIGHT_TRIGGER));
    pads[index].raw=state;
    gamepkg_input_update_gamepad(&state);
}
/* A queued down/up may both precede the next snapshot. Replay only this
 * button against the previous report; sampling SDL here would see future
 * buttons and change first-press player ownership. The regular end-of-pump
 * snapshot still supplies exact axes and reconciles missed OS events. */
static void button_event(const SDL_GamepadButtonEvent *event) {
    if((unsigned)event->button>=SDL_GAMEPAD_BUTTON_COUNT)return;
    SDL_Gamepad *pad=SDL_GetGamepadFromID(event->which);
    if(!pad)return; /* The engine owns open/close. Ignore stale device IDs. */
    int index=slot(event->which);
    if(index<0) {
        for(unsigned i=0;i<GAMEPKG_INPUT_MAX_DEVICES;i++)if(!pads[i].device) {
            uint64_t device=gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD,BACKEND,event->which);
            if(device){pads[i].id=event->which;pads[i].device=device;index=(int)i;}
            break;
        }
    }
    if(index<0)return;
    gamepkg_input_gamepad *raw=&pads[index].raw;
    raw->device=pads[index].device;raw->timestamp_ns=gamepkg_input_time_ns();
    uint32_t mask=1u<<bits[event->button];
    if(event->type==SDL_EVENT_GAMEPAD_BUTTON_DOWN)raw->buttons|=mask;
    else raw->buttons&=~mask;
    gamepkg_input_update_gamepad(raw);
}
int gamepkg_sdl3_gamepads(SDL_Gamepad *out[GAMEPKG_INPUT_MAX_PLAYERS]) {
    int count=0,highest=0;
    SDL_JoystickID *ids;
    gamepkg_input_gamepad state;
    if(!out)return -1;
    memset(out,0,sizeof(*out)*GAMEPKG_INPUT_MAX_PLAYERS);
    if(!on_owner())return -1;
    for(unsigned i=0;i<GAMEPKG_INPUT_MAX_DEVICES;i++)pads[i].seen=false;
    ids=SDL_GetGamepads(&count);
    /* A failed enumeration is not proof every controller disconnected. */
    if(!ids)return -1;
    for(int i=0;i<count;i++) {
        int index=slot(ids[i]);
        if(index>=0&&SDL_GetGamepadFromID(ids[i]))pads[index].seen=true;
    }
    for(unsigned i=0;i<GAMEPKG_INPUT_MAX_DEVICES;i++)if(pads[i].device&&!pads[i].seen) {
        gamepkg_input_disconnect(pads[i].device);memset(&pads[i],0,sizeof pads[i]);
    }
    for(int i=0;i<count;i++) {
        SDL_Gamepad *pad=SDL_GetGamepadFromID(ids[i]);
        int index;
        if(!pad)continue;
        index=slot(ids[i]);
        if(index<0) {
            for(unsigned j=0;j<GAMEPKG_INPUT_MAX_DEVICES;j++)if(!pads[j].device) {
                uint64_t device=gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD,BACKEND,ids[i]);
                if(device){pads[j].id=ids[i];pads[j].device=device;index=(int)j;}
                break;
            }
        }
        if(index>=0){pads[index].seen=true;sample((unsigned)index,pad);}
    }
    SDL_free(ids);
    for(unsigned player=0;player<GAMEPKG_INPUT_MAX_PLAYERS;player++)if(gamepkg_input_gamepad_state(player,&state)==1) {
        for(unsigned i=0;i<GAMEPKG_INPUT_MAX_DEVICES;i++)if(pads[i].device==state.device) {
            out[player]=SDL_GetGamepadFromID(pads[i].id);
            if(out[player])highest=(int)player+1;
        }
    }
    return highest;
}
int gamepkg_sdl3_snapshot(SDL_Gamepad *pad,gamepkg_input_gamepad *out) {
    int index;
    if(!out)return -1;
    memset(out,0,sizeof *out);
    if(!pad||!on_owner())return 0;
    index=slot(SDL_GetGamepadID(pad));
    if(index<0)return 0;
    for(unsigned player=0;player<GAMEPKG_INPUT_MAX_PLAYERS;player++) {
        gamepkg_input_gamepad state;
        if(gamepkg_input_gamepad_state(player,&state)==1&&state.device==pads[index].device){*out=state;return 1;}
    }
    return 0;
}
bool gamepkg_sdl3_button(const gamepkg_input_gamepad *state,SDL_GamepadButton button) {
    return state&&(unsigned)button<SDL_GAMEPAD_BUTTON_COUNT&&(state->buttons&(1u<<bits[button]))!=0;
}
Sint16 gamepkg_sdl3_axis(const gamepkg_input_gamepad *state,SDL_GamepadAxis axis) {
    if(!state)return 0;
    switch(axis) {
    case SDL_GAMEPAD_AXIS_LEFTX:return state->left_x;
    case SDL_GAMEPAD_AXIS_LEFTY:return (Sint16)(-1-(int)state->left_y);
    case SDL_GAMEPAD_AXIS_RIGHTX:return state->right_x;
    case SDL_GAMEPAD_AXIS_RIGHTY:return (Sint16)(-1-(int)state->right_y);
    case SDL_GAMEPAD_AXIS_LEFT_TRIGGER:return (Sint16)(state->left_trigger>>1);
    case SDL_GAMEPAD_AXIS_RIGHT_TRIGGER:return (Sint16)(state->right_trigger>>1);
    default:return 0;
    }
}
void gamepkg_sdl3_focus(bool focused) {
    if(!on_owner())return;
    window_focused=focused;gamepkg_input_focus(focused?1:0);
    if(!focused){memset(touches,0,sizeof touches);last_proxy=0;}
}
int gamepkg_sdl3_bind_window(SDL_WindowID window,bool focused) {
    if(!on_owner()||(window_bound&&window==bound_window))return 0;
    window_bound=true;bound_window=window;
    gamepkg_sdl3_focus(false);
    if(window&&focused)gamepkg_sdl3_focus(true);
    return GAMEPKG_SDL3_RESET_KEYS|GAMEPKG_SDL3_RESET_POINTER|GAMEPKG_SDL3_RESET_TOUCH;
}
void gamepkg_sdl3_disconnect(SDL_JoystickID id) {
    int index;
    if(!on_owner())return;
    index=slot(id);
    if(index>=0){gamepkg_input_disconnect(pads[index].device);memset(&pads[index],0,sizeof pads[index]);}
}
void gamepkg_sdl3_shutdown(void) {
    if(!on_owner())return;
    for(unsigned i=0;i<GAMEPKG_INPUT_MAX_DEVICES;i++)if(pads[i].device)gamepkg_input_disconnect(pads[i].device);
    memset(pads,0,sizeof pads);
    gamepkg_sdl3_focus(false);
    window_bound=false;bound_window=0;
    gamepkg_input_device list[GAMEPKG_INPUT_MAX_DEVICES];
    int count=gamepkg_input_devices(list,GAMEPKG_INPUT_MAX_DEVICES);
    for(int i=0;i<count;i++)if(list[i].backend==BACKEND)gamepkg_input_disconnect(list[i].device);
}
static uint64_t find_device(uint32_t kind,uint64_t token) {
    gamepkg_input_device list[GAMEPKG_INPUT_MAX_DEVICES];
    int count=gamepkg_input_devices(list,GAMEPKG_INPUT_MAX_DEVICES);
    for(int i=0;i<count;i++)if(list[i].backend==BACKEND&&list[i].kind==kind&&list[i].backend_token==token)return list[i].device;
    return 0;
}
static int submit(uint64_t device,uint32_t kind,uint32_t code,float x,float y,uint64_t contact) {
    if(!device)return 0;
    gamepkg_input_event event={0};event.device=device;event.timestamp_ns=gamepkg_input_time_ns();
    event.kind=kind;event.code=code;event.x=x;event.y=y;event.contact=contact;
    return gamepkg_input_submit(&event);
}
static unsigned mouse_code(Uint8 button) {
    switch(button){case SDL_BUTTON_LEFT:return GAMEPKG_INPUT_MOUSE_LEFT;
    case SDL_BUTTON_RIGHT:return GAMEPKG_INPUT_MOUSE_RIGHT;case SDL_BUTTON_MIDDLE:return GAMEPKG_INPUT_MOUSE_MIDDLE;
    case SDL_BUTTON_X1:return GAMEPKG_INPUT_MOUSE_BACK;case SDL_BUTTON_X2:return GAMEPKG_INPUT_MOUSE_FORWARD;default:return 0;}
}
bool gamepkg_sdl3_focused(void) {return on_owner()&&window_focused;}
int gamepkg_sdl3_key_held(SDL_Scancode code) {
    if(!on_owner()||(unsigned)code==0||(unsigned)code>=512)return 0;
    gamepkg_input_device list[GAMEPKG_INPUT_MAX_DEVICES];
    int count=gamepkg_input_devices(list,GAMEPKG_INPUT_MAX_DEVICES);
    for(int i=0;i<count;i++)if(list[i].backend==BACKEND&&list[i].kind==GAMEPKG_INPUT_KEYBOARD&&gamepkg_input_key_state(list[i].device,(uint32_t)code)==1)return 1;
    return 0;
}
int gamepkg_sdl3_mouse_held(Uint8 button) {
    unsigned code=mouse_code(button);if(!on_owner()||!code)return 0;
    gamepkg_input_device list[GAMEPKG_INPUT_MAX_DEVICES];
    int count=gamepkg_input_devices(list,GAMEPKG_INPUT_MAX_DEVICES);
    for(int i=0;i<count;i++)if(list[i].backend==BACKEND&&list[i].kind==GAMEPKG_INPUT_MOUSE) {
        gamepkg_input_pointer state;
        if(gamepkg_input_pointer_state(list[i].device,&state)==1&&(state.buttons&(1u<<(code-1))))return 1;
    }
    return 0;
}
/* SDL owns a NUL-terminated string until the next event poll. Validate the
 * entire string before emitting any scalars; no temporary allocation or
 * arbitrary text-length cap is required. Stop at a missing continuation
 * before reading beyond its terminator. */
static int scalar(const unsigned char **cursor,uint32_t *code) {
    const unsigned char *p=*cursor;uint32_t value=*p++;unsigned extra;uint32_t minimum;
    if(value<0x80){extra=0;minimum=1;}
    else if(value>=0xc2&&value<=0xdf){extra=1;minimum=0x80;value&=0x1f;}
    else if(value>=0xe0&&value<=0xef){extra=2;minimum=0x800;value&=0xf;}
    else if(value>=0xf0&&value<=0xf4){extra=3;minimum=0x10000;value&=7;}
    else return 0;
    for(unsigned i=0;i<extra;i++){unsigned byte=*p++;if((byte&0xc0)!=0x80)return 0;value=(value<<6)|(byte&0x3f);}
    if(value<minimum||value>0x10ffff||(value>=0xd800&&value<=0xdfff))return 0;
    *cursor=p;*code=value;return 1;
}
static int text(const char *value) {
    if(!value)return 0;
    const unsigned char *p=(const unsigned char*)value;uint32_t code;
    while(*p)if(!scalar(&p,&code))return 0;
    if(!window_focused)return 0;
    if(!*value)return GAMEPKG_SDL3_PASS;
    uint64_t keyboard=gamepkg_input_connect(GAMEPKG_INPUT_KEYBOARD,BACKEND,0);
    p=(const unsigned char*)value;
    while(*p){if(!scalar(&p,&code)||submit(keyboard,GAMEPKG_INPUT_TEXT,code,0,0,0)!=1)return 0;}
    return GAMEPKG_SDL3_PASS;
}
static int contact(uint64_t device,uint64_t finger,gamepkg_input_contact *out) {
    gamepkg_input_contact list[GAMEPKG_INPUT_MAX_DEVICES*GAMEPKG_INPUT_MAX_TOUCHES];
    int count=gamepkg_input_contacts(list,GAMEPKG_INPUT_MAX_DEVICES*GAMEPKG_INPUT_MAX_TOUCHES);
    for(int i=0;i<count;i++)if(list[i].device==device&&list[i].contact==finger){*out=list[i];return 1;}
    return 0;
}
int gamepkg_sdl3_legacy_touch(SDL_TouchFingerEvent *event) {
    if(!on_owner()||!event||!last_proxy||event->type!=last_touch_type||event->touchID!=last_surface||event->fingerID!=last_finger)return 0;
    event->fingerID=last_proxy;event->x=last_touch_x;event->y=last_touch_y;return 1;
}
static int touch(const SDL_TouchFingerEvent *event) {
    bool begin=event->type==SDL_EVENT_FINGER_DOWN;
    bool end=event->type==SDL_EVENT_FINGER_UP||event->type==SDL_EVENT_FINGER_CANCELED;
    uint64_t device=begin?gamepkg_input_connect(GAMEPKG_INPUT_TOUCH,BACKEND,event->touchID):find_device(GAMEPKG_INPUT_TOUCH,event->touchID);
    if(!device)return 0;
    gamepkg_input_contact previous;int exists=contact(device,event->fingerID,&previous);
    float x=event->x,y=event->y;
    if(end&&(!isfinite(x)||!isfinite(y)||x<0||x>1||y<0||y>1)) {
        if(!exists)return 0;
        x=previous.x;y=previous.y;
    }
    if(submit(device,begin?GAMEPKG_INPUT_TOUCH_BEGIN:end?GAMEPKG_INPUT_TOUCH_END:GAMEPKG_INPUT_TOUCH_MOVE,0,x,y,event->fingerID)!=1||!window_focused)return 0;
    if(!(end?exists:contact(device,event->fingerID,&previous)))return 0;
    unsigned index;
    for(index=0;index<sizeof touches/sizeof *touches;index++)if(touches[index].device==device&&touches[index].finger==event->fingerID)break;
    if(begin) {
        for(index=0;index<sizeof touches/sizeof *touches;index++)if(!touches[index].device)break;
        if(index==sizeof touches/sizeof *touches||next_proxy==UINT64_MAX) {
            submit(device,GAMEPKG_INPUT_TOUCH_END,0,x,y,event->fingerID);return 0;
        }
        touches[index].device=device;touches[index].finger=event->fingerID;touches[index].proxy=++next_proxy;
    }
    if(index==sizeof touches/sizeof *touches)return 0;
    last_surface=event->touchID;last_finger=event->fingerID;last_proxy=touches[index].proxy;
    last_touch_type=event->type;last_touch_x=x;last_touch_y=y;
    if(end)memset(&touches[index],0,sizeof touches[index]);
    return GAMEPKG_SDL3_PASS;
}
/* Only invisible state can change here. Releases still matter during the
 * interval with no window, or when SDL queued them for a retired window. */
static void release_blocked(const SDL_Event *event) {
    if(event->type==SDL_EVENT_KEY_UP&&event->key.windowID) {
        unsigned code=(unsigned)event->key.scancode;
        uint64_t device=find_device(GAMEPKG_INPUT_KEYBOARD,event->key.which);
        if(device&&code&&code<512&&gamepkg_input_key_state(device,code)==0)
            submit(device,GAMEPKG_INPUT_KEY_UP,code,0,0,0);
    } else if(event->type==SDL_EVENT_MOUSE_BUTTON_UP&&event->button.windowID&&event->button.which!=SDL_TOUCH_MOUSEID) {
        unsigned code=mouse_code(event->button.button);
        uint64_t device=find_device(GAMEPKG_INPUT_MOUSE,event->button.which);
        gamepkg_input_pointer state;
        if(device&&code&&gamepkg_input_pointer_state(device,&state)==1&&!(state.buttons&(1u<<(code-1))))
            submit(device,GAMEPKG_INPUT_MOUSE_UP,code,0,0,0);
    }
}
int gamepkg_sdl3_window_event(const SDL_Event *event,SDL_WindowID game_window) {
    if(!on_owner()||!event)return 0;
    last_proxy=0;
    if(!game_window||(window_bound&&game_window!=bound_window)){release_blocked(event);return 0;}
    SDL_WindowID window=0;bool directed=true;
    switch(event->type) {
    case SDL_EVENT_WINDOW_FOCUS_LOST:case SDL_EVENT_WINDOW_HIDDEN:case SDL_EVENT_WINDOW_MINIMIZED:
    case SDL_EVENT_WINDOW_FOCUS_GAINED:case SDL_EVENT_WINDOW_SHOWN:case SDL_EVENT_WINDOW_RESTORED:
        window=event->window.windowID;break;
    case SDL_EVENT_KEY_DOWN:case SDL_EVENT_KEY_UP:window=event->key.windowID;break;
    case SDL_EVENT_TEXT_INPUT:window=event->text.windowID;break;
    case SDL_EVENT_TEXT_EDITING:window=event->edit.windowID;break;
    case SDL_EVENT_TEXT_EDITING_CANDIDATES:window=event->edit_candidates.windowID;break;
    case SDL_EVENT_MOUSE_MOTION:window=event->motion.windowID;break;
    case SDL_EVENT_MOUSE_BUTTON_DOWN:case SDL_EVENT_MOUSE_BUTTON_UP:window=event->button.windowID;break;
    case SDL_EVENT_MOUSE_WHEEL:window=event->wheel.windowID;break;
    case SDL_EVENT_FINGER_DOWN:case SDL_EVENT_FINGER_MOTION:case SDL_EVENT_FINGER_UP:case SDL_EVENT_FINGER_CANCELED:window=event->tfinger.windowID;break;
    default:directed=false;break;
    }
    if(directed&&window!=game_window) {
        /* A physical release queued for the old window can clear a held
         * latch after replacement. It must never release a visible control
         * in the new window, create a device, or reach legacy actions. */
        release_blocked(event);
        return 0;
    }
    switch(event->type) {
    case SDL_EVENT_WINDOW_FOCUS_LOST:case SDL_EVENT_WINDOW_HIDDEN:case SDL_EVENT_WINDOW_MINIMIZED:
    case SDL_EVENT_WILL_ENTER_BACKGROUND:case SDL_EVENT_DID_ENTER_BACKGROUND:
        gamepkg_sdl3_focus(false);return GAMEPKG_SDL3_PASS|GAMEPKG_SDL3_RESET_KEYS|GAMEPKG_SDL3_RESET_POINTER|GAMEPKG_SDL3_RESET_TOUCH;
    case SDL_EVENT_WINDOW_FOCUS_GAINED:gamepkg_sdl3_focus(true);return GAMEPKG_SDL3_PASS;
    case SDL_EVENT_KEYBOARD_REMOVED:{
        uint64_t device=find_device(GAMEPKG_INPUT_KEYBOARD,event->kdevice.which);if(device)gamepkg_input_disconnect(device);
        return GAMEPKG_SDL3_PASS|GAMEPKG_SDL3_RESET_KEYS;
    }
    case SDL_EVENT_MOUSE_REMOVED:{
        uint64_t device=find_device(GAMEPKG_INPUT_MOUSE,event->mdevice.which);if(device)gamepkg_input_disconnect(device);
        return GAMEPKG_SDL3_PASS|GAMEPKG_SDL3_RESET_POINTER;
    }
    case SDL_EVENT_GAMEPAD_REMOVED:gamepkg_sdl3_disconnect(event->gdevice.which);return GAMEPKG_SDL3_PASS;
    case SDL_EVENT_GAMEPAD_BUTTON_DOWN:case SDL_EVENT_GAMEPAD_BUTTON_UP:
        button_event(&event->gbutton);return GAMEPKG_SDL3_PASS;
    case SDL_EVENT_KEY_DOWN:case SDL_EVENT_KEY_UP:{
        unsigned code=(unsigned)event->key.scancode;bool down=event->type==SDL_EVENT_KEY_DOWN;
        if(!window||!code||code>=512||(down&&event->key.repeat))return 0;
        uint64_t device=gamepkg_input_connect(GAMEPKG_INPUT_KEYBOARD,BACKEND,event->key.which);
        if(down&&(event->key.scancode==SDL_SCANCODE_LGUI||event->key.scancode==SDL_SCANCODE_RGUI)) {
            submit(device,GAMEPKG_INPUT_KEY_DOWN,code,0,0,0);gamepkg_input_reset_device(device);
            return window_focused?GAMEPKG_SDL3_PASS|GAMEPKG_SDL3_RESET_KEYS:0;
        }
        if(down&&(event->key.mod&SDL_KMOD_GUI))return 0;
        if(down&&gamepkg_input_key_state(device,code)==1)return 0;
        if(submit(device,down?GAMEPKG_INPUT_KEY_DOWN:GAMEPKG_INPUT_KEY_UP,code,0,0,0)!=1)return 0;
        return window_focused&&(!down||gamepkg_input_key_state(device,code)==1)?GAMEPKG_SDL3_PASS:0;
    }
    case SDL_EVENT_TEXT_INPUT:return window?text(event->text.text):0;
    case SDL_EVENT_TEXT_EDITING:case SDL_EVENT_TEXT_EDITING_CANDIDATES:return window_focused?GAMEPKG_SDL3_PASS:0;
    case SDL_EVENT_MOUSE_MOTION:{
        if(!window||!isfinite(event->motion.x)||!isfinite(event->motion.y)||!isfinite(event->motion.xrel)||!isfinite(event->motion.yrel))return 0;
        if(event->motion.which==SDL_TOUCH_MOUSEID)return window_focused?GAMEPKG_SDL3_PASS:0;
        uint64_t device=gamepkg_input_connect(GAMEPKG_INPUT_MOUSE,BACKEND,event->motion.which);
        if(submit(device,GAMEPKG_INPUT_POINTER,0,event->motion.x,event->motion.y,0)!=1)return 0;
        if((event->motion.xrel||event->motion.yrel)&&submit(device,GAMEPKG_INPUT_RELATIVE,0,event->motion.xrel,event->motion.yrel,0)!=1)return 0;
        return window_focused?GAMEPKG_SDL3_PASS:0;
    }
    case SDL_EVENT_MOUSE_BUTTON_DOWN:case SDL_EVENT_MOUSE_BUTTON_UP:{
        if(!window)return 0;
        if(event->button.which==SDL_TOUCH_MOUSEID)return window_focused?GAMEPKG_SDL3_PASS:0;
        unsigned code=mouse_code(event->button.button);if(!code)return 0;
        uint64_t device=gamepkg_input_connect(GAMEPKG_INPUT_MOUSE,BACKEND,event->button.which);
        bool down=event->type==SDL_EVENT_MOUSE_BUTTON_DOWN;
        if(submit(device,down?GAMEPKG_INPUT_MOUSE_DOWN:GAMEPKG_INPUT_MOUSE_UP,code,0,0,0)!=1)return 0;
        gamepkg_input_pointer state;
        return window_focused&&(!down||(gamepkg_input_pointer_state(device,&state)==1&&(state.buttons&(1u<<(code-1)))))?GAMEPKG_SDL3_PASS:0;
    }
    case SDL_EVENT_MOUSE_WHEEL:{
        if(!window||!isfinite(event->wheel.x)||!isfinite(event->wheel.y))return 0;
        if(event->wheel.which==SDL_TOUCH_MOUSEID)return window_focused?GAMEPKG_SDL3_PASS:0;
        float x=event->wheel.x,y=event->wheel.y;
        if(event->wheel.direction==SDL_MOUSEWHEEL_FLIPPED){x=-x;y=-y;}
        uint64_t device=gamepkg_input_connect(GAMEPKG_INPUT_MOUSE,BACKEND,event->wheel.which);
        if(submit(device,GAMEPKG_INPUT_WHEEL,0,x,y,0)!=1)return 0;
        return window_focused?GAMEPKG_SDL3_PASS:0;
    }
    case SDL_EVENT_FINGER_DOWN:case SDL_EVENT_FINGER_MOTION:case SDL_EVENT_FINGER_UP:case SDL_EVENT_FINGER_CANCELED:return window?touch(&event->tfinger):0;
    default:return GAMEPKG_SDL3_PASS;
    }
}
