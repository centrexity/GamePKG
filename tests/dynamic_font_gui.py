"""Original Unicode editor text, negative bearings, viewport clipping, long names and stale pixels in eight players."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,os,shutil,subprocess,tempfile,threading,time
from PIL import Image
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--original',type=Path,default=Path('/tmp/gamepkg-candidate-arkanoo'));args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg';GUI=PROJECT/'target/release/gamepkg-gui'
PROOF=PROJECT/'target/dynamic-font-proof';PROOF.mkdir(parents=True,exist_ok=True)
# Complete old/new label region includes both tile seams, transparent margins
# and vacated pixels. No selected bright pixels or tolerance masks.
POINTS=[(x,y) for y in list(range(55,101))+list(range(350,381)) for x in range(640)]
PHASES=[0,1,2,0];KEYS={1:'Right',2:'Up'}
REFERENCE={}
def expected(phase,width=640,height=480):
 key=(phase,width,height)
 if key not in REFERENCE:
  raw=subprocess.check_output([str(reference),str(font),str(width),str(height),str(phase)],env=env);assert len(raw)==width*height*4
  scale=min(width/640,height/480);ox=(width-640*scale)/2;oy=(height-480*scale)/2
  REFERENCE[key]=[list(raw[(round(oy+y*scale)*width+round(ox+x*scale))*4:(round(oy+y*scale)*width+round(ox+x*scale))*4+3]) for x,y in POINTS]
 return REFERENCE[key]
def check(actual,phase,label,width=640,height=480):
 wanted=expected(phase,width,height);delta=max(abs(a-b) for p,q in zip(actual,wanted) for a,b in zip(p,q));print(label,phase,'maximum original-Unicode-text channel difference',delta,flush=True)
 assert delta<=2,(label,phase,delta,[(POINTS[i],p,q) for i,(p,q) in enumerate(zip(actual,wanted)) if max(abs(a-b) for a,b in zip(p,q))>2][:8])
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-dynamic-font-') as temporary:
 root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 def run(*a):
  r=subprocess.run([str(CLI),*map(str,a)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr
 key=root/'key';key.write_text('54'*32)
 font=args.original/'assets/font/static/Roboto-Medium.ttf'
 import hashlib
 assert hashlib.sha256(font.read_bytes()).hexdigest()=='616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'
 assert hashlib.sha256((args.original/'src/rendering.rs').read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
 reference=root/'reference'
 c='\n#include <SDL.h>\n#include <SDL_ttf.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\nvoid label(SDL_Renderer*r,TTF_Font*f,const char*s,int x,int y,int centered,SDL_Color color){SDL_Surface*a=TTF_RenderUTF8_Blended(f,s,color);if(!a)return;SDL_Texture*t=SDL_CreateTextureFromSurface(r,a);SDL_Rect d={centered?x-a->w/2:x,y+120,a->w,a->h};SDL_FreeSurface(a);SDL_RenderCopy(r,t,NULL,&d);SDL_DestroyTexture(t);}\nint main(int argc,char**argv){if(argc!=5||SDL_Init(SDL_INIT_VIDEO)||TTF_Init())return 1;int w=atoi(argv[2]),h=atoi(argv[3]),phase=atoi(argv[4]);SDL_Window*win=SDL_CreateWindow("dynamic-text-reference",0,0,w,h,SDL_WINDOW_HIDDEN);SDL_Renderer*r=SDL_CreateRenderer(win,-1,0);SDL_RendererInfo info;if(!r||SDL_GetRendererInfo(r,&info)||strcmp(info.name,"opengl"))return 2;SDL_RenderSetLogicalSize(r,1280,960);SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY,"linear");TTF_Font*f=TTF_OpenFont(argv[1],26);if(!f)return 3;SDL_SetRenderDrawColor(r,0,0,0,255);SDL_RenderClear(r);SDL_Rect clip={0,120,1280,720};SDL_RenderSetClipRect(r,&clip);\nconst char*names[3]={"Name: my_pattern (N to edit)","Name: café j‍́ i‌́  ️_ (ESC to finish)","Name: Ελληνικά Привет 日本語 🎮 // “Δ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa (N to edit)"};\nconst char*messages[3]={"Saved: café","Message “Δ // aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","Saved: café"};\nlabel(r,f,names[phase],10,40,0,phase==1?(SDL_Color){100,255,100,255}:(SDL_Color){200,200,200,255});label(r,f,messages[phase],640,590,1,(SDL_Color){100,255,100,255});if(phase==2)label(r,f,"Áj Привет",-20,-10,0,(SDL_Color){100,255,100,255});unsigned char*raw=malloc(w*h*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,raw,w*4)||fwrite(raw,1,w*h*4,stdout)!=(size_t)w*h*4)return 4;free(raw);TTF_CloseFont(f);SDL_DestroyRenderer(r);SDL_DestroyWindow(win);TTF_Quit();SDL_Quit();return 0;}\n'
 (root/'reference.c').write_text(c);subprocess.run(['cc',root/'reference.c','-o',reference,*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf'],text=True).split()],check=True)
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  source=root/suffix;shutil.copytree(PROJECT/'ports/arkanoo',source);entry=source/'source/src/lib.rs';modules=entry.read_text().split('use std::sync::Mutex;')[0]
  entry.write_text(modules+"""pub fn game_tick(input:u32,_:u64)->u64 {
   let phase=if input&gamepkg::HELD_RIGHT!=0 {1} else if input&gamepkg::HELD_UP!=0 {2} else {0};
   let names=["Name: my_pattern (N to edit)","Name: café j‍́ i‌́  ️_ (ESC to finish)","Name: Ελληνικά Привет 日本語 🎮 // “Δ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa (N to edit)"];let messages=["Saved: café","Message “Δ // aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","Saved: café"];
   gamepkg::draw_clear(0x000000ff);render::draw_dynamic_label(names[phase as usize],10,40,if phase==1 {[100,255,100]} else {[200,200,200]},false,-1);render::draw_dynamic_label(messages[phase as usize],640,590,[100,255,100],true,-1);if phase==2 {render::draw_dynamic_label("Áj Привет",-20,-10,[100,255,100],false,-1);}
   gamepkg::draw_rect(4.0,4.0,16.0,16.0,if phase==1 {0x00ff00ff} else if phase==2 {0x0000ffff} else {0xff0000ff});phase
  }""")
  manifest=source/'manifest.toml';manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.clip-'+suffix));package=root/(suffix+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.gamepkg.clip-'+suffix,'--target','wasm')
 display=start_display(env,width=1100,height=850)
 try:
  for profile,suffix in [('core-v1','core'),('component-v1','component')]:
   game_id='com.gamepkg.clip-'+suffix
   for target in (['wasm','native'] if suffix=='core' else ['wasm']):
    if target=='wasm' and suffix=='core':run('build',game_id,'--target','wasm')
    if target=='native':run('build',game_id,'--target','native')
    for renderer in ['vulkan','opengl']:
     label=profile+'-'+target+'-'+renderer;app=subprocess.Popen([str(GUI),'--play',game_id,'--renderer',renderer],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);held=False
     try:
      deadline=time.monotonic()+20
      while True:
       if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
       found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
       if found.returncode==0:window=found.stdout.splitlines()[0];break
       assert time.monotonic()<deadline;time.sleep(.05)
      subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
      for phase in PHASES:
       if held:subprocess.run(['xdotool','keyup',held],env=env,check=True);held=False
       if phase:held=KEYS[phase];subprocess.run(['xdotool','keydown',held],env=env,check=True)
       deadline=time.monotonic()+12
       while True:
        path=PROOF/(label+'-'+str(phase)+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);im=Image.open(path).convert('RGB');scale=min(im.width/640,im.height/480);ox=(im.width-640*scale)/2;oy=(im.height-480*scale)/2;actual=[list(im.getpixel((round(ox+x*scale),round(oy+y*scale)))) for x,y in POINTS]
        if all(all(abs(a-b)<=2 for a,b in zip(pixel,wanted)) for pixel,wanted in zip(actual,expected(phase,im.width,im.height))):break
        if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
        assert time.monotonic()<deadline,(label,phase,actual,expected(phase,im.width,im.height));time.sleep(.05)
       check(actual,phase,label,im.width,im.height)
     finally:
      if held:subprocess.run(['xdotool','keyup',held],env=env,check=True)
      if app.poll() is None:app.terminate()
      app.communicate(timeout=10)
   from selenium import webdriver
   from selenium.webdriver.chrome.options import Options
   from selenium.webdriver.chrome.service import Service
   from selenium.webdriver.common.action_chains import ActionChains
   from selenium.webdriver.common.keys import Keys
   from selenium.webdriver.support.ui import WebDriverWait
   output=root/(suffix+'-web');run('web-export',game_id,'--output',output);server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert options.binary_location
   for value in ['--headless=new','--mute-audio','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader']:options.add_argument(value)
   driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');browser=webdriver.Chrome(service=Service(driver),options=options);held=False
   try:
    browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();wait.until(lambda b:b.find_element('id','game').get_attribute('data-state'));browser.find_element('id','game').click()
    for phase in PHASES:
     if held:ActionChains(browser).key_up(held).perform();held=False
     if phase:held={1:Keys.ARROW_RIGHT,2:Keys.ARROW_UP,3:Keys.ARROW_DOWN,4:Keys.ARROW_LEFT}[phase];ActionChains(browser).key_down(held).perform()
     wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')==str(phase));actual=browser.execute_script('const c=document.querySelector("#game").getContext("2d");return arguments[0].map(p=>Array.from(c.getImageData(p[0],p[1],1,1).data).slice(0,3));',POINTS);check(actual,phase,'browser-'+profile)
   finally:
    if held:ActionChains(browser).key_up(held).perform()
    browser.quit();server.shutdown();server.server_close()
 finally:display.terminate();display.wait(timeout=10)
