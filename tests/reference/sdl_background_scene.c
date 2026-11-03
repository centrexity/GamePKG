/* Original full photograph sampling. No GamePKG renderer or derived texture. */
#include <SDL.h>
#include <SDL_image.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc != 12) return 1;
    int width=atoi(argv[2]),height=atoi(argv[3]),alpha=atoi(argv[8]),overlay=atoi(argv[9]),clear=atoi(argv[10]);
    SDL_FRect destination={strtof(argv[4],NULL),strtof(argv[5],NULL),strtof(argv[6],NULL),strtof(argv[7],NULL)};
    if(width<1||height<1||width>4096||height>4096||alpha<0||alpha>255||overlay<0||overlay>255||(clear!=0&&clear!=1)||!isfinite(destination.x)||!isfinite(destination.y)||!isfinite(destination.w)||!isfinite(destination.h)||destination.w<=0||destination.h<=0) return 2;
    if(SDL_Init(SDL_INIT_VIDEO))return 3;
    SDL_Window *window=SDL_CreateWindow("original-background-reference",0,0,width,height,SDL_WINDOW_HIDDEN);
    if(!window)return 4;
    SDL_Renderer *renderer=SDL_CreateRenderer(window,-1,0);
    SDL_RendererInfo info;
    if(!renderer||SDL_GetRendererInfo(renderer,&info)||strcmp(info.name,"opengl"))return 5;
    SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");
    SDL_Texture *image=IMG_LoadTexture(renderer,argv[1]);
    int iw,ih;
    if(!image||SDL_QueryTexture(image,NULL,NULL,&iw,&ih)||iw!=2816||ih!=1536)return 6;
    SDL_SetRenderDrawColor(renderer,clear?11:0,clear?16:0,clear?32:0,255);
    if(SDL_RenderClear(renderer))return 7;
    /* Original image modulation is over black, even when the player has a
     * differently colored letterbox around its logical game rectangle. */
    if(SDL_SetRenderDrawColor(renderer,0,0,0,255)||SDL_RenderFillRectF(renderer,&destination)||SDL_SetTextureBlendMode(image,SDL_BLENDMODE_BLEND)||SDL_SetTextureAlphaMod(image,alpha)||SDL_RenderCopyF(renderer,image,NULL,&destination))return 7;
    if(overlay){
        if(SDL_SetRenderDrawBlendMode(renderer,SDL_BLENDMODE_BLEND)||SDL_SetRenderDrawColor(renderer,0,0,0,overlay)||SDL_RenderFillRectF(renderer,&destination))return 8;
    }
    if(atoi(argv[11])){
        float sx=destination.w/1280.0f,sy=destination.h/720.0f;
        float ox=destination.x/sx,oy=destination.y/sy;
        if(SDL_RenderSetScale(renderer,sx,sy)||SDL_SetRenderDrawBlendMode(renderer,SDL_BLENDMODE_NONE)||SDL_SetRenderDrawColor(renderer,80,80,100,200))return 11;
        for(int row=0;row<=10;row++)if(SDL_RenderDrawLineF(renderer,ox+40,oy+80+row*20,ox+1240,oy+80+row*20))return 12;
        for(int col=0;col<=20;col++)if(SDL_RenderDrawLineF(renderer,ox+40+col*60,oy+80,ox+40+col*60,oy+280))return 13;
    }
    size_t length=(size_t)width*(size_t)height*4;
    unsigned char *pixels=malloc(length);
    if(!pixels)return 9;
    if(SDL_RenderReadPixels(renderer,NULL,SDL_PIXELFORMAT_RGBA32,pixels,width*4)||fwrite(pixels,1,length,stdout)!=length)return 10;
    free(pixels);SDL_DestroyTexture(image);SDL_DestroyRenderer(renderer);SDL_DestroyWindow(window);SDL_Quit();return 0;
}
