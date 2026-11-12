/* Execute original sound dispatch with synthetic device callbacks. */
#include <assert.h>
#include <limits.h>
#include <setjmp.h>
#include <string.h>
#ifndef GAMEPKG_SOUND_SOURCE
#define GAMEPKG_SOUND_SOURCE "../engines/doom/upstream/s_sound.c"
#endif
#include GAMEPKG_SOUND_SOURCE
#include "../engines/doom/upstream/sounds.c"
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*)==4,"actual 32-bit process required");
#endif
player_t players[MAXPLAYERS];
int consoleplayer, gamemap;
const fixed_t finesine[5*FINEANGLES/4]={0};
static jmp_buf error_jump;
static int errors,starts;
void I_Error(char *message,...) {
  assert(!strcmp(message,"Bad sfx #: %d"));
  errors++;
  longjmp(error_jump,1);
}
angle_t R_PointToAngle2(fixed_t a,fixed_t b,fixed_t c,fixed_t d) {
  (void)a;(void)b;(void)c;(void)d;assert(!"unexpected spatial path");return 0;
}
fixed_t FixedMul(fixed_t a,fixed_t b) {
  (void)a;(void)b;assert(!"unexpected spatial path");return 0;
}
boolean I_SoundIsPlaying(int handle) {assert(handle==0);return false;}
void I_StopSound(int handle) {(void)handle;assert(!"unexpected device stop");}
int I_GetSfxLumpNum(sfxinfo_t *effect) {
  assert(effect>=S_sfx+1 && effect<S_sfx+NUMSFX);return 42;
}
int I_StartSound(sfxinfo_t *effect,int channel,int volume,int separation) {
  assert(effect>=S_sfx+1 && effect<S_sfx+NUMSFX);
  assert(channel==0 && volume>0 && separation==128);
  starts++;return 0;
}
static void reject(int id) {
  if(!setjmp(error_jump)) {
    S_StartSound(NULL,id);
    assert(!"invalid sound ID was accepted");
  }
}
int main(void) {
  channel_t slot={0};channels=&slot;snd_channels=1;snd_SfxVolume=127;
  const int invalid[]={INT_MIN,-1,0,NUMSFX,NUMSFX+1,INT_MAX};
  for(unsigned i=0;i<sizeof(invalid)/sizeof(*invalid);i++) {
    int before=errors;
    reject(invalid[i]);
    assert(errors==before+1 && !starts && !slot.sfxinfo);
  }
  for(int i=1;i<NUMSFX;i++) {
    memset(&slot,0,sizeof(slot));
    int before=starts;
    S_StartSound(NULL,i);
    assert(starts==before+1 && slot.sfxinfo==&S_sfx[i]);
  }
  printf("Rejected all invalid sound-ID boundaries; dispatched all %d valid original sound IDs.\n",NUMSFX-1);
  return 0;
}
