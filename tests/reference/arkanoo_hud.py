"""Original full HUD on a contrast backdrop; captures real Roboto and heart sampling."""
from pathlib import Path
import argparse,hashlib,json,os,struct,subprocess,tempfile,zlib,sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);args=parser.parse_args()
for name,digest in [('src/rendering.rs','51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'),('assets/font/static/Roboto-Medium.ttf','616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'),('assets/heart.png','86b8ef21b5544f39542c247518155f4a856aab70851bbd76cfb67df4bd0fbd71')]:assert hashlib.sha256((args.original/name).read_bytes()).hexdigest()==digest
c=r'''
#include <SDL.h>
#include <SDL_ttf.h>
#include <SDL_image.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <limits.h>
float number(const char*s){uint32_t bits=(uint32_t)strtoul(s,NULL,10);float value;memcpy(&value,&bits,4);return value;}
void label(SDL_Renderer*r,TTF_Font*f,const char*text,int x,int y,int align,SDL_Color color){SDL_Surface*s=TTF_RenderUTF8_Blended(f,text,color);if(!s)exit(5);SDL_Texture*t=SDL_CreateTextureFromSurface(r,s);SDL_Rect box={align==1?x-s->w/2:align==2?x-s->w:x,y<0?710-s->h:y,s->w,s->h};SDL_FreeSurface(s);if(!t||SDL_RenderCopy(r,t,NULL,&box))exit(6);SDL_DestroyTexture(t);}
int main(int argc,char**argv){if(argc!=15||SDL_Init(SDL_INIT_VIDEO)||TTF_Init())return 1;uint32_t score=strtoul(argv[3],NULL,10),level=strtoul(argv[4],NULL,10),lives=strtoul(argv[5],NULL,10),fps=strtoul(argv[10],NULL,10);int testing=atoi(argv[6]),gravity=atoi(argv[11]),stolen=atoi(argv[12]);float speed=number(argv[7]);uint64_t frame=strtoull(argv[8],NULL,10),record=strtoull(argv[9],NULL,10);SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");SDL_Window*w=SDL_CreateWindow("hud-reference",0,0,1280,720,SDL_WINDOW_HIDDEN);SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);SDL_RendererInfo info;if(!r||SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 2;TTF_Font*f=TTF_OpenFont(argv[1],26);SDL_Texture*heart=IMG_LoadTexture(r,argv[2]);if(!f||!heart)return 3;SDL_SetRenderDrawColor(r,120,180,240,255);SDL_RenderClear(r);SDL_Rect stripe={0,150,1280,200};SDL_SetRenderDrawColor(r,230,50,90,255);SDL_RenderFillRect(r,&stripe);
char text[100];snprintf(text,sizeof(text),"Score: %u",score);label(r,f,text,10,10,0,(SDL_Color){255,255,255,255});for(uint32_t i=0;i<lives;i++){SDL_Rect box={1240-(int)i*25,15,20,20};SDL_RenderCopy(r,heart,NULL,&box);}if(stolen){SDL_Rect box={(int)number(argv[13])-10,(int)number(argv[14])-10,20,20};SDL_RenderCopy(r,heart,NULL,&box);}if(testing)strcpy(text,"TEST");else snprintf(text,sizeof(text),"Level %u",level);label(r,f,text,640,10,1,(SDL_Color){255,255,255,255});
int32_t max=isnan(speed)?0:speed>=2147483648.0f?INT_MAX:speed<=-2147483648.0f?INT_MIN:(int32_t)speed;snprintf(text,sizeof(text),"MAX SPEED: %d px/s",max);SDL_Color color={200,200,200,255};if((frame>=record?frame-record:0)<120){float pulse=sinf((float)frame*0.15f)*0.3f+0.7f;color=(SDL_Color){(uint8_t)(255.0f*pulse),(uint8_t)(255.0f*pulse),(uint8_t)(100.0f*pulse+100.0f),255};}label(r,f,text,10,680,0,color);
snprintf(text,sizeof(text),"FPS: %u",fps);label(r,f,text,1270,-1,2,(SDL_Color){200,200,200,255});float gravity_pulse=sinf((float)frame*0.1f)*0.3f+0.7f;SDL_Color gravity_color={(uint8_t)(255.0f*gravity_pulse),(uint8_t)(100.0f*gravity_pulse),0,255};if(gravity){label(r,f,"GRAVITY MODE",640,35,1,gravity_color);}
if(getenv("HUD_COLORS")){unsigned char colors[6]={color.r,color.g,color.b,gravity_color.r,gravity_color.g,gravity_color.b};if(fwrite(colors,1,6,stdout)!=6)return 8;}
unsigned char*p=malloc(1280*720*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,p,1280*4)||fwrite(p,1,1280*720*4,stdout)!=1280*720*4)return 4;free(p);TTF_CloseFont(f);SDL_DestroyTexture(heart);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);TTF_Quit();SDL_Quit();return 0;}
'''
def bits(value):return struct.unpack('<I',struct.pack('<f',value))[0]
with tempfile.TemporaryDirectory(prefix='gamepkg-hud-reference-') as temporary:
 root=Path(temporary);(root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf','SDL2_image'],text=True).split(),'-lm'],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=1280,height=720)
 try:
  states=[]
  for i,frame in enumerate([0,1,30,59,119,120,121,239,16777217,18446744073709551615]):
   for gravity in [0,1]:states.append([4294967295 if i%2 else 0,10 if i%2 else 1,3,i%2,bits(3599.9),frame,0,[0,1,59,60,61,144,1000][i%7],gravity,0,bits(0),bits(0)])
  for lives in [0,1,6]:states.append([1234567890,4294967295,lives,0,bits(0),500,400,60,1,1,bits(640.75),bits(240.5)])
  for speed in [-2147483648.0,-1.0,0.0,2147483648.0,float('nan')]:states.append([1,9,1,1,bits(speed),40,50,144,1,1,bits(-.75),bits(-1.75)])
  states.append([0,1,3,0,bits(0),0,0,0,0,0,bits(0),bits(0)])
  cases=[]
  for values in states:
   capture=subprocess.check_output([root/'reference',args.original/'assets/font/static/Roboto-Medium.ttf',args.original/'assets/heart.png',*map(str,values)],env=dict(env,HUD_COLORS='1'));colors=capture[:6];pixels=capture[6:];assert len(pixels)==1280*720*4;cases.append(dict(values=values,record_color=list(colors[:3]),gravity_color=list(colors[3:]),rgba_sha256=hashlib.sha256(pixels).hexdigest(),rgba_zlib_hex=zlib.compress(pixels).hex()))
  text_exe=root/'text';subprocess.run(['cc',Path(__file__).with_name('sdl_dynamic_font.c'),'-o',text_exe,*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf','harfbuzz','freetype2'],text=True).split()],check=True);labels={}
  for word in ['Score: 0','Level 1000',*[f'FPS: {n}' for n in range(1,6)]]:
   raw=subprocess.check_output([text_exe,args.original/'assets/font/static/Roboto-Medium.ttf','text',word]);w,h=struct.unpack('<2i',raw[:8]);labels[word]=dict(width=w,height=h,rgba_zlib_hex=zlib.compress(raw[8:]).hex())
  print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',size=[1280,720],labels=labels,scope='full original HUD over a fixed contrast backdrop; actual FPS time window tested separately',cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
