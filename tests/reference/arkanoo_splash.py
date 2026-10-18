"""Capture the pinned original splash texture, alpha arithmetic and SDL composition."""
from pathlib import Path
import argparse,hashlib,json,os,struct,subprocess,tempfile,zlib,sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);parser.add_argument('--write-assets',type=Path);args=parser.parse_args()
for name,digest in [('src/main.rs','7522df06e4d04bfcc00a2261022e21b18300acc339e394bc22ff83bb95824c73'),('src/rendering.rs','51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'),('assets/antigravity.webp','663904220eeb0bec0cc4b729b617808f0d40fc7de2501672435dccfda84d347e')]:assert hashlib.sha256((args.original/name).read_bytes()).hexdigest()==digest
c=r'''
#include <SDL.h>
#include <SDL_image.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
unsigned char alpha(int timer){return timer<60?(unsigned char)((float)timer/60.0f*255.0f):timer>210?(unsigned char)((float)(270-timer)/60.0f*255.0f):255;}
int main(int argc,char**argv){if(argc!=3)return 1;int timer=atoi(argv[2]);if(timer<0){for(int i=0;i<=270;i++){unsigned char a=alpha(i);if(fwrite(&a,1,1,stdout)!=1)return 2;}return 0;}if(SDL_Init(SDL_INIT_VIDEO))return 3;SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");SDL_Window*w=SDL_CreateWindow("splash-reference",0,0,1280,720,SDL_WINDOW_HIDDEN);SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);SDL_RendererInfo info;if(!r||SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 4;SDL_Texture*t=IMG_LoadTexture(r,argv[1]);if(!t)return 5;SDL_SetRenderDrawColor(r,255,255,255,255);SDL_RenderClear(r);SDL_SetTextureBlendMode(t,SDL_BLENDMODE_BLEND);SDL_SetTextureAlphaMod(t,alpha(timer));SDL_Rect target={0,0,1280,720};if(SDL_RenderCopy(r,t,NULL,&target))return 6;unsigned char*p=malloc(1280*720*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,p,1280*4)||fwrite(p,1,1280*720*4,stdout)!=1280*720*4)return 7;free(p);SDL_DestroyTexture(t);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);SDL_Quit();return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-splash-reference-') as temporary:
 root=Path(temporary);(root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_image'],text=True).split()],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=1280,height=720)
 try:
  def capture(timer):return subprocess.check_output([root/'reference',args.original/'assets/antigravity.webp',str(timer)],env=env)
  alphas=capture(-1);assert len(alphas)==271
  full=capture(60);assert len(full)==1280*720*4;tiles=[]
  for index in range(6):
   x=(index%3)*510;y=(index//3)*510;cw=min(510,1280-x);ch=min(510,720-y);left=max(0,x-1);top=max(0,y-1);right=min(1280,x+cw+1);bottom=min(720,y+ch+1);w=right-left;h=bottom-top
   pixels=b''.join(full[(row*1280+left)*4:(row*1280+right)*4] for row in range(top,bottom));tiles.append(dict(core=[x,y,cw,ch],source=[left,top,w,h],rgba_sha256=hashlib.sha256(pixels).hexdigest()))
   if args.write_assets:
    args.write_assets.mkdir(parents=True,exist_ok=True)
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    raw=b''.join(b'\0'+b''.join(pixels[(row*w+col)*4:(row*w+col)*4+3] for col in range(w)) for row in range(h))
    png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw,0))+chunk(b'IEND',b'')
    (args.write_assets/('splash-'+str(index)+'.png')).write_bytes(png)
  cases=[]
  for timer in [0,1,2,29,30,59,60,61,120,209,210,211,239,240,268,269,270]:
   pixels=capture(timer);cases.append(dict(timer=timer,alpha=alphas[timer],rgba_sha256=hashlib.sha256(pixels).hexdigest(),xor_zlib_hex=zlib.compress(bytes(a^b for a,b in zip(pixels,full))).hex()))
  print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',size=[1280,720],alphas=list(alphas),tiles=tiles,full_rgba_zlib_hex=zlib.compress(full).hex(),cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
