// Test the production audio callback and wire encoder without SDL or game media.
#include <stdint.h>
#include <assert.h>
#include <vector>
#include <mutex>
#include <atomic>
#include <thread>
typedef unsigned char Uint8;
typedef uint16_t Uint16;
typedef int SDL_SpinLock;
static const Uint16 AUDIO_S16SYS=0x8010;
static std::mutex mixer_lock;
static void SDL_AtomicLock(SDL_SpinLock *) { mixer_lock.lock(); }
static void SDL_AtomicUnlock(SDL_SpinLock *) { mixer_lock.unlock(); }
static int Mix_QuerySpec(int *rate,Uint16 *format,int *channels)
{ *rate=48000;*format=AUDIO_S16SYS;*channels=2;return 1; }
static void (*callback)(void *,Uint8 *,int)=0;
static void Mix_SetPostMix(void (*mix)(void *,Uint8 *,int),void *) { callback=mix; }
#include "../engines/cube2/adapters/audio.h"
static void recording(void *userdata, Uint8 *bytes, int length)
{
    int *calls = static_cast<int *>(userdata); ++*calls;
    assert(length == 8);
    assert(reinterpret_cast<int16_t *>(bytes)[0] == 0x1234);
}
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
    int calls=0;
    gamepkg_audio_postmix(recording, &calls);
    callback(0,reinterpret_cast<Uint8 *>(source),sizeof(source));
    assert(calls==1 && gamepkg_audio_snapshot(pcm)==2);
    for(unsigned i=0;i<sizeof(expected);i++) assert(pcm[i]==expected[i]);
    gamepkg_audio_postmix(NULL, NULL);
    callback(0,reinterpret_cast<Uint8 *>(source),sizeof(source));
    assert(calls==1 && gamepkg_audio_snapshot(pcm)==2);
    for(unsigned i=0;i<sizeof(expected);i++) assert(pcm[i]==expected[i]);
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
    std::atomic<bool> stop(false);
    std::thread mixer([&]() {
        while(!stop.load()) callback(0,reinterpret_cast<Uint8 *>(source),sizeof(source));
    });
    for(unsigned i=0;i<1000;i++)
    {
        int active_calls=0;
        gamepkg_audio_postmix(recording, &active_calls);
        std::this_thread::yield();
        gamepkg_audio_postmix(NULL, NULL);
        int retired_calls=active_calls;
        callback(0,reinterpret_cast<Uint8 *>(source),sizeof(source));
        assert(active_calls==retired_calls); // Cleared callback cannot outlive its state.
    }
    stop.store(true); mixer.join();
    assert(gamepkg_audio_snapshot(pcm)>0);

}
