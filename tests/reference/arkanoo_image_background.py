"""Pinned original image backgrounds through actual SDL_image/OpenGL; maintainer only."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,sys,tempfile,zlib,struct
from PIL import Image
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);parser.add_argument('--write-assets',type=Path);args=parser.parse_args()
HASHES=['21c34d3db19f1d2f8560bada107af03762a4003dfbc2c9a3c9b80b0bc9b3f8b2','f5b14219e8b265d6bb2e669b1050768cd2a57688a8691a142569e458a4ba35e9','5634e4eb36fc4ef61120064f003f0fe69fdaf23302e0bfaf64ad1fed84aeb56d','053b1263b2647d128a73d6f0236781a873790fb473af95348fbba1b1b1793eeb','cf7808cee9e154e3ab8b064867557717806476ecbeefa2fd4c16eab0860cee68','a45a1912f5e7d3b6251a0794612a1cd6cd42507b1d38a27bddd24250daf9dcf3']
main=args.original/'src/main.rs';assert hashlib.sha256(main.read_bytes()).hexdigest()=='7522df06e4d04bfcc00a2261022e21b18300acc339e394bc22ff83bb95824c73'
assert 'sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "linear");' in main.read_text()
source=args.original/'src/rendering.rs';assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
c=r'''
#include <SDL.h>
#include <SDL_image.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char**argv){if((argc!=3&&argc!=4&&argc!=5)||SDL_Init(SDL_INIT_VIDEO))return 1;
SDL_Window*w=SDL_CreateWindow("background-reference",0,0,1280,720,SDL_WINDOW_HIDDEN);if(!w)return 2;
SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);if(!r)return 3;SDL_RendererInfo info;if(SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 4;
SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");SDL_Texture*t=IMG_LoadTexture(r,argv[1]);if(!t)return 5;
SDL_SetRenderDrawColor(r,0,0,0,255);SDL_RenderClear(r);SDL_SetTextureBlendMode(t,SDL_BLENDMODE_BLEND);SDL_SetTextureAlphaMod(t,atoi(argv[2]));
if(SDL_RenderCopy(r,t,NULL,NULL))return 6;if(argc>=4){SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,0,0,0,atoi(argv[3]));if(SDL_RenderFillRect(r,NULL))return 10;}if(argc==5){SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_NONE);SDL_SetRenderDrawColor(r,80,80,100,200);for(int row=0;row<=10;row++)if(SDL_RenderDrawLine(r,40,80+row*20,1240,80+row*20))return 11;for(int col=0;col<=20;col++)if(SDL_RenderDrawLine(r,40+col*60,80,40+col*60,280))return 12;}unsigned char*p=malloc(1280*720*4);if(!p)return 7;
if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,p,1280*4))return 8;
if(fwrite(p,1,1280*720*4,stdout)!=1280*720*4)return 9;
free(p);SDL_DestroyTexture(t);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);SDL_Quit();return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-image-background-reference-') as temporary:
 root=Path(temporary);(root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_image'],text=True).split()],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);env.pop('SDL_RENDER_SCALE_QUALITY',None);display=start_display(env,width=1280,height=720)
 try:
  cases=[]
  for level in range(1,7):
   path=args.original/f'assets/background{level}.png';assert hashlib.sha256(path.read_bytes()).hexdigest()==HASHES[level-1]
   bright=subprocess.check_output([root/'reference',path,'255'],env=env);game=subprocess.check_output([root/'reference',path,'64'],env=env)
   editor=subprocess.check_output([root/'reference',path,'255','153'],env=env)
   grid=subprocess.check_output([root/'reference',path,'255','153','grid'],env=env)
   assert len(bright)==len(game)==len(editor)==len(grid)==1280*720*4
   projected=bytearray(bright)
   for at in range(0,len(projected),4):
    for channel in range(3):projected[at+channel]=(projected[at+channel]*102+127)//255
   assert bytes(projected)==editor
   image=Image.frombytes('RGBA',(1280,720),bright);tiles=[]
   for top in [0,512]:
    for left in [0,512,1024]:
     box=(left,top,min(left+512,1280),min(top+512,720));tile=image.crop(box);name=f'background{level}-{len(tiles)}.png';pixels=tile.tobytes()
     tiles.append(dict(path=name,origin=[left,top],size=list(tile.size),rgba_sha256=hashlib.sha256(pixels).hexdigest()))
     if args.write_assets:
      args.write_assets.mkdir(parents=True,exist_ok=True);assert all(alpha==255 for alpha in pixels[3::4])
      # Opaque RGB with unfiltered stored scanlines avoids repeated Deflate and
      # PNG-predictor work when these large images exceed the bounded LRU.
      rgb=tile.convert('RGB').tobytes();w,h=tile.size
      scanlines=b''.join(b'\0'+rgb[row*w*3:(row+1)*w*3] for row in range(h))
      def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
      (args.write_assets/name).write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(scanlines,0))+chunk(b'IEND',b''))
   cases.append(dict(level=level,original_sha256=HASHES[level-1],bright_rgba_sha256=hashlib.sha256(bright).hexdigest(),editor_grid_rgba_sha256=hashlib.sha256(grid).hexdigest(),editor_grid_alpha=sorted(set(grid[3::4])),editor_rgba_sha256=hashlib.sha256(editor).hexdigest(),game_rgba_sha256=hashlib.sha256(game).hexdigest(),game_rgba_zlib_hex=zlib.compress(game).hex(),tiles=tiles))
  print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),main_sha256=hashlib.sha256(main.read_bytes()).hexdigest(),scale_quality='linear',width=1280,height=720,original_size=[2816,1536],alpha_mod=64,editor_overlay=153,cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
