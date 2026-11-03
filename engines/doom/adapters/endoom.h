/* Bounded original ENDOOM text display. GPL-2.0-or-later.
 * Font and EGA colors follow pinned Chocolate DOOM libtextscreen.
 */
typedef struct {const char *name;const uint8_t *data;unsigned w,h;} txt_font_t;
#include "../music/upstream/small_font.h"
static uint8_t ending_text[80*25*2];
static uint32_t quit_frame[320*200];
static uint32_t ending_frame, ending_mask;
/* 0 = native color words; 1 = the original fixed little-endian gameplay stream. */
static uint8_t ending_pixels;
static uint32_t gamepkg_pixel_format(void) { return ending_pixels ? 0 : 1; }
static void gamepkg_gameplay_pixels(void) { ending_pixels = 0; }
static const uint32_t ending_colors[16]={0xff000000,0xff0000a8,0xff00a800,0xff00a8a8,0xffa80000,0xffa800a8,0xffa85400,0xffa8a8a8,0xff545454,0xff5454fe,0xff54fe54,0xff54fefe,0xfffe5454,0xfffe54fe,0xfffefe54,0xfffefefe};
void gamepkg_begin_quit(void) {
  if(quit_requested||quit_countdown) return;
  quit_countdown=netgame?1:90;
  memcpy(quit_frame,DG_ScreenBuffer,sizeof(quit_frame));
}
void gamepkg_show_endoom(const uint8_t *data) {
  memcpy(ending_text,data,sizeof(ending_text)); ending_frame=0; quit_requested=1;
}
static void render_ending(void) {
  for(unsigned cell=0;cell<80*25;cell++) {
    unsigned glyph=ending_text[cell*2], attributes=ending_text[cell*2+1];
    unsigned foreground=attributes&15, background=(attributes>>4)&7;
    if((attributes&128) && ((ending_frame/15)%2)==0) foreground=background;
    for(unsigned y=0;y<small_font.h;y++) for(unsigned x=0;x<small_font.w;x++) {
      unsigned bit=y*small_font.w+x;
      unsigned ink=(small_font.data[glyph*4+bit/8]>>(bit%8))&1;
      DG_ScreenBuffer[((cell/80)*8+y)*320+(cell%80)*4+x]=ending_colors[ink?foreground:background];
    }
  }
  ending_frame++;
  ending_pixels=1;
}
