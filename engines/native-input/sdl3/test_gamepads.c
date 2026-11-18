/* Maintainer fixture: actual SDL3 virtual gamepads and the real Rust service. */
#include "gamepkg_sdl3.h"
#include "../../halo/input/halo_gamepad_ports.h"
#include <assert.h>
#include <stdio.h>

static SDL_Gamepad *players[GAMEPKG_INPUT_MAX_PLAYERS];
static struct gamepkg_halo_ports halo_ports;
static void check_ports(int split,int idle,int zero,int one) {
    uint64_t devices[4]={0};int ports[4];
    for(unsigned i=0;i<4;i++) {
        gamepkg_input_gamepad state;
        if(gamepkg_sdl3_snapshot(players[i],&state)==1)devices[i]=state.device;
    }
    gamepkg_halo_map_ports(&halo_ports,devices,split,idle,ports);
    assert(ports[0]==zero&&ports[1]==one);
}
#ifdef GAMEPKG_HALO_MAPPING
extern int gamepkg_halo_mapping_equal(SDL_Gamepad *,const gamepkg_input_gamepad *);
#endif
static int SDLCALL other_thread(void *pad) {
    SDL_Gamepad *out[GAMEPKG_INPUT_MAX_PLAYERS];gamepkg_input_gamepad snapshot;
    assert(gamepkg_sdl3_gamepads(out)==-1);
    assert(gamepkg_sdl3_snapshot((SDL_Gamepad *)pad,&snapshot)==0);
    gamepkg_sdl3_disconnect(SDL_GetGamepadID((SDL_Gamepad*)pad));
    return 0;
}
static void flush(void) {gamepkg_input_event events[64];while(gamepkg_input_read(events,64)>0){}}
static SDL_Gamepad *open_pad(SDL_JoystickID *id) {
    SDL_VirtualJoystickDesc desc;
    SDL_INIT_INTERFACE(&desc);
    desc.type=SDL_JOYSTICK_TYPE_GAMEPAD;desc.naxes=SDL_GAMEPAD_AXIS_COUNT;
    desc.nbuttons=SDL_GAMEPAD_BUTTON_COUNT;desc.button_mask=(1u<<SDL_GAMEPAD_BUTTON_COUNT)-1;
    desc.axis_mask=(1u<<SDL_GAMEPAD_AXIS_COUNT)-1;desc.name="GamePKG private SDL3 fixture";
    *id=SDL_AttachVirtualJoystick(&desc);assert(*id);
    SDL_Gamepad *pad=SDL_OpenGamepad(*id);assert(pad);return pad;
}
static int pump(void) {
    SDL_PumpEvents();
    SDL_Event event;while(SDL_PollEvent(&event))gamepkg_sdl3_window_event(&event,1);
    return gamepkg_sdl3_gamepads(players);
}
static void button(SDL_Gamepad *pad,SDL_GamepadButton code,bool down) {
    assert(SDL_SetJoystickVirtualButton(SDL_GetGamepadJoystick(pad),(int)code,down));pump();
}
static void close_pad(SDL_Gamepad *pad,SDL_JoystickID id) {
    gamepkg_sdl3_disconnect(id);SDL_CloseGamepad(pad);assert(SDL_DetachVirtualJoystick(id));pump();
}
int main(void) {
    const char *drivers[]={"SDL_JOYSTICK_HIDAPI","SDL_JOYSTICK_RAWINPUT","SDL_JOYSTICK_XINPUT","SDL_JOYSTICK_DINPUT","SDL_JOYSTICK_WGI","SDL_JOYSTICK_GAMEINPUT","SDL_JOYSTICK_MFI","SDL_JOYSTICK_IOKIT","SDL_JOYSTICK_LINUX_CLASSIC"};
    for(unsigned i=0;i<sizeof drivers/sizeof *drivers;i++)SDL_SetHint(drivers[i],"0");
    assert(SDL_Init(SDL_INIT_GAMEPAD|SDL_INIT_EVENTS));
    int count=-1;SDL_JoystickID *ids=SDL_GetGamepads(&count);assert(ids&&count==0);SDL_free(ids);
    gamepkg_sdl3_focus(true);assert(pump()==0);
    SDL_JoystickID aid,bid,cid;
    SDL_Gamepad *a=open_pad(&aid),*b=open_pad(&bid);
    assert(SDL_SetJoystickVirtualAxis(SDL_GetGamepadJoystick(a),SDL_GAMEPAD_AXIS_LEFTX,31000));
    assert(pump()==0); /* drift cannot claim the keyboard player's pad */
    button(b,SDL_GAMEPAD_BUTTON_SOUTH,true);assert(players[0]==b&&!players[1]);
    gamepkg_input_gamepad state;assert(gamepkg_sdl3_snapshot(b,&state)==1);
    uint64_t first=state.device;
    assert(state.buttons==GAMEPKG_INPUT_PAD_SOUTH);
    check_ports(1,0,0,-1); /* held: don't duplicate the join action on P2 */
    assert(state.left_y==(int16_t)(-1-(int)SDL_GetGamepadAxis(b,SDL_GAMEPAD_AXIS_LEFTY)));
    button(b,SDL_GAMEPAD_BUTTON_SOUTH,false);
    check_ports(1,1,-1,0); /* release permits Halo's deliberate P2 shift */
    button(a,SDL_GAMEPAD_BUTTON_EAST,true);assert(players[0]==b&&players[1]==a);
    check_ports(1,0,1,0); /* second pad must not steal the existing P2 */
    button(a,SDL_GAMEPAD_BUTTON_EAST,false);
    /* Every named SDL3 button, including the five newer misc buttons, is
     * delivered at its physical position and gets a down/up queue edge. */
    static const unsigned expected[]={12,13,14,15,5,10,4,6,7,8,9,0,1,2,3,11,17,18,19,20,16,21,22,23,24,25};
    for(unsigned bit=0;bit<SDL_GAMEPAD_BUTTON_COUNT;bit++) {
        flush();
        button(b,(SDL_GamepadButton)bit,true);assert(gamepkg_sdl3_snapshot(b,&state)==1);
        assert(state.buttons==(1u<<expected[bit])&&gamepkg_sdl3_button(&state,(SDL_GamepadButton)bit));
#ifdef GAMEPKG_HALO_MAPPING
        assert(gamepkg_halo_mapping_equal(b,&state));
#endif
        gamepkg_input_event events[64];int count=gamepkg_input_read(events,64),downs=0;
        for(int i=0;i<count;i++)if(events[i].kind==GAMEPKG_INPUT_PAD_DOWN){assert(events[i].device==state.device&&events[i].code==expected[bit]);downs++;}
        assert(downs==1);
        button(b,(SDL_GamepadButton)bit,false);assert(gamepkg_sdl3_snapshot(b,&state)==1&&state.buttons==0);
        count=gamepkg_input_read(events,64);int ups=0;
        for(int i=0;i<count;i++)if(events[i].kind==GAMEPKG_INPUT_PAD_UP){assert(events[i].device==state.device&&events[i].code==expected[bit]);ups++;}
        assert(ups==1);
    }
    /* Raw precision and the original Y inversion are exact at every value.
     * Trigger coordinates are read from SDL's own mapping of the virtual
     * joystick, rather than assuming joystick and gamepad trigger ranges. */
    for(int value=-32768;value<=32767;value++) {
        SDL_Joystick *joy=SDL_GetGamepadJoystick(b);
        for(int axis=0;axis<SDL_GAMEPAD_AXIS_COUNT;axis++)assert(SDL_SetJoystickVirtualAxis(joy,axis,(Sint16)value));
        pump();assert(gamepkg_sdl3_snapshot(b,&state)==1);
        assert(state.left_y==(int16_t)(-1-(int)SDL_GetGamepadAxis(b,SDL_GAMEPAD_AXIS_LEFTY)));
        assert(state.right_y==(int16_t)(-1-(int)SDL_GetGamepadAxis(b,SDL_GAMEPAD_AXIS_RIGHTY)));
        for(int axis=0;axis<SDL_GAMEPAD_AXIS_COUNT;axis++)assert(gamepkg_sdl3_axis(&state,(SDL_GamepadAxis)axis)==SDL_GetGamepadAxis(b,(SDL_GamepadAxis)axis));
#ifdef GAMEPKG_HALO_MAPPING
        assert(gamepkg_halo_mapping_equal(b,&state));
#endif
    }
    button(b,SDL_GAMEPAD_BUTTON_START,true);gamepkg_sdl3_focus(false);pump();
    assert(gamepkg_sdl3_snapshot(b,&state)==1&&state.buttons==0&&state.left_x==0);
    gamepkg_sdl3_focus(true);pump();assert(gamepkg_sdl3_snapshot(b,&state)==1&&state.buttons==0);
    button(b,SDL_GAMEPAD_BUTTON_START,false);button(b,SDL_GAMEPAD_BUTTON_START,true);
    assert(gamepkg_sdl3_snapshot(b,&state)==1&&state.buttons==GAMEPKG_INPUT_PAD_START);
    SDL_Thread *thread=SDL_CreateThread(other_thread,"provider-wrong-thread",b);assert(thread);
    int thread_code=-1;SDL_WaitThread(thread,&thread_code);assert(thread_code==0);
    assert(gamepkg_sdl3_snapshot(b,&state)==1&&state.buttons==GAMEPKG_INPUT_PAD_START);
    close_pad(b,bid);assert(!players[0]&&players[1]==a); /* no slot compaction */
    check_ports(1,1,1,-1);
    SDL_Gamepad *c=open_pad(&cid);pump();button(c,SDL_GAMEPAD_BUTTON_SOUTH,true);
    assert(players[0]==c&&players[1]==a);assert(gamepkg_sdl3_snapshot(c,&state)==1&&state.device>first);
    check_ports(1,0,1,0); /* fresh generation fills only the free game port */
    /* The engine, rather than this adapter, owns SDL open/close lifetime. */
    gamepkg_sdl3_shutdown();assert(SDL_GamepadConnected(a)&&SDL_GamepadConnected(c));
    close_pad(a,aid);close_pad(c,cid);gamepkg_sdl3_shutdown();
    gamepkg_sdl3_focus(true);
    a=open_pad(&aid);b=open_pad(&bid);pump();flush();
    /* Real SDL reports queue both edges before the provider gets a pump.
     * b presses first even though a was connected/enumerated first. */
    assert(SDL_SetJoystickVirtualButton(SDL_GetGamepadJoystick(b),SDL_GAMEPAD_BUTTON_SOUTH,true));
    SDL_UpdateJoysticks();
    assert(SDL_SetJoystickVirtualButton(SDL_GetGamepadJoystick(b),SDL_GAMEPAD_BUTTON_SOUTH,false));
    SDL_UpdateJoysticks();
    assert(SDL_SetJoystickVirtualButton(SDL_GetGamepadJoystick(a),SDL_GAMEPAD_BUTTON_EAST,true));
    SDL_UpdateJoysticks();
    pump();
    int queued_order=players[0]==b&&players[1]==a;assert(queued_order);
    assert(gamepkg_sdl3_snapshot(b,&state)==1&&state.buttons==0);
    gamepkg_input_event queued[64];int queued_count=gamepkg_input_read(queued,64),tap_edges=0;
    for(int i=0;i<queued_count;i++)if(queued[i].device==state.device&&
        (queued[i].kind==GAMEPKG_INPUT_PAD_DOWN||queued[i].kind==GAMEPKG_INPUT_PAD_UP)) {
        assert(queued[i].kind==(tap_edges==0?GAMEPKG_INPUT_PAD_DOWN:GAMEPKG_INPUT_PAD_UP));
        assert(queued[i].code==12);tap_edges++;
    }
    assert(tap_edges==2);
    close_pad(a,aid);close_pad(b,bid);gamepkg_sdl3_shutdown();SDL_Quit();
    puts("PASS SDL3/Rust: 26 buttons, 65536 raw coordinate reports, queued taps and ordered ownership, drift, focus latch, stable slots and engine-owned lifecycle");return 0;
}
