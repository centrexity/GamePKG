"""Original menu backdrop and currently implemented controls; not full settings parity.
Both audio profiles, desktop resolution and VSync widgets are included.
"""
from pathlib import Path
import argparse,hashlib,json,os,struct,subprocess,tempfile,zlib,sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);parser.add_argument('--write-menu-assets',type=Path);args=parser.parse_args()
for name,digest in [('src/rendering.rs','51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'),('src/menu.rs','63bcc800d4d7b50eaf89a87c4c2e1c074d76b2cf578c777ce38edb0806903f28'),('assets/font/static/Roboto-Medium.ttf','616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'),('assets/background1.png','21c34d3db19f1d2f8560bada107af03762a4003dfbc2c9a3c9b80b0bc9b3f8b2')]:
 if digest:assert hashlib.sha256((args.original/name).read_bytes()).hexdigest()==digest
assert 'version = "0.5.0"' in (args.original/'Cargo.toml').read_text()
c=r'''
#include <SDL.h>
#include <SDL_ttf.h>
#include <SDL_image.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
void label(SDL_Renderer*r,TTF_Font*f,const char*s,int x,int y,int center,int box_w,int box_h){SDL_Surface*a=TTF_RenderUTF8_Blended(f,s,(SDL_Color){255,255,255,255});if(!a)return;SDL_Texture*t=SDL_CreateTextureFromSurface(r,a);SDL_Rect d={box_w>=0?x+(box_w-a->w)/2:center?x-a->w/2:x,box_h>=0?y+(box_h-a->h)/2:y,a->w,a->h};SDL_FreeSurface(a);SDL_RenderCopy(r,t,NULL,&d);SDL_DestroyTexture(t);}
void button(SDL_Renderer*r,TTF_Font*f,const char*s,int x,int y,int action,int hover,int width){SDL_Rect box={x,y,width,40};SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,action==hover?100:60,action==hover?100:60,action==hover?150:100,action==hover?200:180);SDL_RenderFillRect(r,&box);SDL_SetRenderDrawColor(r,200,200,200,255);SDL_RenderDrawRect(r,&box);label(r,f,s,x,y,0,width,40);}
void resolution_button(SDL_Renderer*r,TTF_Font*f,const char*s,int x,int y,int action,int hover,int width,int height,int selected){SDL_Rect box={x,y,width,height};SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);if(selected){SDL_SetRenderDrawColor(r,100,200,100,100);SDL_RenderFillRect(r,&box);}SDL_SetRenderDrawColor(r,action==hover?100:60,action==hover?100:60,action==hover?150:100,action==hover?200:180);SDL_RenderFillRect(r,&box);SDL_SetRenderDrawColor(r,200,200,200,255);SDL_RenderDrawRect(r,&box);label(r,f,s,x,y,0,width,height);}
int main(int argc,char**argv){if((argc!=14&&argc!=16)||SDL_Init(SDL_INIT_VIDEO)||TTF_Init())return 1;int settings=atoi(argv[3]),started=atoi(argv[4]),hover=atoi(argv[5]),volume=atoi(argv[6]),muted=atoi(argv[7]),fullscreen=atoi(argv[8]),resolution=atoi(argv[9]),pending=atoi(argv[10]),vsync=atoi(argv[11]),music=atoi(argv[12]),music_volume=atoi(argv[13]);int width=argc==16?atoi(argv[14]):1280,height=argc==16?atoi(argv[15]):720;if(!((width==1280&&height==720)||(width==640&&height==360)))return 12;SDL_Window*w=SDL_CreateWindow("menu-control-reference",0,0,width,height,SDL_WINDOW_HIDDEN);SDL_Renderer*r=SDL_CreateRenderer(w,-1,0);SDL_RendererInfo info;if(!r||SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 2;if(SDL_RenderSetScale(r,width/1280.0f,height/720.0f))return 13;SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");TTF_Font*f=TTF_OpenFont(argv[1],26);SDL_Texture*bg=IMG_LoadTexture(r,argv[2]);if(!f||!bg)return 3;SDL_SetRenderDrawColor(r,0,0,0,255);SDL_RenderClear(r);SDL_RenderCopy(r,bg,NULL,NULL);SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,0,0,0,150);if(settings>=-1)SDL_RenderFillRect(r,NULL);
if(settings>=0){label(r,f,settings?"SETTINGS":started?"PAUSED":"ARKANOO",640,settings?160:180,1,-1,-1);
if(!settings){const char*words[6]={started?"Resume":"New Game","Restart","Gravity Mode","Level Editor","Settings","Quit"};for(int i=0;i<6;i++)button(r,f,words[i],540,235+i*50,i+1,hover,200);button(r,f,"Github",1170,670,9,hover,100);SDL_Surface*v=TTF_RenderUTF8_Blended(f,"Version: 0.5.0",(SDL_Color){150,150,150,255});if(!v)return 5;SDL_Texture*t=SDL_CreateTextureFromSurface(r,v);SDL_Rect d={1150-v->w,670+(40-v->h)/2,v->w,v->h};SDL_FreeSurface(v);if(!t)return 6;SDL_RenderCopy(r,t,NULL,&d);SDL_DestroyTexture(t);}
else{button(r,f,muted?"SFX: OFF":"SFX: ON",540,310,10,hover,200);button(r,f,fullscreen?"Fullscreen":"Windowed",540,400,7,hover,200);button(r,f,"Back",540,760,8,hover,200);SDL_Rect box={540,360,200,20};SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,80,80,80,200);SDL_RenderFillRect(r,&box);SDL_Rect fill={540,360,volume*200/128,20};SDL_SetRenderDrawColor(r,100,200,100,220);SDL_RenderFillRect(r,&fill);SDL_SetRenderDrawColor(r,200,200,200,255);SDL_RenderDrawRect(r,&box);char percent[32];snprintf(percent,sizeof(percent),"%d%%",volume*100/128);label(r,f,percent,540,360,0,200,20);
if(music){button(r,f,music==2?"Music: OFF":"Music: ON",540,220,12,hover,200);SDL_Rect music_box={540,270,200,20};SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,80,80,80,200);SDL_RenderFillRect(r,&music_box);SDL_Rect music_fill={540,270,music_volume*200/128,20};SDL_SetRenderDrawColor(r,100,200,100,220);SDL_RenderFillRect(r,&music_fill);SDL_SetRenderDrawColor(r,200,200,200,255);SDL_RenderDrawRect(r,&music_box);char music_percent[32];snprintf(music_percent,sizeof(music_percent),"%d%%",music_volume*100/128);label(r,f,music_percent,540,270,0,200,20);}
if(vsync){button(r,f,vsync==2?"VSync: ON":"VSync: OFF",540,450,20,hover,200);SDL_Surface*a=TTF_RenderUTF8_Blended(f,"(VSync change requires restart)",(SDL_Color){120,120,120,255});if(!a)return 10;SDL_Texture*t=SDL_CreateTextureFromSurface(r,a);SDL_Rect d={640-a->w/2,190,a->w,a->h};SDL_FreeSurface(a);if(!t)return 11;SDL_RenderCopy(r,t,NULL,&d);SDL_DestroyTexture(t);}
if(resolution){label(r,f,"Resolution:",640,490,1,-1,-1);if(pending){SDL_Rect dialog={440,680,400,80};SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,30,30,50,220);SDL_RenderFillRect(r,&dialog);SDL_SetRenderDrawColor(r,100,100,150,255);SDL_RenderDrawRect(r,&dialog);char message[64];snprintf(message,sizeof(message),"Keep this resolution? (%ds)",((pending-1)+59)/60);SDL_Surface*a=TTF_RenderUTF8_Blended(f,message,(SDL_Color){255,255,100,255});if(!a)return 7;SDL_Texture*t=SDL_CreateTextureFromSurface(r,a);SDL_Rect d={640-a->w/2,690,a->w,a->h};SDL_FreeSurface(a);if(!t)return 8;SDL_RenderCopy(r,t,NULL,&d);SDL_DestroyTexture(t);resolution_button(r,f,"Keep",480,710,18,hover,100,35,0);resolution_button(r,f,"Revert",600,710,19,hover,100,35,0);}else if(!fullscreen){const char*res[5]={"1280x720","1366x768","1600x900","1920x1080","2560x1440"};for(int i=0;i<5;i++)resolution_button(r,f,res[i],540,510+i*35,13+i,hover,200,30,resolution==i+1);}else{SDL_Surface*a=TTF_RenderUTF8_Blended(f,"Exit fullscreen to change",(SDL_Color){200,200,200,255});if(!a)return 9;SDL_Texture*t=SDL_CreateTextureFromSurface(r,a);SDL_Rect d={640-a->w/2,530,a->w,a->h};SDL_FreeSurface(a);SDL_RenderCopy(r,t,NULL,&d);SDL_DestroyTexture(t);}}}}
unsigned char*p=malloc(width*height*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,p,width*4)||fwrite(p,1,width*height*4,stdout)!=(size_t)(width*height*4))return 4;free(p);TTF_CloseFont(f);SDL_DestroyTexture(bg);SDL_DestroyRenderer(r);SDL_DestroyWindow(w);TTF_Quit();SDL_Quit();return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-menu-controls-') as temporary:
 root=Path(temporary);(root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',root/'reference',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf','SDL2_image'],text=True).split()],check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=1280,height=720)
 try:
  def capture(settings,started,hover,volume,muted,fullscreen,resolution=0,pending=0,vsync=0,music=0,music_volume=64):return subprocess.check_output([root/'reference',args.original/'assets/font/static/Roboto-Medium.ttf',args.original/'assets/background1.png',str(settings),str(started),str(hover),str(volume),str(muted),str(fullscreen),str(resolution),str(pending),str(vsync),str(music),str(music_volume)],env=env)
  base=capture(-1,0,0,64,0,0);assert len(base)==1280*720*4
  full=capture(-2,0,0,64,0,0);assert len(full)==len(base);tiles=[]
  for index in range(6):
   x=(index%3)*510;y=(index//3)*510;core_w=min(510,1280-x);core_h=min(510,720-y);left=max(0,x-1);top=max(0,y-1);right=min(1280,x+core_w+1);bottom=min(720,y+core_h+1);w=right-left;h=bottom-top
   pixels=b''.join(full[(row*1280+left)*4:(row*1280+right)*4] for row in range(top,bottom));assert w<=512 and h<=512
   tiles.append(dict(core=[x,y,core_w,core_h],source=[left,top,w,h],rgba_sha256=hashlib.sha256(pixels).hexdigest()))
   if args.write_menu_assets:
    args.write_menu_assets.mkdir(parents=True,exist_ok=True)
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    raw=b''.join(b'\0'+b''.join(pixels[(row*w+col)*4:(row*w+col)*4+3] for col in range(w)) for row in range(h))
    png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw,0))+chunk(b'IEND',b'')
    (args.write_menu_assets/('menu-background1-'+str(index)+'.png')).write_bytes(png)

  states=[(0,started,hover,64,0,0) for started in [0,1] for hover in [0,1,2,3,4,5,6,9]]+[(1,0,[0,7,10,8][i%4],volume,muted,fullscreen) for i,(volume,muted,fullscreen) in enumerate([(v,m,f) for v in [0,1,63,64,127,128] for m in [0,1] for f in [0,1]])]
  states=[(*values,0,0) for values in states]+[(1,0,hover,64,0,0,resolution,0) for resolution in range(1,6) for hover in [0,12+resolution]]+[(1,0,hover,64,0,0,1,ticks+1) for ticks in [300,240,121,1,0] for hover in [18,19]]+[(1,0,0,64,0,0,6,0),(1,0,0,64,0,1,6,0)]
  states=[(*values,0) for values in states]+[(1,0,hover,64,0,0,1,0,vsync) for vsync in [1,2] for hover in [0,20]]+[(1,0,18,64,0,0,1,301,vsync) for vsync in [1,2]]
  states=[(*values,0,64) for values in states]+[(1,0,hover,64,0,0,1,0,2,music,volume) for music in [1,2] for volume in [0,32,64,128] for hover in [0,12]]
  assert len(states)==84
  cases=[]
  for values in states:
   raw=capture(*values);assert len(raw)==len(base);cases.append(dict(settings=values[0],started=values[1],hover=values[2],volume=values[3],muted=values[4],fullscreen=values[5],resolution=values[6],pending=values[7],vsync=values[8],music=values[9],music_volume=values[10],rgba_sha256=hashlib.sha256(raw).hexdigest(),xor_zlib_hex=zlib.compress(bytes(a^b for a,b in zip(raw,base))).hex()))
  exe=root/'text';subprocess.run(['cc',Path(__file__).with_name('sdl_dynamic_font.c'),'-o',exe,*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf','harfbuzz','freetype2'],text=True).split()],check=True);labels={}
  for word in ['ARKANOO','PAUSED','SETTINGS','New Game','Resume','Restart','Gravity Mode','Level Editor','Settings','Quit','SFX: ON','SFX: OFF','Windowed','Fullscreen','Back','Github','Version: 0.5.0','Music: ON','Music: OFF','25%','VSync: ON','VSync: OFF','(VSync change requires restart)','0%','50%','100%']:
   raw=subprocess.check_output([exe,args.original/'assets/font/static/Roboto-Medium.ttf','text',word]);w,h=struct.unpack('<2i',raw[:8]);labels[word]=dict(width=w,height=h,rgba_zlib_hex=zlib.compress(raw[8:]).hex())
  print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',scope='original backdrop, six main controls, original Github button/version label and implemented SFX/fullscreen/back plus resolution/confirmation and VSync settings; restart notice relocated to y=190 to avoid original overlapping text; recorded-profile music mute/volume included',size=[1280,720],menu_tiles=tiles,full_bright_sha256=hashlib.sha256(full).hexdigest(),base_rgba_zlib_hex=zlib.compress(base).hex(),labels=labels,cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
