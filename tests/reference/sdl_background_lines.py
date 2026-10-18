"""Maintainer baseline: actual default OpenGL versus software SDL line drawing."""
from pathlib import Path
import hashlib,json,os,subprocess,sys,tempfile,zlib
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
code='''#include <SDL.h>
#include <stdio.h>
#include <string.h>
int main(int argc,char**argv){if(argc!=6)return 1;int gl=strcmp(argv[1],"opengl")==0;SDL_Surface*s=NULL;SDL_Window*w=NULL;SDL_Renderer*r=NULL;
if(gl){if(SDL_Init(SDL_INIT_VIDEO))return 2;w=SDL_CreateWindow("line reference",0,0,64,64,SDL_WINDOW_HIDDEN);if(w)r=SDL_CreateRenderer(w,-1,0);}else{s=SDL_CreateRGBSurfaceWithFormat(0,64,64,32,SDL_PIXELFORMAT_RGBA32);if(s)r=SDL_CreateSoftwareRenderer(s);}if(!r)return 3;
SDL_RendererInfo info;if(SDL_GetRendererInfo(r,&info)||strcmp(info.name,argv[1]))return 4;
SDL_SetRenderDrawColor(r,11,16,32,255);SDL_RenderClear(r);SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,200,100,255,150);
if(SDL_RenderDrawLine(r,atoi(argv[2]),atoi(argv[3]),atoi(argv[4]),atoi(argv[5])))return 5;
unsigned char pixels[64*64*4];if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,pixels,64*4))return 6;if(fwrite(pixels,1,sizeof(pixels),stdout)!=sizeof(pixels))return 7;
SDL_DestroyRenderer(r);if(w)SDL_DestroyWindow(w);if(s)SDL_FreeSurface(s);SDL_Quit();return 0;}
'''
segments=[[5,5,55,20],[55,20,5,5],[5,5,20,55],[20,55,5,5],[5,5,55,55],[55,55,5,5],[5,20,55,20],[20,5,20,55],[-20,5,55,60],[5,-20,60,55],[5,5,70,30],[5,5,30,70]]
segments += [[5,5,55,30],[55,30,5,5],[5,5,30,55],[30,55,5,5],[55,5,5,30],[5,30,55,5],[55,5,30,55],[30,55,55,5]]
segments += [[0,0,0,0],[63,63,63,63],[-1,-1,-1,-1],[-10,-10,-2,-2],[64,0,64,63],[0,64,63,64],[-20,32,80,32],[32,-20,32,80],[-10,0,0,-10],[0,0,63,0],[0,0,0,63],[63,63,64,64]]
with tempfile.TemporaryDirectory(prefix='gamepkg-line-reference-') as directory:
 root=Path(directory);(root/'reference.c').write_text(code)
 subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('SDL_RENDER_LINE_METHOD',None);env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=1280,height=720)
 try:
  cases=[]
  for segment in segments:
   images={mode:subprocess.check_output([root/'reference',mode,*map(str,segment)],env=env) for mode in ['opengl','software']}
   coverage={mode:{i for i in range(64*64) if data[i*4:i*4+3]!=bytes([11,16,32])} for mode,data in images.items()}
   differences=[i for i in range(64*64) if images['opengl'][i*4:i*4+3]!=images['software'][i*4:i*4+3]]
   cases.append(dict(segment=segment,coverage_pixel_differences=len(coverage['opengl']^coverage['software']),rgb_pixel_differences=len(differences),maximum_rgb_delta=max(abs(images['opengl'][i*4+c]-images['software'][i*4+c]) for i in range(64*64) for c in range(3)),images={mode:dict(rgba_sha256=hashlib.sha256(data).hexdigest(),rgba_zlib_hex=zlib.compress(data).hex()) for mode,data in images.items()}))
  assert any(case['rgb_pixel_differences']>0 for case in cases)
  print(json.dumps(dict(sdl_version=subprocess.check_output(['pkg-config','--modversion','sdl2'],text=True).strip(),width=64,height=64,foreground=[200,100,255,150],background=[11,16,32,255],cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
