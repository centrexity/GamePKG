/* Original attenuation routine with a controlled angle oracle. This tests
 * distance and pan separately from the renderer's coordinate-to-angle math. */
#include <assert.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#ifndef GAMEPKG_SOUND_SOURCE
#define GAMEPKG_SOUND_SOURCE "../engines/doom/upstream/s_sound.c"
#endif
#include GAMEPKG_SOUND_SOURCE
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*)==4,"actual 32-bit process required");
#endif
int gamemap;
#ifndef GAMEPKG_REAL_ANGLES
static angle_t direction;
static unsigned angle_calls;
angle_t R_PointToAngle2(fixed_t x1,fixed_t y1,fixed_t x2,fixed_t y2) {
  (void)x1;(void)y1;(void)x2;(void)y2;
  angle_calls++;
  return direction;
}
#endif
static unsigned cases;
static void check(int x,int y,int sx,int sy,int map,int gain) {
  mobj_t listener={.x=x,.y=y},source={.x=sx,.y=sy};
  gamemap=map;snd_SfxVolume=gain;
#ifndef GAMEPKG_REAL_ANGLES
  direction=ANG90;angle_calls=0;
#endif
  int volume=-123,separation=-456;
  int audible=S_AdjustSoundParams(&listener,&source,&volume,&separation);
  long double dx=(long double)x-sx,dy=(long double)y-sy;
  if(dx<0)dx=-dx;
  if(dy<0)dy=-dy;
  /* The original approximate metric floors half of the lesser raw distance. */
  uint64_t lesser=(uint64_t)(dx<dy?dx:dy);
  uint64_t distance=(uint64_t)(dx+dy)-lesser/2;
  if(map!=8 && distance>1200u*65536u) {
    assert(!audible && volume==-123 && separation==-456);
#ifndef GAMEPKG_REAL_ANGLES
    assert(!angle_calls);
#endif
  } else {
    if(distance>1200u*65536u) distance=1200u*65536u;
    int expected=distance<200u*65536u?gain:
      map==8?15+(gain-15)*(int)((1200u*65536u-distance)/65536u)/1000:
      gain*(int)((1200u*65536u-distance)/65536u)/1000;
    assert(volume==expected && audible==(expected>0));
#ifdef GAMEPKG_REAL_ANGLES
    assert(separation>=32 && separation<=224);
#else
    assert(angle_calls==1);
    assert(separation==33); // Original ANG90 fine-sine lookup and fixed multiply.
#endif
  }
  cases++;
}
int main(int argc,char **argv) {
  int normal_only=argc==2 && !strcmp(argv[1],"--normal-only");
  for(int map=1;map<=8;map+=7) for(int gain=0;gain<=127;gain++) {
    const int units[]={0,1,199,200,201,1199,1200,1201,2000};
    for(unsigned i=0;i<sizeof(units)/sizeof(*units);i++) for(int sign=-1;sign<=1;sign+=2) {
      int x=sign*units[i]*65536;
      check(0,0,x,0,map,gain);
      check(0,0,0,x,map,gain);
      check(0,0,x,x,map,gain);
    }
  }
  if(!normal_only) {
    const int edges[]={INT_MIN,INT_MIN+1,-1,0,1,INT_MAX-1,INT_MAX};
    for(unsigned a=0;a<7;a++) for(unsigned b=0;b<7;b++)
      for(unsigned c=0;c<7;c++) for(unsigned d=0;d<7;d++) {
        check(edges[a],edges[b],edges[c],edges[d],1,127);
        check(edges[a],edges[b],edges[c],edges[d],8,127);
      }
  }
#ifdef GAMEPKG_REAL_ANGLES
  printf("%u original spatial-distance cases passed with actual angle backend.\n",cases);
#else
  printf("%u original spatial-distance/pan cases passed (angle backend separately controlled).\n",cases);
#endif
  return 0;
}
