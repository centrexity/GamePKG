#include "gamepkg_input_backend.h"
#include <assert.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
_Static_assert(sizeof(gamepkg_input_event)==40,"event ABI");
_Static_assert(sizeof(gamepkg_input_gamepad)==32,"pad ABI");
_Static_assert(sizeof(gamepkg_input_pointer)==32,"pointer ABI");
_Static_assert(sizeof(gamepkg_input_contact)==32,"touch ABI");
_Static_assert(sizeof(gamepkg_input_device)==32,"device ABI");
_Static_assert(offsetof(gamepkg_input_event,kind)==16,"event tag");
_Static_assert(offsetof(gamepkg_input_event,x)==24,"event value");
_Static_assert(offsetof(gamepkg_input_gamepad,left_x)==20,"pad axes");
_Static_assert(offsetof(gamepkg_input_gamepad,right_trigger)==30,"pad trigger");
static gamepkg_input_event event(uint64_t device,uint32_t kind,uint32_t code) {
 gamepkg_input_event e={0};e.device=device;e.kind=kind;e.code=code;if(kind>=8&&kind<=10){e.contact=code;e.code=0;}return e;
}
int main(void) {
 assert(gamepkg_input_focus(1)==1);
 uint64_t idle=gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD,3,0);
 uint64_t chosen=gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD,3,3);
 assert(idle&&chosen&&idle!=chosen);
 gamepkg_input_gamepad pad={0},state={0};
 pad.device=idle;pad.left_x=32767;assert(gamepkg_input_update_gamepad(&pad)==1);
 assert(gamepkg_input_gamepad_state(0,&state)==0);
 pad.device=chosen;pad.buttons=GAMEPKG_INPUT_PAD_SOUTH;pad.left_x=-32768;pad.right_trigger=65535;
 assert(gamepkg_input_update_gamepad(&pad)==1);assert(gamepkg_input_gamepad_state(0,&state)==1);
 assert(state.device==chosen&&state.buttons==GAMEPKG_INPUT_PAD_SOUTH&&state.left_x==-32768&&state.right_trigger==65535);
 uint64_t keyboard=gamepkg_input_connect(GAMEPKG_INPUT_KEYBOARD,1,0);
 gamepkg_input_event e=event(keyboard,GAMEPKG_INPUT_KEY_DOWN,4);
 assert(gamepkg_input_submit(&e)==1);assert(gamepkg_input_key_state(keyboard,4)==1);
 uint64_t touch=gamepkg_input_connect(GAMEPKG_INPUT_TOUCH,1,0);
 e=event(touch,GAMEPKG_INPUT_TOUCH_BEGIN,77);e.contact=(UINT64_C(1)<<40)|77;e.x=.25f;e.y=.75f;assert(gamepkg_input_submit(&e)==1);
 gamepkg_input_contact contact={0};assert(gamepkg_input_contacts(&contact,1)==1);
 assert(contact.device==touch&&contact.contact==((UINT64_C(1)<<40)|77)&&contact.x==.25f&&contact.y==.75f);
 assert(gamepkg_input_focus(0)==1);assert(gamepkg_input_gamepad_state(0,&state)==1&&state.buttons==0&&state.left_x==0);
 assert(gamepkg_input_key_state(keyboard,4)==0);assert(gamepkg_input_contacts(NULL,0)==0);
 assert(gamepkg_input_focus(1)==1);assert(gamepkg_input_update_gamepad(&pad)==1);
 assert(gamepkg_input_gamepad_state(0,&state)==1&&state.buttons==0);
 pad.buttons=0;assert(gamepkg_input_update_gamepad(&pad)==1);pad.buttons=GAMEPKG_INPUT_PAD_SOUTH;
 assert(gamepkg_input_update_gamepad(&pad)==1);assert(gamepkg_input_gamepad_state(0,&state)==1&&state.buttons==GAMEPKG_INPUT_PAD_SOUTH);
 assert(gamepkg_input_disconnect(chosen)==1);assert(gamepkg_input_gamepad_state(0,&state)==0&&state.device==0);
 uint64_t reconnected=gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD,3,3);assert(reconnected&&reconnected!=chosen);
 assert(gamepkg_input_update_gamepad(&pad)==0);pad.device=reconnected;assert(gamepkg_input_update_gamepad(&pad)==1);
 unsigned char unaligned[33];memcpy(unaligned+1,&pad,32);
 assert(gamepkg_input_update_gamepad((const gamepkg_input_gamepad*)(unaligned+1))==1);
 assert(gamepkg_input_gamepad_state(0,(gamepkg_input_gamepad*)(unaligned+1))==1);
 memcpy(&state,unaligned+1,32);assert(state.device==reconnected);
 assert(gamepkg_input_read(NULL,1)==-1);gamepkg_input_event events[2048];assert(gamepkg_input_read(events,2048)>0);
 assert(gamepkg_input_read(events,2048)==0);
 uint64_t mouse=gamepkg_input_connect(GAMEPKG_INPUT_MOUSE,1,0);assert(mouse);
 assert(gamepkg_input_read(events,2048)>0);
 for(unsigned i=0;i<=2048;i++){e=event(mouse,GAMEPKG_INPUT_RELATIVE,0);e.x=1;assert(gamepkg_input_submit(&e)==1);}
 assert(gamepkg_input_gamepad_state(0,&state)==1&&state.buttons==GAMEPKG_INPUT_PAD_SOUTH);
 assert(gamepkg_input_read(events,2048)==1&&events[0].kind==GAMEPKG_INPUT_RESET);
 printf("PASS actual C/Rust native input ABI: %zu-bit, player selection, focus, hotplug, keyboard, touch, precision, unaligned buffers\n",sizeof(void*)*8);
 return 0;
}
