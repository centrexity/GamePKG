/* Actual original angle routine against a wide integer/octant reference. */
#include <assert.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#ifndef GAMEPKG_ANGLE_SOURCE
#define GAMEPKG_ANGLE_SOURCE "../engines/doom/upstream/r_main.c"
#endif
#include GAMEPKG_ANGLE_SOURCE
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*)==4,"actual 32-bit process required");
#endif
static unsigned cases;
static uint64_t digest=UINT64_C(14695981039346656037);
static void check(int x1,int y1,int x2,int y2,int normal_only) {
  int64_t dx=(int64_t)x2-x1,dy=(int64_t)y2-y1;
  if(normal_only && (dx < -INT_MAX || dx > INT_MAX || dy < -INT_MAX || dy > INT_MAX)) return;
  uint64_t ax=dx<0?(uint64_t)-dx:(uint64_t)dx;
  uint64_t ay=dy<0?(uint64_t)-dy:(uint64_t)dy;
  uint64_t major=ax>ay?ax:ay,minor=ax>ay?ay:ax;
  uint64_t slope=major<512?2048:(minor*8%UINT64_C(4294967296))/(major/256);
  if(slope>2048)slope=2048;
  const uint32_t offset[]={0,ANG90-1,0,ANG270,ANG180-1,ANG90,ANG180,ANG270-1};
  const int sign[]={1,-1,-1,1,-1,1,1,-1};
  unsigned octant=(dx<0?4:0)+(dy<0?2:0)+(ax<=ay);
  uint32_t expected=major?(uint32_t)((int64_t)offset[octant]+sign[octant]*(int64_t)tantoangle[slope]):0;
  uint32_t actual=R_PointToAngle2(x1,y1,x2,y2);
  assert(actual==expected);
  assert(viewx==x1 && viewy==y1); // Original public routine's side effect.
  assert(R_PointToAngle(x2,y2)==actual);
  digest=(digest^actual)*UINT64_C(1099511628211);
  cases++;
}
int main(int argc,char **argv) {
  int normal_only=argc==2 && !strcmp(argv[1],"--normal-only");
  const int edges[]={INT_MIN,INT_MIN+1,-1073741824,-513,-512,-511,-1,0,1,511,512,513,1073741824,INT_MAX-1,INT_MAX};
  for(unsigned a=0;a<15;a++)for(unsigned b=0;b<15;b++)
    for(unsigned c=0;c<15;c++)for(unsigned d=0;d<15;d++)
      check(edges[a],edges[b],edges[c],edges[d],normal_only);
  uint32_t state=0x12345678u;
  for(unsigned i=0;i<200000;i++) {
    int coordinates[4];
    for(unsigned c=0;c<4;c++) {
      state^=state<<13;state^=state>>17;state^=state<<5;
      coordinates[c]=state<=INT_MAX?(int)state:(int)((int64_t)state-INT64_C(4294967296));
    }
    check(coordinates[0],coordinates[1],coordinates[2],coordinates[3],normal_only);
  }
  assert(cases==(normal_only?143042u:250625u));
  assert(digest==(normal_only?UINT64_C(0xfb6890b7cc5694a9):UINT64_C(0xaeae00b2766557e9)));
  printf("%u original angle cases passed; digest %016llx\n",cases,(unsigned long long)digest);
  return 0;
}
