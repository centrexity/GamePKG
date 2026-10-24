/* Exercise the unchanged original channel allocator with observable device IO.
 * No game assets, window, audio device or replacement allocation logic. */
#include <assert.h>
#include <string.h>
#include "../engines/doom/upstream/s_sound.c"
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*) == 4, "actual 32-bit process required");
#endif
static boolean playing[16];
static unsigned stops[16];
boolean I_SoundIsPlaying(int handle) {
  assert(handle>=0 && handle<16);
  return playing[handle];
}
void I_StopSound(int handle) {
  assert(handle>=0 && handle<16);
  assert(playing[handle]);
  playing[handle]=false;
  stops[handle]++;
}
static channel_t slots[16];
static sfxinfo_t effects[17];
static mobj_t origins[17];
static void reset(unsigned count) {
  memset(slots,0,sizeof(slots));
  memset(effects,0,sizeof(effects));
  memset(playing,0,sizeof(playing));
  memset(stops,0,sizeof(stops));
  channels=slots;
  snd_channels=(int)count;
  for(unsigned i=0;i<16;i++) slots[i].handle=(int)i;
}
static void fill(unsigned count,int priority) {
  reset(count);
  for(unsigned i=0;i<count;i++) {
    effects[i].priority=priority;
    effects[i].usefulness=1;
    slots[i].sfxinfo=&effects[i];
    slots[i].origin=&origins[i];
    playing[i]=true;
  }
}
int main(void) {
  for(unsigned count=0;count<=16;count++) {
    reset(count);
    for(unsigned i=0;i<count;i++) {
      assert(S_GetChannel(&origins[i],&effects[i])==(int)i);
      slots[i].handle=(int)i;
      effects[i].priority=10;
      effects[i].usefulness=1;
      playing[i]=true;
    }
    effects[16].priority=11; // Larger numbers mean lower priority.
    assert(S_GetChannel(&origins[16],&effects[16])==-1);
    for(unsigned i=0;i<count;i++) {
      assert(!stops[i] && playing[i]);
      assert(slots[i].sfxinfo==&effects[i]);
    }
    if(!count) continue;
    effects[16].priority=10;
    assert(S_GetChannel(&origins[16],&effects[16])==0);
    assert(stops[0]==1 && !playing[0] && effects[0].usefulness==0);
    assert(slots[0].sfxinfo==&effects[16]);
    for(unsigned selected=0;selected<count;selected++) {
      fill(count,10);
      effects[selected].priority=20;
      effects[16].priority=15;
      assert(S_GetChannel(&origins[16],&effects[16])==(int)selected);
      for(unsigned i=0;i<count;i++) {
        assert(stops[i]==(i==selected));
        assert(playing[i]==(i!=selected));
        assert(effects[i].usefulness==(i==selected?0:1));
      }
      fill(count,10);
      effects[16].priority=100; // Same-source replacement ignores priority.
      assert(S_GetChannel(&origins[selected],&effects[16])==(int)selected);
      assert(stops[selected]==1 && !playing[selected]);
      assert(effects[selected].usefulness==0);
      fill(count,10);
      S_StopSound(&origins[selected]);
      S_StopSound(&origins[selected]); // Repeated stop must be harmless.
      for(unsigned i=0;i<count;i++) {
        assert(stops[i]==(i==selected));
        assert(playing[i]==(i!=selected));
        assert((slots[i].sfxinfo==NULL)==(i==selected));
      }
      fill(count,10);
      playing[selected]=false; // The mixer already completed this voice.
      S_StopChannel((int)selected);
      assert(!stops[selected] && !slots[selected].sfxinfo);
      assert(effects[selected].usefulness==0);
    }
    fill(count,10);
    for(unsigned i=0;i<count;i++) slots[i].origin=NULL;
    effects[16].priority=100;
    assert(S_GetChannel(NULL,&effects[16])==-1);
    for(unsigned i=0;i<count;i++) assert(!stops[i]);
    fill(count,10);
    for(unsigned i=0;i<count;i++) slots[i].sfxinfo=&effects[0];
    effects[0].usefulness=(int)count;
    for(unsigned i=0;i<count;i++) {
      S_StopChannel((int)i);
      assert(effects[0].usefulness==(int)(count-i-1));
      for(unsigned j=i+1;j<count;j++) assert(playing[j]);
    }
    if(count>1) {
      fill(count,10);
      slots[0].sfxinfo=NULL;
      playing[0]=false;
      effects[16].priority=100;
      assert(S_GetChannel(&origins[count-1],&effects[16])==0);
      assert(!stops[count-1] && playing[count-1]);
    }
  }
  puts("Original allocation, priority, same-source replacement, null origins, shared effects and stop lifecycle pass for 0 through 16 channels.");
  return 0;
}
