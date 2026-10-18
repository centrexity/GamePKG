"""Observe SDL/OpenGL filtered texture coverage at scaled integer rectangle edges."""
from pathlib import Path
import json,os,subprocess,tempfile,sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
c=r'''
#include <SDL.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char**argv){if(argc!=6||SDL_Init(SDL_INIT_VIDEO))return 1;SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");SDL_Window*w=SDL_CreateWindow("filtered-edges",0,0,64,64,SDL_WINDOW_HIDDEN);SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);SDL_RendererInfo info;if(!r||SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 2;SDL_Texture*t=SDL_CreateTexture(r,SDL_PIXELFORMAT_RGBA32,SDL_TEXTUREACCESS_STATIC,1,1);unsigned char pixel[4]={255,0,0,255};SDL_UpdateTexture(t,NULL,pixel,4);SDL_SetRenderDrawColor(r,0,0,0,255);SDL_RenderClear(r);float scale=strtof(argv[1],NULL);SDL_RenderSetScale(r,scale,scale);SDL_Rect box={atoi(argv[2]),atoi(argv[3]),atoi(argv[4]),atoi(argv[5])};SDL_RenderCopy(r,t,NULL,&box);unsigned char pixels[64*64*4];if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,pixels,64*4)||fwrite(pixels,1,sizeof(pixels),stdout)!=sizeof(pixels))return 3;SDL_DestroyTexture(t);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);SDL_Quit();return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-sdl-filtered-edges-') as temp:
 root=Path(temp);(root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf'],text=True).split()],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=64,height=64)
 try:
  cases=[]
  for scale,rect in [(.5,[21,41,4,4]),(.5,[-1,-1,4,4]),(.5,[1,1,1,1]),(.5,[5,5,3,7]),(.25,[5,5,7,7]),(1.25,[13,11,3,5]),(1,[3,5,4,4]),(.5,[21,41,0,4]),(.5,[21,41,4,0])]:
   raw=subprocess.check_output([root/'reference',str(scale),*map(str,rect)],env=env);assert len(raw)==64*64*4;pixels=[[x,y] for y in range(64) for x in range(64) if raw[(y*64+x)*4]];cases.append(dict(scale=scale,rect=rect,pixels=pixels))
  print(json.dumps(dict(renderer='SDL OpenGL, linear texture sampling',size=[64,64],cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
