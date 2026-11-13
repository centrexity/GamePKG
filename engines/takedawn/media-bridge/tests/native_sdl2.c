/* Real pinned SDL backend/core + Rust service. Virtual devices only. */
#include "input_backend.h"
#include "input_map.h"
#include "pad_glyphs.h"
#include <SDL.h>
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define GAMEPKG_SDL2_MAP_ONLY
#ifndef GAMEPKG_SDL2_TEST_HEADER
#define GAMEPKG_SDL2_TEST_HEADER "native_sdl2.h"
#endif
#include GAMEPKG_SDL2_TEST_HEADER
extern void xbox_InputPadEvent(const SDL_Event *event);
extern void xbox_InputSetMainPump(int on);
static int window_pump;
static unsigned glyph_callbacks;
static void glyph_poll(int set){(void)set;XBOX_INPUT_STATE state;++glyph_callbacks;xbox_input_host_backend.get_state(0,&state);}
static unsigned calls[2];static uint16_t left[2],right[2];
static int SDLCALL rumble(void *user,Uint16 low,Uint16 high) {
 unsigned slot=(unsigned)(uintptr_t)user;assert(slot<2);++calls[slot];left[slot]=low;right[slot]=high;return 0;
}
static void mapping(void) {
 uint8_t buttons[GAMEPKG_SDL2_BUTTONS]={0};int16_t axes[6]={0};
 const uint32_t expected[]={GAMEPKG_INPUT_PAD_SOUTH,GAMEPKG_INPUT_PAD_EAST,
  GAMEPKG_INPUT_PAD_WEST,GAMEPKG_INPUT_PAD_NORTH,GAMEPKG_INPUT_PAD_BACK,
  GAMEPKG_INPUT_PAD_GUIDE,GAMEPKG_INPUT_PAD_START,GAMEPKG_INPUT_PAD_LEFT_STICK,
  GAMEPKG_INPUT_PAD_RIGHT_STICK,GAMEPKG_INPUT_PAD_LEFT_SHOULDER,GAMEPKG_INPUT_PAD_RIGHT_SHOULDER,
  GAMEPKG_INPUT_PAD_DPAD_UP,GAMEPKG_INPUT_PAD_DPAD_DOWN,GAMEPKG_INPUT_PAD_DPAD_LEFT,
  GAMEPKG_INPUT_PAD_DPAD_RIGHT,GAMEPKG_INPUT_PAD_MISC,GAMEPKG_INPUT_PAD_PADDLE_1,
  GAMEPKG_INPUT_PAD_PADDLE_2,GAMEPKG_INPUT_PAD_PADDLE_3,GAMEPKG_INPUT_PAD_PADDLE_4,
  GAMEPKG_INPUT_PAD_TOUCHPAD};
 for(unsigned bit=0;bit<GAMEPKG_SDL2_BUTTONS;bit++) {
  gamepkg_input_gamepad report;memset(buttons,0,sizeof buttons);buttons[bit]=1;
  gamepkg_sdl2_pack(buttons,axes,1,&report);
  assert(report.buttons==expected[bit]);
 }
 for(unsigned value=0;value<65536;value++) {
  XBOX_HOST_SDL_GAMEPAD original,roundtrip;XBOX_GAMEPAD before,after;gamepkg_input_gamepad report;
  memset(&original,0,sizeof original);memset(buttons,0,sizeof buttons);
  for(unsigned b=0;b<XBOX_SDL_BUTTON_COUNT;b++)original.button[b]=buttons[b]=(uint8_t)((value>>b)&1u);
  for(unsigned a=0;a<6;a++)original.axis[a]=axes[a]=(int16_t)(uint16_t)(value+a*7919u);
  gamepkg_sdl2_pack(buttons,axes,1,&report);gamepkg_sdl2_unpack(&report,&roundtrip);
  assert(report.left_x==axes[0]&&report.left_y==-1-(int)axes[1]);
  assert(report.right_x==axes[2]&&report.right_y==-1-(int)axes[3]);
  unsigned lt=axes[4]>0?(unsigned)axes[4]:0,rt=axes[5]>0?(unsigned)axes[5]:0;
  assert(report.left_trigger==((lt<<1)|(lt>>14))&&report.right_trigger==((rt<<1)|(rt>>14)));
  xbox_InputMapSdl(&original,&before);xbox_InputMapSdl(&roundtrip,&after);
  assert(!memcmp(&before,&after,sizeof before));
 }
}
static int attach(unsigned slot) {
 SDL_VirtualJoystickDesc descriptor;SDL_zero(descriptor);
 descriptor.version=SDL_VIRTUAL_JOYSTICK_DESC_VERSION;descriptor.type=SDL_JOYSTICK_TYPE_GAMECONTROLLER;
 descriptor.naxes=SDL_CONTROLLER_AXIS_MAX;descriptor.nbuttons=SDL_CONTROLLER_BUTTON_MAX;
 descriptor.axis_mask=(1u<<SDL_CONTROLLER_AXIS_MAX)-1u;
 descriptor.button_mask=(1u<<SDL_CONTROLLER_BUTTON_MAX)-1u;
 descriptor.name=slot?"GamePKG virtual second":"GamePKG virtual first";
 descriptor.userdata=(void*)(uintptr_t)slot;descriptor.Rumble=rumble;
 int index=SDL_JoystickAttachVirtualEx(&descriptor);assert(index>=0&&SDL_IsGameController(index));return index;
}
static void focus(int gained) {
 SDL_Event event;SDL_zero(event);event.type=SDL_WINDOWEVENT;
 event.window.event=gained?SDL_WINDOWEVENT_FOCUS_GAINED:SDL_WINDOWEVENT_FOCUS_LOST;
 xbox_InputPadEvent(&event);
}
static void poll(void) {
 if(window_pump){SDL_Event event;SDL_GameControllerUpdate();while(SDL_PollEvent(&event))xbox_InputPadEvent(&event);}
 XBOX_INPUT_STATE state;xbox_InputGetState(0,&state);xbox_InputGetState(1,&state);
}
int main(int argc,char **argv) {
 assert(argc==1||(argc==2&&!strcmp(argv[1],"--window-pump")));window_pump=argc==2;
 mapping();
 /* Refuse to enumerate/open any real controller through the engine. */
 SDL_SetHint(SDL_HINT_NO_SIGNAL_HANDLERS,"1");
 const char *drivers[]={"SDL_JOYSTICK_HIDAPI","SDL_JOYSTICK_MFI","SDL_JOYSTICK_IOKIT","SDL_JOYSTICK_RAWINPUT","SDL_JOYSTICK_LINUX_CLASSIC"};
 for(unsigned i=0;i<sizeof drivers/sizeof *drivers;i++)SDL_SetHint(drivers[i],"0");
 assert(SDL_Init(SDL_INIT_GAMECONTROLLER|SDL_INIT_EVENTS)==0);
 assert(SDL_NumJoysticks()==0);
 int index0=attach(0),index1=attach(1);
 SDL_Joystick *a=SDL_JoystickOpen(index0),*b=SDL_JoystickOpen(index1);assert(a&&b);
 SDL_JoystickID aid=SDL_JoystickInstanceID(a),bid=SDL_JoystickInstanceID(b);
 xbox_InputSetMainPump(window_pump);xbox_InputInit();if(window_pump)focus(1);poll();
 xbox_InputSetActivePad(XBOX_PAD_FAMILY_PLAYSTATION,0);xbox_InputSetGlyphListener(glyph_poll);
 unsigned callbacks_before=glyph_callbacks;
 gamepkg_input_gamepad player0,player1;
 assert(gamepkg_input_gamepad_state(0,&player0)==0);
 /* Axis drift cannot claim a slot, and the second physical pad can own P1. */
 assert(SDL_JoystickSetVirtualAxis(a,SDL_CONTROLLER_AXIS_LEFTX,32000)==0);poll();
 assert(gamepkg_input_gamepad_state(0,&player0)==0);
 assert(SDL_JoystickSetVirtualButton(b,SDL_CONTROLLER_BUTTON_A,1)==0);poll();
 assert(gamepkg_input_gamepad_state(0,&player0)==1&&player0.buttons==GAMEPKG_INPUT_PAD_SOUTH);
 assert(glyph_callbacks>callbacks_before);
 uint64_t first=player0.device;
 XBOX_INPUT_STATE state;assert(xbox_input_host_backend.get_state(0,&state)==0&&state.Gamepad.bAnalogButtons[0]==255);
 assert(SDL_JoystickSetVirtualButton(a,SDL_CONTROLLER_BUTTON_B,1)==0);poll();
 assert(gamepkg_input_gamepad_state(1,&player1)==1&&player1.buttons==GAMEPKG_INPUT_PAD_EAST);
 uint64_t second=player1.device;
 XBOX_VIBRATION vibration={12345,54321};
 assert(xbox_input_host_backend.set_state(0,&vibration)==0&&left[1]==12345&&right[1]==54321&&!calls[0]);
 assert(xbox_input_host_backend.set_state(1,&vibration)==0&&left[0]==12345);
 focus(0);poll();
 assert(gamepkg_input_gamepad_state(0,&player0)==1&&!player0.buttons&&!player0.left_x&&!player0.left_trigger);
 assert(left[0]==0&&left[1]==0);
 focus(1);poll();assert(gamepkg_input_gamepad_state(0,&player0)==1&&!player0.buttons);
 assert(SDL_JoystickSetVirtualButton(b,SDL_CONTROLLER_BUTTON_A,0)==0);poll();
 assert(SDL_JoystickSetVirtualButton(b,SDL_CONTROLLER_BUTTON_A,1)==0);poll();
 assert(gamepkg_input_gamepad_state(0,&player0)==1&&player0.buttons==GAMEPKG_INPUT_PAD_SOUTH);
 /* Detaching P2 must not move P1, and a reused SDL index gets a new handle. */
 assert(SDL_JoystickDetachVirtual(index0)==0);poll();
 assert(gamepkg_input_gamepad_state(0,&player0)==1&&player0.device==first);
 assert(gamepkg_input_gamepad_state(1,&player1)==0);
 SDL_JoystickClose(a);index0=attach(0);a=SDL_JoystickOpen(index0);assert(a&&SDL_JoystickInstanceID(a)!=aid);
 if(window_pump) {
  /* Consume the add event first: SDL only emits controller-button events
   * after the engine has opened its GameController handle. */
  poll();assert(gamepkg_input_gamepad_state(1,&player1)==0);
  /* A complete tap may already be released in SDL's current snapshot when
   * its queued down event is consumed. It still claims P2, without sticking. */
  assert(SDL_JoystickSetVirtualButton(a,SDL_CONTROLLER_BUTTON_X,1)==0);SDL_GameControllerUpdate();
  assert(SDL_JoystickSetVirtualButton(a,SDL_CONTROLLER_BUTTON_X,0)==0);SDL_GameControllerUpdate();poll();
  assert(gamepkg_input_gamepad_state(1,&player1)==1&&!player1.buttons);
 }
 assert(SDL_JoystickSetVirtualButton(a,SDL_CONTROLLER_BUTTON_X,1)==0);poll();
 assert(gamepkg_input_gamepad_state(1,&player1)==1&&player1.device!=second&&player1.buttons==GAMEPKG_INPUT_PAD_WEST);
 assert(SDL_JoystickInstanceID(b)==bid);
 xbox_InputSetGlyphListener(NULL);xbox_InputShutdown();
 assert(gamepkg_input_gamepad_state(0,&player0)==0&&gamepkg_input_gamepad_state(1,&player1)==0);
 assert(left[0]==0&&left[1]==0&&right[0]==0&&right[1]==0);
 SDL_JoystickClose(a);SDL_JoystickClose(b);SDL_Quit();
 puts("PASS native SDL2/Rust: 65536 original Duke mappings, 21 physical buttons, virtual device ownership, focus, hotplug, generation reuse, rumble routing and shutdown");return 0;
}
