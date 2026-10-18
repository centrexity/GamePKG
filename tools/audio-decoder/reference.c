/* Maintainer oracle: SDL's decoded PCM at the source sample rate, without resampling. */
#include <SDL.h>
#include <SDL_mixer.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc,char**argv){
 if(argc!=3)return 1;int rate=atoi(argv[2]);if(rate!=44100&&rate!=48000)return 2;
 SDL_setenv("SDL_AUDIODRIVER","dummy",1);
 if(SDL_Init(SDL_INIT_AUDIO)||Mix_OpenAudio(rate,AUDIO_S16LSB,2,1024))return 3;
 int actual,channels;Uint16 format;if(!Mix_QuerySpec(&actual,&format,&channels)||actual!=rate||channels!=2||format!=AUDIO_S16LSB)return 4;
 Mix_Chunk*chunk=Mix_LoadWAV(argv[1]);if(!chunk||chunk->alen>64*1024*1024||chunk->alen%4)return 5;
 if(fwrite(chunk->abuf,1,chunk->alen,stdout)!=chunk->alen)return 6;
 Mix_FreeChunk(chunk);Mix_CloseAudio();SDL_Quit();return 0;
}
