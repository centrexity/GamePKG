/* Original pinned font, palette, blink, cell bounds and quit-frame snapshot. */
#include <assert.h>
#include <stdint.h>
#include <string.h>
static struct {uint32_t before,pixels[320*200],after;} screen;
#define DG_ScreenBuffer screen.pixels
static int quit_requested,netgame;
static unsigned quit_countdown;
#include "../engines/doom/adapters/endoom.h"
int main(void) {
  screen.before=0xdeadbeef;screen.after=0xcafebabe;
  for(unsigned i=0;i<320*200;i++)screen.pixels[i]=0x12345678;
  gamepkg_begin_quit();assert(quit_countdown==90&&quit_frame[63999]==0x12345678);
  assert(ending_mask==0);
  uint8_t text[4000];for(unsigned i=0;i<2000;i++){text[i*2]=219;text[i*2+1]=0x1e;}
  gamepkg_show_endoom(text);assert(quit_requested==1);render_ending();
  for(unsigned i=0;i<320*200;i++)assert(screen.pixels[i]==ending_colors[14]);
  for(unsigned i=0;i<2000;i++)text[i*2+1]=0x9e;
  gamepkg_show_endoom(text);render_ending();assert(screen.pixels[0]==ending_colors[1]);
  ending_frame=15;render_ending();assert(screen.pixels[0]==ending_colors[14]);
  for(unsigned i=0;i<2000;i++){text[i*2]=(uint8_t)i;text[i*2+1]=(uint8_t)i;}
  gamepkg_show_endoom(text);for(unsigned i=0;i<31;i++)render_ending();
  assert(screen.before==0xdeadbeef&&screen.after==0xcafebabe);
  quit_requested=0;quit_countdown=0;netgame=1;gamepkg_begin_quit();assert(quit_countdown==1);
  return 0;
}
