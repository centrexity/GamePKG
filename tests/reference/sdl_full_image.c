/* Independent original SDL full-texture scaling; no tiled intermediary. */
#include <SDL.h>
#include <SDL_image.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char **argv) {
  if(argc!=4||SDL_Init(SDL_INIT_VIDEO))return 1;
  int width=atoi(argv[2]),height=atoi(argv[3]);
  if(width<1||width>4096||height<1||height>4096)return 2;
  SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");
  SDL_Window *window=SDL_CreateWindow("private-original-texture-reference",0,0,width,height,SDL_WINDOW_HIDDEN);
  SDL_Renderer *renderer=SDL_CreateRenderer(window,-1,0);SDL_RendererInfo info;
  if(!renderer||SDL_GetRendererInfo(renderer,&info)||strcmp(info.name,"opengl"))return 3;
  SDL_Texture *image=IMG_LoadTexture(renderer,argv[1]);if(!image)return 4;
  if(SDL_RenderCopy(renderer,image,NULL,NULL))return 5;
  size_t length=(size_t)width*height*4;unsigned char *pixels=malloc(length);if(!pixels)return 6;
  if(SDL_RenderReadPixels(renderer,NULL,SDL_PIXELFORMAT_RGBA32,pixels,width*4)||fwrite(pixels,1,length,stdout)!=length)return 7;
  free(pixels);SDL_DestroyTexture(image);SDL_DestroyRenderer(renderer);SDL_DestroyWindow(window);SDL_Quit();return 0;
}
