/* Independent SDL oracle: continuous mixing, a finite track, then silence. */
#include <SDL.h>
#include <stdio.h>
static int reference(int music_volume,int effect_volume) {
    const int lengths[]={1025,2051,4096,777};
    Uint16 raw[8000*2]={0},mixed[8000*2]={0},effect[4096*2];
    for(int i=0;i<6000;i++)for(int c=0;c<2;c++)raw[i*2+c]=SDL_SwapLE16((Uint16)((i*7919+c*31337)%65536-32768));
    SDL_MixAudioFormat((Uint8*)mixed,(const Uint8*)raw,AUDIO_S16LSB,sizeof(raw),music_volume);
    for(int slot=0;slot<4;slot++){
        for(int i=0;i<lengths[slot];i++)for(int c=0;c<2;c++)effect[i*2+c]=SDL_SwapLE16((Uint16)((i*(slot*1543+197)+c*12345+slot*313)%65536-32768));
        SDL_MixAudioFormat((Uint8*)mixed,(const Uint8*)effect,AUDIO_S16LSB,lengths[slot]*4,effect_volume);
    }
    SDL_AudioCVT cvt;
    if(SDL_BuildAudioCVT(&cvt,AUDIO_S16LSB,2,44100,AUDIO_S16LSB,2,48000)<0)return 1;
    cvt.len=sizeof(mixed);cvt.buf=SDL_malloc((size_t)cvt.len*cvt.len_mult);if(!cvt.buf)return 2;
    SDL_memcpy(cvt.buf,mixed,sizeof(mixed));if(SDL_ConvertAudio(&cvt)||cvt.len_cvt<8192*4)return 3;
    if(fwrite(cvt.buf,1,8192*4,stdout)!=8192*4)return 4;
    SDL_free(cvt.buf);return 0;
}
int main(void){
    const int volumes[]={0,1,63,64,127,128};
    for(unsigned i=0;i<sizeof(volumes)/sizeof(volumes[0]);i++){int error=reference(128,volumes[i]);if(error)return error;}
    int error=reference(0,64);if(error)return error;return reference(64,64);
}
