// GamePKG stereo audio adapter; Zlib license, see ../upstream/readme_source.txt.
#include <stdint.h>
#include <stdlib.h>
// Fixed 100 ms stereo ring. SDL's dummy mixer never opens a host audio device.
static SDL_SpinLock gamepkg_audio_lock = 0;
static int16_t gamepkg_audio_ring[4800*2];
static unsigned gamepkg_audio_head = 0, gamepkg_audio_count = 0;
static void (*gamepkg_recording_mix)(void *, Uint8 *, int) = NULL;
static void *gamepkg_recording_userdata = NULL;
// The same lock fences callback removal before the recorder frees its state.
void gamepkg_audio_postmix(void (*callback)(void *, Uint8 *, int), void *userdata)
{
    SDL_AtomicLock(&gamepkg_audio_lock);
    gamepkg_recording_mix = callback;
    gamepkg_recording_userdata = userdata;
    SDL_AtomicUnlock(&gamepkg_audio_lock);
}
static void gamepkg_mix(void *, Uint8 *bytes, int length)
{
    if(length < 0 || length > 1048576 || length%4) return;
    const int16_t *samples = reinterpret_cast<const int16_t *>(bytes);
    SDL_AtomicLock(&gamepkg_audio_lock);
    for(int i=0;i<length/4;i++)
    {
        if(gamepkg_audio_count == 4800)
        {
            gamepkg_audio_head = (gamepkg_audio_head+1)%4800;
            gamepkg_audio_count--;
        }
        unsigned frame=(gamepkg_audio_head+gamepkg_audio_count)%4800;
        gamepkg_audio_ring[frame*2]=samples[i*2];
        gamepkg_audio_ring[frame*2+1]=samples[i*2+1];
        gamepkg_audio_count++;
    }
    if(gamepkg_recording_mix) gamepkg_recording_mix(gamepkg_recording_userdata, bytes, length);
    SDL_AtomicUnlock(&gamepkg_audio_lock);
}
void gamepkg_audio_hook()
{
    int frequency, channels; Uint16 format;
    if(!Mix_QuerySpec(&frequency, &format, &channels) || frequency != 48000 ||
       format != AUDIO_S16SYS || channels != 2) exit(2);
    Mix_SetPostMix(gamepkg_mix, NULL);
}
static unsigned gamepkg_audio_snapshot(unsigned char *pcm)
{
    SDL_AtomicLock(&gamepkg_audio_lock);
    unsigned count = gamepkg_audio_count;
    for(unsigned i=0;i<count;i++) for(unsigned channel=0;channel<2;channel++)
    {
        uint16_t sample=uint16_t(gamepkg_audio_ring[((gamepkg_audio_head+i)%4800)*2+channel]);
        unsigned offset=(i*2+channel)*2;
        pcm[offset]=sample&255; pcm[offset+1]=sample>>8;
    }
    gamepkg_audio_count = 0; gamepkg_audio_head = 0;
    SDL_AtomicUnlock(&gamepkg_audio_lock);
    return count;
}
