/* SDL calls used by the original renderer; sample away from rasterized edges. */
#include <SDL.h>
#include <stdio.h>
#include <stdlib.h>
static const unsigned char backgrounds[4][4]={{0,0,0,255},{11,16,32,255},{80,80,100,255},{255,255,255,255}};
static const unsigned char colors[6][4]={{255,255,255,30},{0,200,255,200},{255,0,0,128},{40,150,220,80},{130,70,200,0},{14,92,167,255}};
int main(int argc,char**argv){
 if(SDL_Init(SDL_INIT_VIDEO))return 1;
 SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");
 SDL_Window*w=SDL_CreateWindow("reference",0,0,40,10,SDL_WINDOW_HIDDEN);if(!w)return 2;
 SDL_Renderer*r=SDL_CreateRenderer(w,-1,argc>1&&atoi(argv[1])?SDL_RENDERER_SOFTWARE:0);if(!r)return 3;
 SDL_RendererInfo info;if(SDL_GetRendererInfo(r,&info))return 4;
 SDL_Texture*textures[6];for(int i=0;i<6;i++){
  SDL_Surface*s=SDL_CreateRGBSurfaceWithFormat(0,1,1,32,SDL_PIXELFORMAT_RGBA32);if(!s)return 5;
  const unsigned char*c=colors[i];SDL_FillRect(s,NULL,SDL_MapRGBA(s->format,c[0],c[1],c[2],c[3]));
  textures[i]=SDL_CreateTextureFromSurface(r,s);SDL_FreeSurface(s);if(!textures[i])return 6;SDL_SetTextureBlendMode(textures[i],SDL_BLENDMODE_BLEND);
 }
 printf("{\"driver\":\"%s\",\"samples\":[",info.name);
 for(int bg=0;bg<4;bg++)for(int fg=0;fg<6;fg++){
  const unsigned char*b=backgrounds[bg],*c=colors[fg];SDL_SetRenderDrawColor(r,b[0],b[1],b[2],255);SDL_RenderClear(r);
  SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,c[0],c[1],c[2],c[3]);
  SDL_Rect left={0,0,10,10},middle={10,0,10,10},right={20,0,10,10};SDL_RenderFillRect(r,&left);SDL_RenderFillRect(r,&right);
  SDL_RenderCopy(r,textures[fg],NULL,&middle);SDL_RenderCopy(r,textures[(fg+1)%6],NULL,&right);
  unsigned char pixels[40*10*4];if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,pixels,40*4))return 7;
  printf("%s[",bg||fg?",":"");for(int sample=0;sample<3;sample++){unsigned char*p=&pixels[(5*40+5+sample*10)*4];printf("%s[%u,%u,%u]",sample?",":"",p[0],p[1],p[2]);}printf("]");
 }
 printf("]}\n");for(int i=0;i<6;i++)SDL_DestroyTexture(textures[i]);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);SDL_Quit();return 0;
}
