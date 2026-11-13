/* Actual engine dispatcher + canonical Rust input, hidden dummy SDL window. */
#include "keyboard_sdl.h"
#include "keyboard.h"
#include "touch_pad.h"
#include "native_sdl2_events.h"
#include "../../../../include/gamepkg_input_backend.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
extern void xbox_InputPadEvent(const SDL_Event *event);
static Uint32 window_id;
static unsigned activity_count;
static void activity(void){++activity_count;}
static void dispatch(SDL_Event *event){xbox_InputSdlEvent(event);xbox_InputPadEvent(event);}
static void flush(void){gamepkg_input_event events[64];while(gamepkg_input_read(events,64)>0){}}
static uint64_t device(uint32_t kind,uint64_t token){
 gamepkg_input_device list[16];int count=gamepkg_input_devices(list,16);
 for(int i=0;i<count;i++)if(list[i].backend==2&&list[i].kind==kind&&list[i].backend_token==token)return list[i].device;
 return 0;
}
static void window_event(Uint8 kind){SDL_Event e;SDL_zero(e);e.type=SDL_WINDOWEVENT;e.window.windowID=window_id;e.window.event=kind;dispatch(&e);}
static void key(Uint32 type,SDL_Scancode scan,SDL_Keycode label,Uint16 mod,Uint8 repeat){
 SDL_Event e;SDL_zero(e);e.type=type;e.key.keysym.scancode=scan;e.key.keysym.sym=label;e.key.keysym.mod=mod;e.key.repeat=repeat;dispatch(&e);
}
static void mouse_button(Uint32 type,Uint8 button){SDL_Event e;SDL_zero(e);e.type=type;e.button.which=5;e.button.button=button;dispatch(&e);}
static void finger(Uint32 type,Sint64 surface,Sint64 id,float x,float y){
 SDL_Event e;SDL_zero(e);e.type=type;e.tfinger.windowID=window_id;e.tfinger.touchId=surface;e.tfinger.fingerId=id;e.tfinger.x=x;e.tfinger.y=y;dispatch(&e);
}
static void text(const char *bytes,size_t length){
 SDL_Event e;SDL_zero(e);e.type=SDL_TEXTINPUT;assert(length<=sizeof e.text.text);memcpy(e.text.text,bytes,length);dispatch(&e);
}
static void text_checks(void){
 const char valid[]="A\xc3\xa9\xf0\x9f\x8e\xae";flush();text(valid,sizeof valid);
 gamepkg_input_event events[64];int count=gamepkg_input_read(events,64);assert(count==3);
 const uint32_t codes[]={0x41,0xe9,0x1f3ae};for(int i=0;i<3;i++)assert(events[i].kind==GAMEPKG_INPUT_TEXT&&events[i].code==codes[i]);
 const unsigned char invalid[][5]={{0xc0,0xaf,0},{0xed,0xa0,0x80,0},{0xf4,0x90,0x80,0x80,0},{0xf0,0x9f,0},{'A',0xff,0}};
 for(unsigned i=0;i<sizeof invalid/sizeof *invalid;i++){text((const char*)invalid[i],sizeof *invalid);assert(gamepkg_input_read(events,64)==0);}
 char unterminated[32];memset(unterminated,'A',sizeof unterminated);text(unterminated,sizeof unterminated);assert(gamepkg_input_read(events,64)==0);
 /* Every byte position is exercised with every possible byte, including
  * missing terminators and truncated UTF-8 at the fixed buffer boundary. */
 for(unsigned position=0;position<32;position++)for(unsigned value=0;value<256;value++){
  char bytes[32];memset(bytes,'A',sizeof bytes);bytes[31]=0;bytes[position]=(char)value;text(bytes,sizeof bytes);flush();
 }
}
int main(int argc,char **argv){
 assert(argc==1||(argc==2&&!strcmp(argv[1],"--focus-control")));
 assert(SDL_Init(SDL_INIT_VIDEO)==0);
 SDL_Window *window=SDL_CreateWindow("private input fixture",0,0,640,480,SDL_WINDOW_HIDDEN);assert(window);window_id=SDL_GetWindowID(window);
 xbox_InputSdlInit();window_event(SDL_WINDOWEVENT_FOCUS_GAINED);
 /* Physical A can carry the Q label on AZERTY. Both views stay intact. */
 key(SDL_KEYDOWN,SDL_SCANCODE_A,SDLK_q,0,0);
 uint64_t keyboard=device(GAMEPKG_INPUT_KEYBOARD,0);assert(keyboard);
 assert(gamepkg_input_key_state(keyboard,SDL_SCANCODE_A)==1&&xbox_FramebufferKeyDown('Q')&&!xbox_FramebufferKeyDown('A'));
 flush();key(SDL_KEYDOWN,SDL_SCANCODE_A,SDLK_q,0,1);gamepkg_input_event events[64];assert(gamepkg_input_read(events,64)==0);
 /* The focus negative control skips this earlier duplicate-key probe so
  * it must fail specifically on background handling. The full positive
  * fixture continues to require both rejection behaviors. */
 if(argc==1){key(SDL_KEYDOWN,SDL_SCANCODE_A,SDLK_a,0,0);assert(gamepkg_input_read(events,64)==0&&xbox_FramebufferKeyDown('Q')&&!xbox_FramebufferKeyDown('A'));}
 /* A changed layout on key-up releases the label from the actual down. */
 key(SDL_KEYUP,SDL_SCANCODE_A,SDLK_a,0,0);assert(gamepkg_input_key_state(keyboard,SDL_SCANCODE_A)==0&&!xbox_FramebufferKeyDown('Q'));
 xbox_InputKeyboardSetActivityHook(activity);
 key(SDL_KEYDOWN,SDL_SCANCODE_LSHIFT,SDLK_LSHIFT,0,0);key(SDL_KEYDOWN,SDL_SCANCODE_RSHIFT,SDLK_RSHIFT,0,0);
 key(SDL_KEYUP,SDL_SCANCODE_LSHIFT,SDLK_LSHIFT,0,0);assert(xbox_FramebufferKeyDown(XBOX_VK_SHIFT)&&activity_count==2);
 key(SDL_KEYUP,SDL_SCANCODE_RSHIFT,SDLK_RSHIFT,0,0);assert(!xbox_FramebufferKeyDown(XBOX_VK_SHIFT)&&activity_count==2);
 xbox_InputKeyboardSetActivityHook(NULL);
 key(SDL_KEYDOWN,SDL_SCANCODE_RETURN,SDLK_RETURN,0,0);key(SDL_KEYDOWN,SDL_SCANCODE_KP_ENTER,SDLK_KP_ENTER,0,0);
 key(SDL_KEYUP,SDL_SCANCODE_RETURN,SDLK_RETURN,0,0);assert(xbox_FramebufferKeyDown(XBOX_VK_RETURN));
 key(SDL_KEYUP,SDL_SCANCODE_KP_ENTER,SDLK_KP_ENTER,0,0);assert(!xbox_FramebufferKeyDown(XBOX_VK_RETURN));
 key(SDL_KEYDOWN,SDL_SCANCODE_W,SDLK_w,0,0);
 uint64_t pad=device(GAMEPKG_INPUT_GAMEPAD,0);(void)pad;
 pad=gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD,99,0);gamepkg_input_gamepad report={0},state;
 report.device=pad;report.buttons=GAMEPKG_INPUT_PAD_SOUTH;report.left_x=123;assert(gamepkg_input_update_gamepad(&report)==1);
 key(SDL_KEYDOWN,SDL_SCANCODE_LGUI,SDLK_LGUI,KMOD_LGUI,0);
 assert(gamepkg_input_key_state(keyboard,SDL_SCANCODE_W)==0&&!xbox_FramebufferKeyDown('W'));
 assert(gamepkg_input_gamepad_state(0,&state)==1&&state.buttons==GAMEPKG_INPUT_PAD_SOUTH&&state.left_x==123);
 key(SDL_KEYDOWN,SDL_SCANCODE_W,SDLK_w,KMOD_LGUI,0);assert(!xbox_FramebufferKeyDown('W'));
 key(SDL_KEYUP,SDL_SCANCODE_W,SDLK_w,KMOD_LGUI,0);key(SDL_KEYUP,SDL_SCANCODE_LGUI,SDLK_LGUI,0,0);
 text_checks();
 SDL_Event e;SDL_zero(e);e.type=SDL_MOUSEMOTION;e.motion.which=5;e.motion.x=123;e.motion.y=456;e.motion.xrel=-7;e.motion.yrel=11;flush();dispatch(&e);
 uint64_t mouse=device(GAMEPKG_INPUT_MOUSE,5);assert(mouse);gamepkg_input_pointer pointer;
 assert(gamepkg_input_pointer_state(mouse,&pointer)==1&&pointer.x==123&&pointer.y==456);
 int count=gamepkg_input_read(events,64),absolute=0,relative=0;
 for(int i=0;i<count;i++){absolute+=events[i].kind==GAMEPKG_INPUT_POINTER;relative+=events[i].kind==GAMEPKG_INPUT_RELATIVE;}
 assert(absolute==1&&relative==1);
 const Uint8 buttons[]={SDL_BUTTON_LEFT,SDL_BUTTON_RIGHT,SDL_BUTTON_MIDDLE,SDL_BUTTON_X1,SDL_BUTTON_X2};
 for(unsigned i=0;i<5;i++){mouse_button(SDL_MOUSEBUTTONDOWN,buttons[i]);assert(gamepkg_input_pointer_state(mouse,&pointer)==1&&pointer.buttons==(1u<<i));mouse_button(SDL_MOUSEBUTTONUP,buttons[i]);}
 flush();e.motion.which=SDL_TOUCH_MOUSEID;dispatch(&e);assert(gamepkg_input_read(events,64)==0&&device(GAMEPKG_INPUT_MOUSE,SDL_TOUCH_MOUSEID)==0);
 SDL_zero(e);e.type=SDL_MOUSEWHEEL;e.wheel.which=5;e.wheel.direction=SDL_MOUSEWHEEL_FLIPPED;e.wheel.preciseX=.25f;e.wheel.preciseY=-.75f;dispatch(&e);
 assert(gamepkg_input_read(events,64)==1&&events[0].kind==GAMEPKG_INPUT_WHEEL&&events[0].x==-.25f&&events[0].y==.75f);
 /* Identical full-width finger IDs on two surfaces stay independent in
  * both canonical contacts and the SDK's single-ID touch action layout. */
 xbox_InputTouchSetMode(XBOX_TOUCH_ALWAYS);xbox_InputTouchSetScreen(640,480);
 xbox_touch_draw controls[16];count=xbox_InputTouchDrawList(controls,16);float ax=0,ay=0,bx=0,by=0;
 for(int i=0;i<count;i++)if(controls[i].label){if(!strcmp(controls[i].label,"A")){ax=controls[i].x/640;ay=controls[i].y/480;}if(!strcmp(controls[i].label,"B")){bx=controls[i].x/640;by=controls[i].y/480;}}
 assert(ax>0&&bx>0);const Sint64 first=INT64_C(1)<<50,second=INT64_C(2)<<50,same_finger=(INT64_C(1)<<60)|7;
 finger(SDL_FINGERDOWN,first,same_finger,ax,ay);finger(SDL_FINGERDOWN,second,same_finger,bx,by);
 gamepkg_input_contact contacts[16];assert(gamepkg_input_contacts(contacts,16)==2);
 assert(contacts[0].device!=contacts[1].device&&contacts[0].contact==(uint64_t)same_finger&&contacts[1].contact==(uint64_t)same_finger);
 XBOX_INPUT_STATE touch;assert(xbox_InputTouchState(&touch)&&touch.Gamepad.bAnalogButtons[XBOX_BUTTON_A]==255&&touch.Gamepad.bAnalogButtons[XBOX_BUTTON_B]==255);
 xbox_InputTouchState(&touch);finger(SDL_FINGERUP,first,same_finger,ax,ay);
 assert(gamepkg_input_contacts(contacts,16)==1&&xbox_InputTouchState(&touch)&&!touch.Gamepad.bAnalogButtons[XBOX_BUTTON_A]&&touch.Gamepad.bAnalogButtons[XBOX_BUTTON_B]==255);
 finger(SDL_FINGERUP,second,same_finger,NAN,NAN);assert(gamepkg_input_contacts(NULL,0)==0);xbox_InputTouchState(&touch);assert(!touch.Gamepad.bAnalogButtons[XBOX_BUTTON_B]);
 /* Focus/background changes release physical and mapped action views. */
 const Uint8 losses[]={SDL_WINDOWEVENT_FOCUS_LOST,SDL_WINDOWEVENT_HIDDEN,SDL_WINDOWEVENT_MINIMIZED};
 for(unsigned i=0;i<3;i++){
  window_event(SDL_WINDOWEVENT_FOCUS_GAINED);key(SDL_KEYUP,SDL_SCANCODE_W,SDLK_w,0,0);key(SDL_KEYDOWN,SDL_SCANCODE_W,SDLK_w,0,0);mouse_button(SDL_MOUSEBUTTONUP,SDL_BUTTON_LEFT);mouse_button(SDL_MOUSEBUTTONDOWN,SDL_BUTTON_LEFT);finger(SDL_FINGERDOWN,first,same_finger,ax,ay);
  window_event(losses[i]);assert(!xbox_FramebufferKeyDown('W')&&gamepkg_input_key_state(keyboard,SDL_SCANCODE_W)==0&&gamepkg_input_contacts(NULL,0)==0);
  assert(gamepkg_input_pointer_state(mouse,&pointer)==1&&!pointer.buttons);xbox_InputTouchState(&touch);assert(!touch.Gamepad.bAnalogButtons[XBOX_BUTTON_A]);
  key(SDL_KEYDOWN,SDL_SCANCODE_W,SDLK_w,0,0);finger(SDL_FINGERDOWN,first,same_finger,ax,ay);assert(!xbox_FramebufferKeyDown('W')&&gamepkg_input_contacts(NULL,0)==0);
  window_event(SDL_WINDOWEVENT_FOCUS_GAINED);key(SDL_KEYDOWN,SDL_SCANCODE_W,SDLK_w,0,0);assert(!xbox_FramebufferKeyDown('W'));
 }
 key(SDL_KEYUP,SDL_SCANCODE_W,SDLK_w,0,0);key(SDL_KEYDOWN,SDL_SCANCODE_W,SDLK_w,0,0);finger(SDL_FINGERDOWN,first,same_finger,ax,ay);
 SDL_zero(e);e.type=SDL_APP_WILLENTERBACKGROUND;dispatch(&e);assert(!xbox_FramebufferKeyDown('W')&&gamepkg_input_contacts(NULL,0)==0);xbox_InputTouchState(&touch);assert(!touch.Gamepad.bAnalogButtons[XBOX_BUTTON_A]);
 e.type=SDL_APP_DIDENTERFOREGROUND;assert(SDL_GetKeyboardFocus()==NULL);dispatch(&e);key(SDL_KEYDOWN,SDL_SCANCODE_W,SDLK_w,0,0);assert(!xbox_FramebufferKeyDown('W'));
 gamepkg_sdl2_window_shutdown();assert(device(GAMEPKG_INPUT_KEYBOARD,0)==0&&device(GAMEPKG_INPUT_MOUSE,5)==0);
 gamepkg_input_disconnect(pad);SDL_DestroyWindow(window);SDL_Quit();
 puts("PASS actual SDL2 window dispatcher/Rust: physical HID and key labels, repeats, device-only reset, Unicode/malformed UTF8, 8192 byte mutations, mouse buttons/motion/wheel, full-width multi-surface touch, lifecycle focus gates and shutdown");return 0;
}
