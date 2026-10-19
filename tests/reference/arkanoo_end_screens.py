"""Original end-state overlays on a fixed contrast backdrop, independent SDL oracle."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,tempfile,zlib,sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);args=parser.parse_args()
for name,digest in [('src/rendering.rs','51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'),('assets/font/static/Roboto-Medium.ttf','616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e')]:assert hashlib.sha256((args.original/name).read_bytes()).hexdigest()==digest
c=r'''
#include <SDL.h>
#include <SDL_ttf.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
void label(SDL_Renderer*r,TTF_Font*f,const char*text,int y,SDL_Color color){SDL_Surface*s=TTF_RenderUTF8_Blended(f,text,color);if(!s)exit(5);SDL_Texture*t=SDL_CreateTextureFromSurface(r,s);SDL_Rect box={640-s->w/2,y,s->w,s->h};SDL_FreeSurface(s);if(!t||SDL_RenderCopy(r,t,NULL,&box))exit(6);SDL_DestroyTexture(t);}
int main(int argc,char**argv){if(argc!=6||SDL_Init(SDL_INIT_VIDEO)||TTF_Init())return 1;int mode=atoi(argv[2]),empty=atoi(argv[5]);uint32_t level=(uint32_t)strtoul(argv[3],NULL,10),score=(uint32_t)strtoul(argv[4],NULL,10);SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");SDL_Window*w=SDL_CreateWindow("end-screen-reference",0,0,1280,720,SDL_WINDOW_HIDDEN);SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);SDL_RendererInfo info;if(!r||SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 2;TTF_Font*f=TTF_OpenFont(argv[1],26);if(!f)return 3;SDL_SetRenderDrawColor(r,120,180,240,255);SDL_RenderClear(r);SDL_Rect stripe={0,150,1280,200};SDL_SetRenderDrawColor(r,230,50,90,255);SDL_RenderFillRect(r,&stripe);SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,0,0,0,mode==3?150:180);SDL_RenderFillRect(r,NULL);SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_NONE);char title[80],points[80];snprintf(points,sizeof(points),mode==3?"Score: %u":"Final Score: %u",score);
if(mode==3){snprintf(title,sizeof(title),"Level %u Complete!",level);label(r,f,title,260,(SDL_Color){0,255,100,255});label(r,f,points,320,(SDL_Color){255,255,255,255});label(r,f,"Click to start next level",400,(SDL_Color){255,255,100,255});}
else if(mode==4){label(r,f,empty?"VICTORY!":"GAME OVER",260,empty?(SDL_Color){0,255,0,255}:(SDL_Color){255,100,100,255});label(r,f,points,320,(SDL_Color){255,255,255,255});label(r,f,"Press R to Restart or Q to Quit",380,(SDL_Color){200,200,200,255});}
else if(mode==5){label(r,f,"CONGRATULATIONS!",240,(SDL_Color){255,215,0,255});label(r,f,"Campaign Complete!",290,(SDL_Color){0,255,0,255});label(r,f,points,340,(SDL_Color){255,255,255,255});label(r,f,"Press ENTER for Infinite Mode",400,(SDL_Color){255,215,0,255});label(r,f,"Press R to Restart or Q to Quit",440,(SDL_Color){200,200,200,255});}else return 7;
unsigned char*p=malloc(1280*720*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,p,1280*4)||fwrite(p,1,1280*720*4,stdout)!=1280*720*4)return 4;free(p);TTF_CloseFont(f);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);TTF_Quit();SDL_Quit();return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-end-screen-reference-') as temporary:
 root=Path(temporary);(root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf'],text=True).split()],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=1280,height=720)
 try:
  states=[(3,level,score,0) for level in [1,9,10,4294967295] for score in [0,1,1234567890,4294967295]]+[(4,1,score,empty) for empty in [0,1] for score in [0,1,1234567890,4294967295]]+[(5,9,score,0) for score in [0,1,1234567890,4294967295]]
  cases=[]
  for mode,level,score,empty in states:
   pixels=subprocess.check_output([root/'reference',args.original/'assets/font/static/Roboto-Medium.ttf',str(mode),str(level),str(score),str(empty)],env=env);assert len(pixels)==1280*720*4;cases.append(dict(mode=mode,level=level,score=score,empty=empty,rgba_sha256=hashlib.sha256(pixels).hexdigest(),rgba_zlib_hex=zlib.compress(pixels).hex()))
  print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',size=[1280,720],scope='end-state overlays over a fixed contrast backdrop; HUD and gameplay scene are separate',cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
