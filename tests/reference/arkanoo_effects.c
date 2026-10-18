#include <SDL.h>
#include <SDL_mixer.h>
#include <stdio.h>
#include <string.h>
int main(int argc,char**argv){
 if(argc!=2)return 1;
 SDL_setenv("SDL_AUDIODRIVER","dummy",1);
 if(SDL_Init(SDL_INIT_AUDIO)||Mix_OpenAudio(44100,AUDIO_S16LSB,2,1024))return 2;
 int rate,channels;Uint16 format;if(!Mix_QuerySpec(&rate,&format,&channels)||rate!=44100||format!=AUDIO_S16LSB||channels!=2)return 3;
 Mix_Chunk*chunk=Mix_LoadWAV(argv[1]);if(!chunk||!chunk->alen||chunk->alen>2*1024*1024||chunk->alen%4)return 4;
 SDL_AudioCVT convert;if(SDL_BuildAudioCVT(&convert,AUDIO_S16LSB,2,44100,AUDIO_S16LSB,2,48000)<0)return 5;
 convert.len=(int)chunk->alen;convert.buf=SDL_malloc((size_t)convert.len*convert.len_mult);if(!convert.buf)return 6;
 memcpy(convert.buf,chunk->abuf,chunk->alen);if(SDL_ConvertAudio(&convert)||convert.len_cvt%4)return 7;
 if(fwrite(convert.buf,1,convert.len_cvt,stdout)!=(size_t)convert.len_cvt)return 8;
 SDL_free(convert.buf);Mix_FreeChunk(chunk);Mix_CloseAudio();SDL_Quit();return 0;
}
