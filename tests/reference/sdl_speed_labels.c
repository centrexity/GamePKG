/* Original font and speed-label destination rules, projected into host coordinates. */
#include <SDL.h>
#include <SDL_ttf.h>
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>
int main(int argc,char**argv){
 if(argc!=5||SDL_Init(SDL_INIT_VIDEO)||TTF_Init())return 1;
 int width=atoi(argv[2]),height=atoi(argv[3]),phase=atoi(argv[4]);
 SDL_Window*w=SDL_CreateWindow("speed-reference",0,0,width,height,SDL_WINDOW_HIDDEN);if(!w)return 2;
 SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);if(!r)return 3;
 SDL_RendererInfo info;if(SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 11;
 TTF_Font*f=TTF_OpenFont(argv[1],26);if(!f)return 4;
 float scale=fminf(width/640.0f,height/480.0f),ox=(width-640*scale)/2,oy=(height-480*scale)/2;
 SDL_SetRenderDrawColor(r,11,16,32,255);SDL_RenderClear(r);
 int speeds[4]={0,60,300,600};if(phase==2){speeds[0]=1200;speeds[1]=1800;}
 for(int i=0;i<4;i++){
  char label[32];snprintf(label,sizeof(label),"%d px/s",speeds[i]);
  SDL_Surface*s=TTF_RenderUTF8_Blended(f,label,(SDL_Color){200,200,200,255});if(!s)return 5;
  unsigned int sw=(unsigned int)(s->w*0.6f),sh=(unsigned int)(s->h*0.6f);
  SDL_Texture*t=SDL_CreateTextureFromSurface(r,s);SDL_FreeSurface(s);if(!t)return 6;
  SDL_SetTextureBlendMode(t,SDL_BLENDMODE_BLEND);SDL_SetTextureScaleMode(t,SDL_ScaleModeLinear);
  int cx=(120+i*140+(phase==1?20:0))*2,cy=320;
  SDL_FRect dst={ox+(cx-(int)sw/2)*0.5f*scale,oy+(60+(cy-6-(int)sh-5)*0.5f)*scale,sw*0.5f*scale,sh*0.5f*scale};
  if(SDL_RenderCopyF(r,t,NULL,&dst))return 7;SDL_DestroyTexture(t);
 }
 unsigned char*p=malloc((size_t)width*height*4);if(!p)return 8;
 if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,p,width*4))return 9;
 if(fwrite(p,4,(size_t)width*height,stdout)!=(size_t)width*height)return 10;
 free(p);TTF_CloseFont(f);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);TTF_Quit();SDL_Quit();return 0;
}
