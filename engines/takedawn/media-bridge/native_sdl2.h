/* SPDX-License-Identifier: Apache-2.0
 * SDL2 snapshots feed the shared service from the existing engine poll/pump.
 * The engine's Duke mapping, glyph selection and rumble lifetime stay intact. */
#ifndef GAMEPKG_NATIVE_SDL2_H
#define GAMEPKG_NATIVE_SDL2_H
#include "../../../include/gamepkg_input_backend.h"
#define GAMEPKG_SDL2_BACKEND 2u
#define GAMEPKG_SDL2_BUTTONS 21
/* SDL2 button index -> common physical-position bit. Keep Guide/Misc,
 * touchpad and paddles even though the original Duke has no such buttons. */
static const unsigned gamepkg_sdl2_bits[GAMEPKG_SDL2_BUTTONS] = {
 12,13,14,15,5,10,4,6,7,8,9,0,1,2,3,11,17,18,19,20,16
};
static int16_t gamepkg_sdl2_invert(int16_t value) {
 return (int16_t)(-1 - (int)value);
}
static uint16_t gamepkg_sdl2_trigger(int16_t value) {
 unsigned v = value > 0 ? (unsigned)value : 0;
 /* Exact inverse is >>1, including 32767 -> 65535. No extra deadzone. */
 return (uint16_t)((v << 1) | (v >> 14));
}
static void gamepkg_sdl2_pack(const uint8_t buttons[GAMEPKG_SDL2_BUTTONS],
 const int16_t axes[6],uint64_t device,gamepkg_input_gamepad *report) {
 memset(report,0,sizeof *report);report->device=device;
 report->timestamp_ns=gamepkg_input_time_ns();
 for(unsigned b=0;b<GAMEPKG_SDL2_BUTTONS;b++)
  if(buttons[b])report->buttons |= UINT32_C(1)<<gamepkg_sdl2_bits[b];
 report->left_x=axes[0];report->left_y=gamepkg_sdl2_invert(axes[1]);
 report->right_x=axes[2];report->right_y=gamepkg_sdl2_invert(axes[3]);
 report->left_trigger=gamepkg_sdl2_trigger(axes[4]);
 report->right_trigger=gamepkg_sdl2_trigger(axes[5]);
}
static void gamepkg_sdl2_unpack(const gamepkg_input_gamepad *report,XBOX_HOST_SDL_GAMEPAD *pad) {
 memset(pad,0,sizeof *pad);
 for(unsigned b=0;b<XBOX_SDL_BUTTON_COUNT;b++)
  pad->button[b]=(report->buttons & (UINT32_C(1)<<gamepkg_sdl2_bits[b])) ? 1 : 0;
 pad->axis[0]=report->left_x;pad->axis[1]=gamepkg_sdl2_invert(report->left_y);
 pad->axis[2]=report->right_x;pad->axis[3]=gamepkg_sdl2_invert(report->right_y);
 pad->axis[4]=(int16_t)(report->left_trigger>>1);
 pad->axis[5]=(int16_t)(report->right_trigger>>1);
}
#ifndef GAMEPKG_SDL2_MAP_ONLY
static void gamepkg_sdl2_sample(SDL_GameController *controller,uint64_t device,int pressed_button) {
 uint8_t buttons[GAMEPKG_SDL2_BUTTONS]={0};int16_t axes[6];gamepkg_input_gamepad report;
 for(int b=0;b<GAMEPKG_SDL2_BUTTONS && b<SDL_CONTROLLER_BUTTON_MAX;b++)
  buttons[b]=SDL_GameControllerGetButton(controller,(SDL_GameControllerButton)b)?1:0;
 for(int a=0;a<6;a++)axes[a]=SDL_GameControllerGetAxis(controller,(SDL_GameControllerAxis)a);
 /* A queued press can already have been released in SDL's current snapshot.
  * Preserve its ownership edge; the next actual poll supplies the release. */
 if(pressed_button>=0 && pressed_button<GAMEPKG_SDL2_BUTTONS)buttons[pressed_button]=1;
 gamepkg_sdl2_pack(buttons,axes,device,&report);gamepkg_input_update_gamepad(&report);
}
static void gamepkg_sdl2_background(int on) {
 gamepkg_input_focus(on?0u:1u);xbox_InputSetBackground(on);
}
#define xbox_InputSetBackground gamepkg_sdl2_background
#endif
#endif
