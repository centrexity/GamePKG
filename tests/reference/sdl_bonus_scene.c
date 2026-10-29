/* Independent SDL sampling: original full image and original bonus primitives. */
#include <SDL.h>
#include <SDL_image.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {char op; int x,y,w,h,r,g,b,a;} Command;
static Command commands[4][3000];
static int counts[4];
static SDL_Window *window;
static SDL_Renderer *renderer;
static int initialize(int width,int height) {
    if(SDL_Init(SDL_INIT_VIDEO))return 1;
    window=SDL_CreateWindow("independent-original-art",0,0,width,height,SDL_WINDOW_HIDDEN);
    if(!window)return 2;
    renderer=SDL_CreateRenderer(window,-1,SDL_RENDERER_TARGETTEXTURE);
    SDL_RendererInfo info;
    if(!renderer||SDL_GetRendererInfo(renderer,&info)||strcmp(info.name,"opengl"))return 3;
    return 0;
}
static int background(int argc,char **argv) {
    if(argc!=9)return 4;
    int width=atoi(argv[3]),height=atoi(argv[4]);
    SDL_FRect destination={strtof(argv[5],NULL),strtof(argv[6],NULL),strtof(argv[7],NULL),strtof(argv[8],NULL)};
    if(width<1||height<1||width>4096||height>4096||!isfinite(destination.x)||!isfinite(destination.y)||!isfinite(destination.w)||!isfinite(destination.h)||destination.w<=0||destination.h<=0)return 5;
    int error=initialize(width,height);if(error)return error;
    SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");
    SDL_Texture *image=IMG_LoadTexture(renderer,argv[2]);if(!image)return 6;
    int iw,ih;if(SDL_QueryTexture(image,NULL,NULL,&iw,&ih)||iw!=2816||ih!=1536)return 7;
    SDL_SetRenderDrawColor(renderer,0,0,0,255);if(SDL_RenderClear(renderer))return 8;
    SDL_SetTextureBlendMode(image,SDL_BLENDMODE_BLEND);SDL_SetTextureAlphaMod(image,64);
    if(SDL_RenderCopyF(renderer,image,NULL,&destination))return 9;
    size_t length=(size_t)width*(size_t)height*4;unsigned char *pixels=malloc(length);if(!pixels)return 10;
    if(SDL_RenderReadPixels(renderer,NULL,SDL_PIXELFORMAT_RGBA32,pixels,width*4)||fwrite(pixels,1,length,stdout)!=length)return 11;
    free(pixels);SDL_DestroyTexture(image);return 0;
}
static int samples(int argc,char **argv) {
    if(argc!=3)return 12;
    FILE *trace=fopen(argv[2],"r");if(!trace)return 13;
    char line[256];int kind=-1;
    while(fgets(line,sizeof line,trace)) {
        if(line[0]=='K'){if(sscanf(line,"K %d",&kind)!=1||kind<0||kind>3)return 14;continue;}
        Command c;int layer;
        if(kind<0||counts[kind]>=3000||sscanf(line," %c %d %d %d %d %d %d %d %d %d",&c.op,&layer,&c.x,&c.y,&c.w,&c.h,&c.r,&c.g,&c.b,&c.a)!=10)return 15;
        if((c.op!='P'&&c.op!='R')||c.x<0||c.x>40||c.y<0||c.y>40||c.w<0||c.w>41||c.h<0||c.h>41||c.r<0||c.r>255||c.g<0||c.g>255||c.b<0||c.b>255||c.a<0||c.a>255)return 16;
        commands[kind][counts[kind]++]=c;
    }
    fclose(trace);
    for(int k=0;k<4;k++)if(counts[k]!=(k==1?2537:2297))return 17;
    int error=initialize(41,41);if(error)return error;
    SDL_Texture *target=SDL_CreateTexture(renderer,SDL_PIXELFORMAT_RGBA8888,SDL_TEXTUREACCESS_TARGET,41,41);
    if(!target||SDL_SetRenderTarget(renderer,target))return 18;
    int k,x,y,r,g,b,requests=0;
    while(scanf("%d %d %d %d %d %d",&k,&x,&y,&r,&g,&b)==6) {
        if(++requests>4096||k<0||k>3||x<0||x>40||y<0||y>40||r<0||r>255||g<0||g>255||b<0||b>255)return 19;
        SDL_SetRenderDrawColor(renderer,r,g,b,255);if(SDL_RenderClear(renderer))return 20;
        if(SDL_SetRenderDrawBlendMode(renderer,SDL_BLENDMODE_BLEND))return 21;
        for(int i=0;i<counts[k];i++) {
            Command *c=&commands[k][i];SDL_SetRenderDrawColor(renderer,c->r,c->g,c->b,c->a);
            if(c->op=='P'){if(SDL_RenderDrawPoint(renderer,c->x,c->y))return 22;}
            else {SDL_Rect rectangle={c->x,c->y,c->w,c->h};if(SDL_RenderFillRect(renderer,&rectangle))return 23;}
        }
        SDL_Rect pixel={x,y,1,1};unsigned char rgba[4];
        if(SDL_RenderReadPixels(renderer,&pixel,SDL_PIXELFORMAT_RGBA32,rgba,4)||fwrite(rgba,1,4,stdout)!=4)return 24;
    }
    SDL_DestroyTexture(target);return 0;
}
int main(int argc,char **argv) {
    int result=argc>1&&!strcmp(argv[1],"background")?background(argc,argv):samples(argc,argv);
    if(renderer)SDL_DestroyRenderer(renderer);
    if(window)SDL_DestroyWindow(window);
    SDL_Quit();
    if(result)fprintf(stderr,"independent SDL scene reference failed: %d (%s)\n",result,SDL_GetError());
    return result;
}
