"""Original bonus primitives over the original campaign image, using real SDL."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,sys,tempfile
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);parser.add_argument('--sdl-crate',required=True);parser.add_argument('--sdl-sys-crate',required=True);args=parser.parse_args()
command=subprocess.run([sys.executable,str(Path(__file__).with_name('arkanoo_bonus.py')),str(args.original),'--sdl-crate',args.sdl_crate,'--sdl-sys-crate',args.sdl_sys_crate,'--commands-only'],capture_output=True,text=True,check=True);commands=json.loads(command.stdout)
old=json.loads((Path(__file__).resolve().parents[1]/'fixtures/arkanoo-bonus.json').read_text());assert commands['fragment_sha256']==old['fragment_sha256']
background=args.original/'assets/background1.png';assert hashlib.sha256(background.read_bytes()).hexdigest()=='21c34d3db19f1d2f8560bada107af03762a4003dfbc2c9a3c9b80b0bc9b3f8b2'
c=r'''
#include <SDL.h>
#include <SDL_image.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char**argv){if(argc!=3||SDL_Init(SDL_INIT_VIDEO))return 1;int shift=atoi(argv[2]);
SDL_Window*w=SDL_CreateWindow("bonus-scene-reference",0,0,1280,720,SDL_WINDOW_HIDDEN);if(!w)return 2;
SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);if(!r)return 3;SDL_RendererInfo info;if(SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 4;
SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");SDL_Texture*t=IMG_LoadTexture(r,argv[1]);if(!t)return 5;
SDL_SetRenderDrawColor(r,0,0,0,255);SDL_RenderClear(r);SDL_SetTextureBlendMode(t,SDL_BLENDMODE_BLEND);SDL_SetTextureAlphaMod(t,64);if(SDL_RenderCopy(r,t,NULL,NULL))return 6;
SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);char line[256];int kind=0;
while(fgets(line,sizeof(line),stdin)){if(line[0]=='K'){if(sscanf(line,"K %d",&kind)!=1)return 7;continue;}
char op;int layer,x,y,width,height,red,green,blue,alpha;if(sscanf(line," %c %d %d %d %d %d %d %d %d %d",&op,&layer,&x,&y,&width,&height,&red,&green,&blue,&alpha)!=10)return 8;
x+=400+kind*100+shift*2;y+=320;SDL_SetRenderDrawColor(r,red,green,blue,alpha);int error;
if(op=='P')error=SDL_RenderDrawPoint(r,x,y);else if(op=='R'){SDL_Rect rect={x,y,width,height};error=SDL_RenderFillRect(r,&rect);}else return 9;if(error)return 10;}
unsigned char*p=malloc(1280*720*4);if(!p||SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,p,1280*4))return 11;
for(int i=0;i<4;i++)for(int y=320;y<361;y++)if(fwrite(p+(y*1280+400+i*100+shift*2)*4,4,41,stdout)!=41)return 12;
free(p);SDL_DestroyTexture(t);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);SDL_Quit();return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-bonus-scene-reference-') as temporary:
 root=Path(temporary);(root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_image'],text=True).split()],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=1280,height=720)
 try:
  trace=''.join('K '+str(case['kind'])+'\n'+'\n'.join(case['commands'])+'\n' for case in commands['cases']).encode();scenes={}
  for shift in [0,20]:
   raw=subprocess.check_output([root/'reference',background,str(shift)],input=trace,env=env);assert len(raw)==4*41*41*4
   scenes[str(shift)]=[raw[kind*41*41*4:(kind+1)*41*41*4].hex() for kind in range(4)]
  print(json.dumps(dict(revision=commands['revision'],fragment_sha256=commands['fragment_sha256'],background_sha256=hashlib.sha256(background.read_bytes()).hexdigest(),trace_sha256=hashlib.sha256(trace).hexdigest(),scenes=scenes),indent=2))
 finally:display.terminate();display.wait(timeout=10)
