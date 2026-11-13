/* SPDX-License-Identifier: Apache-2.0
 * Canonical window input from the existing SDL2 pump. The legacy SDK's
 * layout-dependent keyboard and touch action mappings remain separate. */
#include "native_sdl2_events.h"
#include "../../../include/gamepkg_input_backend.h"
#include <math.h>
#include <stddef.h>
#include <string.h>
#define BACKEND 2u
static uint64_t find_device(uint32_t kind,uint64_t token) {
 gamepkg_input_device devices[GAMEPKG_INPUT_MAX_DEVICES];
 int count=gamepkg_input_devices(devices,GAMEPKG_INPUT_MAX_DEVICES);
 for(int i=0;i<count;i++)if(devices[i].backend==BACKEND&&devices[i].kind==kind&&devices[i].backend_token==token)return devices[i].device;
 return 0;
}
static uint64_t connect(uint32_t kind,uint64_t token) {
 return gamepkg_input_connect(kind,BACKEND,token);
}
static int submit(uint64_t device,uint32_t kind,uint32_t code,float x,float y,uint64_t contact) {
 if(!device)return 0;
 gamepkg_input_event event={0};event.device=device;event.timestamp_ns=gamepkg_input_time_ns();
 event.kind=kind;event.code=code;event.x=x;event.y=y;event.contact=contact;
 return gamepkg_input_submit(&event);
}
static unsigned usage(const SDL_KeyboardEvent *key) {
 SDL_Scancode scan=key->keysym.scancode;
 /* Native SDL supplies a physical usage. Synthetic legacy SDK fixtures
  * may only have a keycode; SDL's current-layout lookup is their fallback. */
 if(scan==SDL_SCANCODE_UNKNOWN)scan=SDL_GetScancodeFromKey(key->keysym.sym);
 return scan>SDL_SCANCODE_UNKNOWN&&scan<SDL_NUM_SCANCODES?(unsigned)scan:0;
}
static SDL_Keycode key_labels[SDL_NUM_SCANCODES];
static SDL_Keycode group(SDL_Keycode label) {
 if(label==SDLK_RSHIFT)return SDLK_LSHIFT;
 if(label==SDLK_RCTRL)return SDLK_LCTRL;
 if(label==SDLK_KP_ENTER)return SDLK_RETURN;
 return label;
}
int gamepkg_sdl2_label_held(SDL_Keycode label) {
 uint64_t keyboard=find_device(GAMEPKG_INPUT_KEYBOARD,0);
 if(!keyboard)return 0;
 label=group(label);
 for(unsigned scan=1;scan<SDL_NUM_SCANCODES;scan++)
  if(key_labels[scan]&&group(key_labels[scan])==label&&gamepkg_input_key_state(keyboard,scan)==1)return 1;
 return 0;
}
SDL_Keycode gamepkg_sdl2_release_label(const SDL_KeyboardEvent *event) {
 unsigned scan=usage(event);
 return scan&&key_labels[scan]?key_labels[scan]:event->keysym.sym;
}
static unsigned button(Uint8 sdl) {
 switch(sdl){case SDL_BUTTON_LEFT:return GAMEPKG_INPUT_MOUSE_LEFT;
 case SDL_BUTTON_RIGHT:return GAMEPKG_INPUT_MOUSE_RIGHT;
 case SDL_BUTTON_MIDDLE:return GAMEPKG_INPUT_MOUSE_MIDDLE;
 case SDL_BUTTON_X1:return GAMEPKG_INPUT_MOUSE_BACK;
 case SDL_BUTTON_X2:return GAMEPKG_INPUT_MOUSE_FORWARD;default:return 0;}
}
static void text(const char buffer[SDL_TEXTINPUTEVENT_TEXT_SIZE]) {
 const char *end=memchr(buffer,0,SDL_TEXTINPUTEVENT_TEXT_SIZE);
 if(!end)return;
 const unsigned char *p=(const unsigned char*)buffer,*limit=(const unsigned char*)end;
 uint32_t scalars[SDL_TEXTINPUTEVENT_TEXT_SIZE];unsigned count=0;
 while(p<limit){
  uint32_t code=*p++;unsigned extra;uint32_t minimum;
  if(code<0x80){extra=0;minimum=1;}
  else if(code>=0xc2&&code<=0xdf){code&=0x1f;extra=1;minimum=0x80;}
  else if(code>=0xe0&&code<=0xef){code&=0xf;extra=2;minimum=0x800;}
  else if(code>=0xf0&&code<=0xf4){code&=7;extra=3;minimum=0x10000;}
  else return;
  if((size_t)(limit-p)<extra)return;
  for(unsigned i=0;i<extra;i++){if((*p&0xc0)!=0x80)return;code=(code<<6)|(*p++&0x3f);}
  if(code<minimum||code>0x10ffff||(code>=0xd800&&code<=0xdfff))return;
  scalars[count++]=code;
 }
 if(!count)return;
 uint64_t keyboard=connect(GAMEPKG_INPUT_KEYBOARD,0);
 for(unsigned i=0;i<count;i++)submit(keyboard,GAMEPKG_INPUT_TEXT,scalars[i],0,0,0);
}
static int contact_state(uint64_t device,uint64_t finger,gamepkg_input_contact *out) {
 gamepkg_input_contact contacts[GAMEPKG_INPUT_MAX_DEVICES*GAMEPKG_INPUT_MAX_TOUCHES];
 int count=gamepkg_input_contacts(contacts,GAMEPKG_INPUT_MAX_DEVICES*GAMEPKG_INPUT_MAX_TOUCHES);
 for(int i=0;i<count;i++)if(contacts[i].device==device&&contacts[i].contact==finger){*out=contacts[i];return 1;}
 return 0;
}
struct layout_contact {uint64_t device,finger,proxy;};
static struct layout_contact layout_contacts[GAMEPKG_INPUT_MAX_DEVICES*GAMEPKG_INPUT_MAX_TOUCHES];
static uint64_t next_proxy,current_proxy,current_touch,current_finger;
static Uint32 current_type;
static int layout_contact(uint64_t device,uint64_t finger,Uint32 type) {
 struct layout_contact *found=NULL,*free_slot=NULL;
 for(unsigned i=0;i<sizeof layout_contacts/sizeof *layout_contacts;i++){
  if(layout_contacts[i].device==device&&layout_contacts[i].finger==finger)found=&layout_contacts[i];
  if(!layout_contacts[i].device&&!free_slot)free_slot=&layout_contacts[i];
 }
 if(type==SDL_FINGERDOWN&&!found){
  if(!free_slot||next_proxy==INT64_MAX)return 0;
  found=free_slot;found->device=device;found->finger=finger;found->proxy=++next_proxy;
 }
 if(!found)return 0;
 current_proxy=found->proxy;
 if(type==SDL_FINGERUP)memset(found,0,sizeof *found);
 return 1;
}
int64_t gamepkg_sdl2_layout_finger(const SDL_TouchFingerEvent *event) {
 if(!event||event->type!=current_type||(uint64_t)event->touchId!=current_touch||
    (uint64_t)event->fingerId!=current_finger)return 0;
 return (int64_t)current_proxy;
}
int gamepkg_sdl2_window_event(const SDL_Event *event) {
 if(!event)return 0;
 current_proxy=0;
 switch(event->type){
 case SDL_KEYDOWN:case SDL_KEYUP:{
  if(event->type==SDL_KEYDOWN&&event->key.repeat)return 1;
  unsigned code=usage(&event->key);if(!code)return 1;
  uint64_t keyboard=connect(GAMEPKG_INPUT_KEYBOARD,0);
  if(event->type==SDL_KEYDOWN&&gamepkg_input_key_state(keyboard,code)==1)return GAMEPKG_SDL2_EVENT_DROP;
  if(event->type==SDL_KEYDOWN){
   if(event->key.keysym.sym==SDLK_LGUI||event->key.keysym.sym==SDLK_RGUI)
    gamepkg_input_reset_device(keyboard);
   else if(event->key.keysym.mod&KMOD_GUI)return 1;
  }
  if(event->type==SDL_KEYDOWN)key_labels[code]=event->key.keysym.sym;
  submit(keyboard,event->type==SDL_KEYDOWN?GAMEPKG_INPUT_KEY_DOWN:GAMEPKG_INPUT_KEY_UP,code,0,0,0);
  return event->type==SDL_KEYUP||gamepkg_input_key_state(keyboard,code)==1;
 }
 case SDL_TEXTINPUT:text(event->text.text);break;
 case SDL_MOUSEMOTION:{
  if(event->motion.which==SDL_TOUCH_MOUSEID)return 1;
  uint64_t mouse=connect(GAMEPKG_INPUT_MOUSE,event->motion.which);
  submit(mouse,GAMEPKG_INPUT_POINTER,0,(float)event->motion.x,(float)event->motion.y,0);
  if(event->motion.xrel||event->motion.yrel)submit(mouse,GAMEPKG_INPUT_RELATIVE,0,(float)event->motion.xrel,(float)event->motion.yrel,0);
  break;
 }
 case SDL_MOUSEBUTTONDOWN:case SDL_MOUSEBUTTONUP:{
  if(event->button.which==SDL_TOUCH_MOUSEID)return 1;
  unsigned code=button(event->button.button);if(!code)return 1;
  submit(connect(GAMEPKG_INPUT_MOUSE,event->button.which),event->type==SDL_MOUSEBUTTONDOWN?GAMEPKG_INPUT_MOUSE_DOWN:GAMEPKG_INPUT_MOUSE_UP,code,0,0,0);break;
 }
 case SDL_MOUSEWHEEL:{
  if(event->wheel.which==SDL_TOUCH_MOUSEID)return 1;
  float x=(float)event->wheel.x,y=(float)event->wheel.y;
#if SDL_VERSION_ATLEAST(2,0,18)
  if(event->wheel.preciseX!=0.0f)x=event->wheel.preciseX;
  if(event->wheel.preciseY!=0.0f)y=event->wheel.preciseY;
#endif
  if(event->wheel.direction==SDL_MOUSEWHEEL_FLIPPED){x=-x;y=-y;}
  submit(connect(GAMEPKG_INPUT_MOUSE,event->wheel.which),GAMEPKG_INPUT_WHEEL,0,x,y,0);break;
 }
 case SDL_FINGERDOWN:case SDL_FINGERMOTION:case SDL_FINGERUP:{
  uint64_t token=(uint64_t)event->tfinger.touchId,finger=(uint64_t)event->tfinger.fingerId;
  uint64_t device=event->type==SDL_FINGERDOWN?connect(GAMEPKG_INPUT_TOUCH,token):find_device(GAMEPKG_INPUT_TOUCH,token);
  if(!device)return 0;
  gamepkg_input_contact previous;int was_down=contact_state(device,finger,&previous);
  float x=event->tfinger.x,y=event->tfinger.y;
  if(event->type==SDL_FINGERUP&&(!isfinite(x)||!isfinite(y)||x<0||x>1||y<0||y>1)){
   if(!was_down)return 0;
   x=previous.x;y=previous.y;
  }
  int accepted=submit(device,event->type==SDL_FINGERDOWN?GAMEPKG_INPUT_TOUCH_BEGIN:event->type==SDL_FINGERUP?GAMEPKG_INPUT_TOUCH_END:GAMEPKG_INPUT_TOUCH_MOVE,0,x,y,finger);
  if(accepted!=1)return 0;
  if(!(event->type==SDL_FINGERUP?was_down:contact_state(device,finger,&previous)))return 0;
  if(!layout_contact(device,finger,event->type)){gamepkg_input_reset_device(device);return 0;}
  current_touch=token;current_finger=finger;current_type=event->type;return 1;
 }
 case SDL_WINDOWEVENT:
  switch(event->window.event){
  case SDL_WINDOWEVENT_FOCUS_LOST:case SDL_WINDOWEVENT_HIDDEN:case SDL_WINDOWEVENT_MINIMIZED:
   gamepkg_input_focus(0);memset(layout_contacts,0,sizeof layout_contacts);return GAMEPKG_SDL2_EVENT_RESET;
  case SDL_WINDOWEVENT_FOCUS_GAINED:gamepkg_input_focus(1);break;
  case SDL_WINDOWEVENT_SHOWN:case SDL_WINDOWEVENT_RESTORED:{
   SDL_Window *window=SDL_GetWindowFromID(event->window.windowID);
   if(window){
    int focused=(SDL_GetWindowFlags(window)&SDL_WINDOW_INPUT_FOCUS)!=0;
    gamepkg_input_focus(focused?1u:0u);
    if(!focused){memset(layout_contacts,0,sizeof layout_contacts);return GAMEPKG_SDL2_EVENT_RESET;}
   }
   break;
  }
  default:break;
  }break;
 case SDL_APP_WILLENTERBACKGROUND:gamepkg_input_focus(0);memset(layout_contacts,0,sizeof layout_contacts);return GAMEPKG_SDL2_EVENT_RESET;
 case SDL_APP_DIDENTERFOREGROUND:
  if(SDL_GetKeyboardFocus())gamepkg_input_focus(1);
  else{gamepkg_input_focus(0);memset(layout_contacts,0,sizeof layout_contacts);return GAMEPKG_SDL2_EVENT_RESET;}
  break;
 default:break;
 }
 return 1;
}
void gamepkg_sdl2_window_shutdown(void) {
 memset(layout_contacts,0,sizeof layout_contacts);memset(key_labels,0,sizeof key_labels);current_proxy=0;
 gamepkg_input_device devices[GAMEPKG_INPUT_MAX_DEVICES];int count=gamepkg_input_devices(devices,GAMEPKG_INPUT_MAX_DEVICES);
 for(int i=0;i<count;i++)if(devices[i].backend==BACKEND&&devices[i].kind!=GAMEPKG_INPUT_GAMEPAD)gamepkg_input_disconnect(devices[i].device);
}
