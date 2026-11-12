/* Actual original device dispatch with observable backend callbacks. */
#include <assert.h>
#include <limits.h>
#ifndef GAMEPKG_SOUND_DEVICES_SOURCE
#define GAMEPKG_SOUND_DEVICES_SOURCE "../engines/doom/upstream/i_sound.c"
#endif
#include GAMEPKG_SOUND_DEVICES_SOURCE
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*)==4,"actual 32-bit process required");
#endif
static unsigned initializations,volumes;
static boolean init(void) {initializations++;return true;}
static void volume(int value) {assert(value==13);volumes++;}
static snddevice_t supported[]={SNDDEVICE_ADLIB,SNDDEVICE_SB};
sound_module_t DG_sound_module={0};
music_module_t DG_music_module={.sound_devices=supported,.num_sound_devices=2,
  .Init=init,.SetMusicVolume=volume};
int main(void) {
  const int devices[]={INT_MIN,-1,0,1,2,3,4,5,6,7,8,9,INT_MAX};
  for(unsigned i=0;i<sizeof(devices)/sizeof(*devices);i++) {
    initializations=volumes=0;
    snd_musicdevice=devices[i];
    InitMusicModule();
    unsigned enabled=devices[i]==SNDDEVICE_ADLIB || devices[i]==SNDDEVICE_SB;
    assert((music_module!=NULL)==enabled);
    I_InitMusic();
    I_SetMusicVolume(13);
    assert(initializations==enabled && volumes==enabled);
  }
  /* Selection follows the actual backend list, including future replacements. */
  supported[0]=SNDDEVICE_GENMIDI;
  DG_music_module.num_sound_devices=1;
  snd_musicdevice=SNDDEVICE_GENMIDI;InitMusicModule();assert(music_module);
  snd_musicdevice=SNDDEVICE_ADLIB;InitMusicModule();assert(!music_module);
  puts("Music device selection honors supported devices and disables callbacks for unavailable devices.");
  return 0;
}
