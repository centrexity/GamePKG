/* Opaque filtering reference using SDL's default renderer and encoded texels. */
#include <SDL.h>
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
static unsigned char gray[]={0,0,0,255,255,255,255,255};
static unsigned char colors[]={255,0,0,255,0,255,0,255,0,0,255,255,255,255,255,255};
int main(int argc,char**argv){
 if(argc!=5||SDL_Init(SDL_INIT_VIDEO))return 1;int width=atoi(argv[1]),height=atoi(argv[2]),shift=atoi(argv[3]),swap=atoi(argv[4]);if(width<640||height<480)return 2;
 SDL_Window*w=SDL_CreateWindow("filter-reference",0,0,width,height,SDL_WINDOW_HIDDEN);if(!w)return 3;SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);if(!r)return 4;
 SDL_RendererInfo info;if(SDL_GetRendererInfo(r,&info))return 5;
 float scale=fminf(width/640.0f,height/480.0f),ox=(width-640*scale)/2,oy=(height-480*scale)/2;
 SDL_RenderSetViewport(r,NULL);SDL_SetRenderDrawColor(r,16,23,29,255);SDL_RenderClear(r);
 for(int i=0;i<2;i++){
  int kind=(i+swap)%2;SDL_Surface*s=SDL_CreateRGBSurfaceWithFormatFrom(kind?colors:gray,2,kind?2:1,32,8,SDL_PIXELFORMAT_RGBA32);if(!s)return 6;
  SDL_Texture*t=SDL_CreateTextureFromSurface(r,s);SDL_FreeSurface(s);if(!t)return 7;SDL_SetTextureBlendMode(t,SDL_BLENDMODE_BLEND);
  SDL_SetTextureScaleMode(t,SDL_ScaleModeNearest);SDL_FRect nearest={ox+(80+i*280)*scale,oy+40*scale,200*scale,20*scale};SDL_RenderCopyF(r,t,NULL,&nearest);SDL_RenderFlush(r);
  SDL_SetTextureScaleMode(t,SDL_ScaleModeLinear);SDL_FRect linear={ox+(80+i*280+shift)*scale,oy+80*scale,200*scale,320*scale};SDL_RenderCopyF(r,t,NULL,&linear);SDL_DestroyTexture(t);
 }
 unsigned char stripes[512*4];for(int i=0;i<512;i++){stripes[i*4]=stripes[i*4+1]=stripes[i*4+2]=(i%2)?255:0;stripes[i*4+3]=255;}
 for(int i=0;i<2;i++){
  SDL_Surface*s=SDL_CreateRGBSurfaceWithFormatFrom(i?stripes:gray,i?512:2,1,32,i?2048:8,SDL_PIXELFORMAT_RGBA32);if(!s)return 10;
  SDL_Texture*t=SDL_CreateTextureFromSurface(r,s);SDL_FreeSurface(s);if(!t)return 11;SDL_SetTextureBlendMode(t,SDL_BLENDMODE_BLEND);SDL_SetTextureScaleMode(t,SDL_ScaleModeLinear);
  SDL_FRect dst={ox+(i?240:-40)*scale,oy+420*scale,(i?128:200)*scale,40*scale};SDL_RenderCopyF(r,t,NULL,&dst);SDL_DestroyTexture(t);
 }
 unsigned char*pixels=malloc((size_t)width*height*4);if(!pixels)return 8;if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,pixels,width*4))return 9;
 printf("{\"driver\":\"%s\",\"samples\":[",info.name);int count=0;
 for(int image=0;image<2;image++)for(int row=0;row<3;row++)for(int col=0;col<5;col++){
  int x=(int)floorf(ox+(100+image*280+col*40+shift)*scale+.5f),y=(int)floorf(oy+(150+row*90)*scale+.5f);unsigned char*p=&pixels[((size_t)y*width+x)*4];printf("%s[%u,%u,%u]",count++?",":"",p[0],p[1],p[2]);
 }
 for(int image=0;image<2;image++)for(int col=0;col<2;col++){
  int x=(int)floorf(ox+(120+image*280+col*120)*scale+.5f),y=(int)floorf(oy+50*scale+.5f);unsigned char*p=&pixels[((size_t)y*width+x)*4];printf(",[%u,%u,%u]",p[0],p[1],p[2]);
 }
 for(int i=0;i<2;i++)for(int col=0;col<(i?7:5);col++){
  int x=(int)floorf(ox+(i?240+col*20:col*40-(col?20:0))*scale+.5f),y=(int)floorf(oy+440*scale+.5f);unsigned char*p=&pixels[((size_t)y*width+x)*4];printf(",[%u,%u,%u]",p[0],p[1],p[2]);
 }
 printf("]}\n");free(pixels);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);SDL_Quit();return 0;
}
