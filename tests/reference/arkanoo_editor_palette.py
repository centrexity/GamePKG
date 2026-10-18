"""Actual pinned SDL/Roboto palette with original colors, borders and labels."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,sys,tempfile,zlib
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);args=parser.parse_args()
for name,digest in [('src/rendering.rs','51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'),('src/entities.rs','ee6fc52f2ea582900edee90350c0c4608235aeecae6e56cc875cfd7893313545'),('assets/font/static/Roboto-Medium.ttf','616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'),('assets/background1.png','21c34d3db19f1d2f8560bada107af03762a4003dfbc2c9a3c9b80b0bc9b3f8b2')]:assert hashlib.sha256((args.original/name).read_bytes()).hexdigest()==digest
c=r'''
#include <SDL.h>
#include <SDL_image.h>
#include <SDL_ttf.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char**argv){if(argc!=5||SDL_Init(SDL_INIT_VIDEO)||TTF_Init())return 1;int selected=atoi(argv[3]),hovered=atoi(argv[4]);
SDL_Window*w=SDL_CreateWindow("palette-reference",0,0,1280,720,SDL_WINDOW_HIDDEN);if(!w)return 2;SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);if(!r)return 3;SDL_RendererInfo info;if(SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 4;
SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");SDL_Texture*bg=IMG_LoadTexture(r,argv[1]);TTF_Font*f=TTF_OpenFont(argv[2],26);if(!bg||!f)return 5;
SDL_SetRenderDrawColor(r,0,0,0,255);SDL_RenderClear(r);SDL_RenderCopy(r,bg,NULL,NULL);SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,0,0,0,153);SDL_RenderFillRect(r,NULL);
const unsigned char colors[9][3]={{255,0,0},{255,165,0},{255,255,0},{0,255,0},{0,0,255},{138,43,226},{200,240,255},{255,100,0},{30,35,40}};
for(int index=0;index<9;index++){
SDL_Rect rect={860+index*45,20,40,30};SDL_SetRenderDrawColor(r,colors[index][0],colors[index][1],colors[index][2],255);SDL_RenderFillRect(r,&rect);
int shade=index==selected?255:index==hovered?150:100;SDL_SetRenderDrawColor(r,shade,shade,shade,255);
for(int i=0;i<(index==selected?3:1);i++){SDL_Rect expanded={rect.x-i,rect.y-i,rect.w+2*i,rect.h+2*i};SDL_RenderDrawRect(r,&expanded);}
char label[2]={(char)('0'+index),0};SDL_Surface*s=TTF_RenderUTF8_Blended(f,label,(SDL_Color){0,0,0,255});if(!s)return 6;SDL_Texture*t=SDL_CreateTextureFromSurface(r,s);int width=s->w,height=s->h;SDL_FreeSurface(s);if(!t)return 7;SDL_SetTextureBlendMode(t,SDL_BLENDMODE_BLEND);SDL_Rect dst={rect.x+(rect.w-width)/2,rect.y,width,height};SDL_RenderCopy(r,t,NULL,&dst);SDL_DestroyTexture(t);
}
SDL_Rect region={858,18,404,34};unsigned char pixels[404*34*4];if(SDL_RenderReadPixels(r,&region,SDL_PIXELFORMAT_RGBA32,pixels,404*4))return 8;if(fwrite(pixels,1,sizeof(pixels),stdout)!=sizeof(pixels))return 9;
TTF_CloseFont(f);SDL_DestroyTexture(bg);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);TTF_Quit();SDL_Quit();return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-editor-palette-reference-') as temporary:
 root=Path(temporary);(root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_image','SDL2_ttf'],text=True).split()],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=1280,height=720)
 try:
  cases=[]
  for selected,hovered in [(s,h) for s in range(9) for h in [255,(s+1)%9,s]]+[(255,255)]:
   raw=subprocess.check_output([root/'reference',args.original/'assets/background1.png',args.original/'assets/font/static/Roboto-Medium.ttf',str(selected),str(hovered)],env=env);assert len(raw)==404*34*4
   cases.append(dict(selected=selected,hovered=hovered,rgba_sha256=hashlib.sha256(raw).hexdigest(),rgba_zlib_hex=zlib.compress(raw).hex()))
  print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',origin=[858,18],size=[404,34],font_size=26,cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
