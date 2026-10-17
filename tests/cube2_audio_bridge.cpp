// Test the production audio callback and wire encoder without SDL or game media.
#include <stdint.h>
#include <assert.h>
#include <vector>
typedef unsigned char Uint8;
typedef uint16_t Uint16;
typedef int SDL_SpinLock;
static const Uint16 AUDIO_S16SYS=0x8010;
static void SDL_AtomicLock(SDL_SpinLock *) {}
static void SDL_AtomicUnlock(SDL_SpinLock *) {}
static int Mix_QuerySpec(int *rate,Uint16 *format,int *channels)
{ *rate=48000;*format=AUDIO_S16SYS;*channels=2;return 1; }
static void (*callback)(void *,Uint8 *,int)=0;
static void Mix_SetPostMix(void (*mix)(void *,Uint8 *,int),void *) { callback=mix; }
#include "../engines/cube2/adapters/audio.h"
int main()
{
    gamepkg_audio_hook();assert(callback);
    int16_t source[]={0x1234,-0x2345,-32768,32767};
    callback(0,reinterpret_cast<Uint8 *>(source),sizeof(source));
    unsigned char pcm[4800*4];
    assert(gamepkg_audio_snapshot(pcm)==2);
    const unsigned char expected[]={0x34,0x12,0xbb,0xdc,0,0x80,0xff,0x7f};
    for(unsigned i=0;i<sizeof(expected);i++) assert(pcm[i]==expected[i]);
    assert(gamepkg_audio_snapshot(pcm)==0);
    callback(0,reinterpret_cast<Uint8 *>(source),3);
    callback(0,reinterpret_cast<Uint8 *>(source),-1);
    callback(0,reinterpret_cast<Uint8 *>(source),1048580);
    assert(gamepkg_audio_snapshot(pcm)==0);
    std::vector<int16_t> many(5000*2);
    for(int i=0;i<5000;i++) { many[i*2]=i;many[i*2+1]=-i; }
    callback(0,reinterpret_cast<Uint8 *>(many.data()),int(many.size()*2));
    assert(gamepkg_audio_snapshot(pcm)==4800);
    for(int i=0;i<4800;i++)
    {
        uint16_t left=uint16_t(i+200),right=uint16_t(-i-200);
        assert(pcm[i*4]==(left&255) && pcm[i*4+1]==(left>>8));
        assert(pcm[i*4+2]==(right&255) && pcm[i*4+3]==(right>>8));
    }
}
