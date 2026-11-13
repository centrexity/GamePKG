/* Main SDL owner hotplugs while another thread polls the actual backend. */
#include "input_backend.h"
#include "input_map.h"
#include "keyboard_sdl.h"
#include "../../../../include/gamepkg_input_backend.h"
#include <SDL.h>
#include <assert.h>
#include <stdio.h>
extern void xbox_InputPadEvent(const SDL_Event *event);
extern void xbox_InputSetMainPump(int on);
static SDL_atomic_t running,polls;
static int SDLCALL reader(void *unused){
 (void)unused;
 while(SDL_AtomicGet(&running)){
  for(DWORD port=0;port<4;port++){XBOX_INPUT_STATE state;DWORD result=xbox_input_host_backend.get_state(port,&state);assert(result==ERROR_SUCCESS||result==ERROR_DEVICE_NOT_CONNECTED);}
  SDL_AtomicAdd(&polls,1);SDL_Delay(0);
 }
 return 0;
}
static void pump(void){SDL_Event event;SDL_GameControllerUpdate();while(SDL_PollEvent(&event)){xbox_InputSdlEvent(&event);xbox_InputPadEvent(&event);}}
int main(void){
 const char *drivers[]={"SDL_JOYSTICK_HIDAPI","SDL_JOYSTICK_MFI","SDL_JOYSTICK_IOKIT","SDL_JOYSTICK_RAWINPUT","SDL_JOYSTICK_LINUX_CLASSIC"};
 for(unsigned i=0;i<sizeof drivers/sizeof *drivers;i++)SDL_SetHint(drivers[i],"0");
 SDL_SetHint(SDL_HINT_NO_SIGNAL_HANDLERS,"1");assert(SDL_Init(SDL_INIT_GAMECONTROLLER|SDL_INIT_EVENTS)==0);assert(SDL_NumJoysticks()==0);
 xbox_InputSetMainPump(1);xbox_InputSetDeadzone(0);xbox_InputSetFaceLayoutNintendo(0);xbox_InputInit();
 SDL_Event focus;SDL_zero(focus);focus.type=SDL_WINDOWEVENT;focus.window.event=SDL_WINDOWEVENT_FOCUS_GAINED;xbox_InputSdlEvent(&focus);xbox_InputPadEvent(&focus);
 SDL_AtomicSet(&running,1);SDL_Thread *thread=SDL_CreateThread(reader,"private-input-reader",NULL);assert(thread);
 uint64_t last_device=0;
 for(unsigned cycle=0;cycle<500;cycle++){
  SDL_VirtualJoystickDesc descriptor;SDL_zero(descriptor);descriptor.version=SDL_VIRTUAL_JOYSTICK_DESC_VERSION;descriptor.type=SDL_JOYSTICK_TYPE_GAMECONTROLLER;
  descriptor.naxes=SDL_CONTROLLER_AXIS_MAX;descriptor.nbuttons=SDL_CONTROLLER_BUTTON_MAX;descriptor.axis_mask=(1u<<SDL_CONTROLLER_AXIS_MAX)-1u;descriptor.button_mask=(1u<<SDL_CONTROLLER_BUTTON_MAX)-1u;descriptor.name="private hotplug virtual pad";
  int index=SDL_JoystickAttachVirtualEx(&descriptor);assert(index>=0&&SDL_JoystickIsVirtual(index));
  SDL_Joystick *joystick=SDL_JoystickOpen(index);assert(joystick);pump();
  assert(SDL_JoystickSetVirtualButton(joystick,SDL_CONTROLLER_BUTTON_A,1)==0);pump();SDL_Delay(1);
  gamepkg_input_gamepad joined;assert(gamepkg_input_gamepad_state(0,&joined)==1&&joined.device>last_device);last_device=joined.device;
  assert(SDL_JoystickDetachVirtual(index)==0);pump();SDL_JoystickClose(joystick);
  gamepkg_input_gamepad state;assert(gamepkg_input_gamepad_state(0,&state)==0);SDL_Delay(1);
 }
 /* The owner closes while the reader is still active. All further reads
  * must see a closed backend, never a freed controller handle. */
 xbox_InputShutdown();SDL_AtomicSet(&running,0);int result=-1;SDL_WaitThread(thread,&result);assert(result==0&&SDL_AtomicGet(&polls)>100);
 SDL_Quit();printf("PASS concurrent native SDL2 hotplug: 500 virtual reconnects, %d polling rounds, shutdown while polling\n",SDL_AtomicGet(&polls));return 0;
}
