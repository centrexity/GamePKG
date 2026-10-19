/* Maintainer oracle: actual SDL_mixer music repeat count and callback continuity. */
#include <SDL.h>
#include <SDL_mixer.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static SDL_atomic_t finished;
static int captured, first, last, last_nonzero, bad;
static int16_t samples[256][2];
static void ended(void) { SDL_AtomicSet(&finished, 1); }
static void mixed(void *unused, Uint8 *bytes, int length) {
    (void)unused;
    for (int i=0; i+3<length; i+=4) {
        int16_t left=(int16_t)((unsigned)bytes[i]|((unsigned)bytes[i+1]<<8));
        int16_t right=(int16_t)((unsigned)bytes[i+2]|((unsigned)bytes[i+3]<<8));
        if (left || right) {
            if (first<0) first=last;
            last_nonzero=last;
            if (captured>=256) { bad=1; return; }
            samples[captured][0]=left; samples[captured][1]=right; captured++;
        }
        last++;
    }
}
static void word(Uint8 *out, unsigned value) { out[0]=(Uint8)value;out[1]=(Uint8)(value>>8); }
static void dword(Uint8 *out, unsigned value) {word(out,value);word(out+2,value>>16);}
int main(void) {
    Uint8 wav[44+16*4]={0};
    memcpy(wav,"RIFF",4);dword(wav+4,sizeof(wav)-8);memcpy(wav+8,"WAVEfmt ",8);
    dword(wav+16,16);word(wav+20,1);word(wav+22,2);dword(wav+24,44100);
    dword(wav+28,176400);word(wav+32,4);word(wav+34,16);memcpy(wav+36,"data",4);dword(wav+40,64);
    for(int i=0;i<16;i++){word(wav+44+i*4,1000+i*101);word(wav+46+i*4,(unsigned)(-2000-i*113));}
    if(SDL_setenv("SDL_AUDIODRIVER","dummy",1)||SDL_Init(SDL_INIT_AUDIO))return 1;
    for(int loops=0;loops<=2;loops++) {
        if(Mix_OpenAudio(44100,AUDIO_S16LSB,2,64))return 2;
        captured=last=bad=0;first=last_nonzero=-1;SDL_AtomicSet(&finished,0);
        Mix_SetPostMix(mixed,NULL);Mix_HookMusicFinished(ended);
        Mix_Music *music=Mix_LoadMUS_RW(SDL_RWFromConstMem(wav,sizeof(wav)),1);
        if(!music||Mix_PlayMusic(music,loops))return 3;
        Uint32 start=SDL_GetTicks();
        while(!SDL_AtomicGet(&finished)){if(SDL_GetTicks()-start>2000)return 4;SDL_Delay(1);}
        Mix_CloseAudio();Mix_FreeMusic(music);
        int passes=loops>0?loops:1;
        if(bad||captured!=16*passes||first<0||last_nonzero-first+1!=captured)return 5;
        for(int i=0;i<captured;i++)if(samples[i][0]!=1000+(i%16)*101||samples[i][1]!=-2000-(i%16)*113)return 6;
        printf("SDL music loops=%d: %d exact frames (%d complete passes)\n",loops,captured,passes);
    }
    SDL_Quit();return 0;
}
