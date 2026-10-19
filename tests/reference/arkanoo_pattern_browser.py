"""Original SDL pattern-browser overlay/panel/rows and font composition."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,tempfile,zlib
from sys import path
path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);args=parser.parse_args()
for name,digest in [('src/rendering.rs','51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'),('src/editor.rs','ad85d563ee863a873026919d44dc9704542746e6e549ffba6d44cc70b5cd8675'),('assets/font/static/Roboto-Medium.ttf','616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e')]:assert hashlib.sha256((args.original/name).read_bytes()).hexdigest()==digest
names=['café','j́','Ελληνικά','Привет','日本語 🎮','// “Δ','a'*200,'ä́','last','x']
c=r'''
#include <SDL.h>
#include <SDL_ttf.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
void label(SDL_Renderer*r,TTF_Font*f,const char*s,int x,int y,int center,int row_height,SDL_Color color){SDL_Surface*a=TTF_RenderUTF8_Blended(f,s,color);if(!a)return;SDL_Texture*t=SDL_CreateTextureFromSurface(r,a);SDL_Rect d={center?x-a->w/2:x,row_height<0?y:y+(row_height-a->h)/2,a->w,a->h};SDL_FreeSurface(a);SDL_RenderCopy(r,t,NULL,&d);SDL_DestroyTexture(t);}
int main(int argc,char**argv){if(argc!=4||SDL_Init(SDL_INIT_VIDEO)||TTF_Init())return 1;int count=atoi(argv[2]),selected=atoi(argv[3]);SDL_Window*w=SDL_CreateWindow("pattern-browser-reference",0,0,1280,720,SDL_WINDOW_HIDDEN);SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);SDL_RendererInfo info;if(!r||SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 2;SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");TTF_Font*f=TTF_OpenFont(argv[1],26);if(!f)return 3;SDL_SetRenderDrawColor(r,120,180,240,255);SDL_RenderClear(r);SDL_SetRenderDrawColor(r,230,50,90,255);SDL_Rect stripe={0,150,1280,200};SDL_RenderFillRect(r,&stripe);
SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,0,0,0,200);SDL_RenderFillRect(r,NULL);SDL_Rect panel={340,110,600,500};SDL_SetRenderDrawColor(r,40,45,52,255);SDL_RenderFillRect(r,&panel);SDL_SetRenderDrawColor(r,100,110,120,255);SDL_RenderDrawRect(r,&panel);label(r,f,"Load Pattern",640,125,1,-1,(SDL_Color){255,255,255,255});
const char*names[10]={NAMES};
for(int i=0;i<count;i++){int y=170+i*35;SDL_Rect row={360,y,560,30};SDL_SetRenderDrawColor(r,i==selected?70:55,i==selected?130:60,i==selected?180:68,255);SDL_RenderFillRect(r,&row);label(r,f,names[i],370,y,0,30,(SDL_Color){255,255,255,255});}
label(r,f,"Click pattern to load | ESC to cancel",432,570,0,-1,(SDL_Color){180,180,180,255});unsigned char*p=malloc(1280*720*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,p,1280*4)||fwrite(p,1,1280*720*4,stdout)!=1280*720*4)return 4;free(p);TTF_CloseFont(f);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);TTF_Quit();SDL_Quit();return 0;}
'''.replace('NAMES',','.join(json.dumps(n,ensure_ascii=False) for n in names))
with tempfile.TemporaryDirectory(prefix='gamepkg-pattern-browser-') as temporary:
 root=Path(temporary);(root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf'],text=True).split()],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=1280,height=720)
 try:
  cases=[]
  for count,selected in [(0,0),(1,0)]+[(10,i) for i in range(10)]:
   raw=subprocess.check_output([root/'reference',args.original/'assets/font/static/Roboto-Medium.ttf',str(count),str(selected)],env=env);assert len(raw)==1280*720*4
   cases.append(dict(count=count,selected=selected,rgba_sha256=hashlib.sha256(raw).hexdigest(),rgba_zlib_hex=zlib.compress(raw).hex()))
  print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',names=names,size=[1280,720],cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
