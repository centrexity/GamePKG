/* Maintainer oracle: SDL native-rate decoding, optionally followed by one
 * whole-buffer SDL conversion. This is not SDL_AudioStream chunk conversion. */
#include <SDL.h>
#include <SDL_mixer.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc,char**argv){
 if(argc!=3&&argc!=4)return 1;int rate=atoi(argv[2]);if(rate!=44100&&rate!=48000)return 2;
 SDL_setenv("SDL_AUDIODRIVER","dummy",1);
 if(SDL_Init(SDL_INIT_AUDIO)||Mix_OpenAudio(rate,AUDIO_S16LSB,2,1024))return 3;
 int actual,channels;Uint16 format;if(!Mix_QuerySpec(&actual,&format,&channels)||actual!=rate||channels!=2||format!=AUDIO_S16LSB)return 4;
 Mix_Chunk*chunk=Mix_LoadWAV(argv[1]);if(!chunk||chunk->alen>64*1024*1024||chunk->alen%4)return 5;
 if(argc==4){
  int target=atoi(argv[3]);if(target!=44100&&target!=48000)return 7;
  SDL_AudioCVT cvt;if(SDL_BuildAudioCVT(&cvt,AUDIO_S16LSB,2,rate,AUDIO_S16LSB,2,target)<0||cvt.len_mult<=0||cvt.len_mult>16)return 8;
  cvt.len=(int)chunk->alen;cvt.buf=SDL_malloc((size_t)cvt.len*cvt.len_mult);if(!cvt.buf)return 9;
  SDL_memcpy(cvt.buf,chunk->abuf,chunk->alen);if(SDL_ConvertAudio(&cvt)||cvt.len_cvt<0)return 10;
  if(fwrite(cvt.buf,1,(size_t)cvt.len_cvt,stdout)!=(size_t)cvt.len_cvt)return 11;
  SDL_free(cvt.buf);
 }else if(fwrite(chunk->abuf,1,chunk->alen,stdout)!=chunk->alen)return 6;
 Mix_FreeChunk(chunk);Mix_CloseAudio();SDL_Quit();return 0;
}
