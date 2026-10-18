"""Original editor static lettering and toolbar, including overlapping filtered text tiles, in six canonical desktop configurations."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,os,shutil,subprocess,tempfile,threading,time
from PIL import Image
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--original',type=Path,default=Path('/tmp/gamepkg-candidate-arkanoo'));args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg';GUI=PROJECT/'target/release/gamepkg-gui'
PROOF=PROJECT/'target/editor-labels-proof';PROOF.mkdir(parents=True,exist_ok=True)
# Complete static-label and toolbar regions, including transparent margins,
# overlapping controls and all glyph pixels. No tolerance masks.
REGIONS=[(10,10,170,31),(760,25,83,31),(296,620,688,31),(1084,570,186,31)]+[(225+i*170,660,150,40) for i in range(5)]+[(1020,610,120,35),(1150,610,120,35)]
POINTS=[(x/2,y/2+60) for left,top,w,h in REGIONS for y in range(top,top+h) for x in range(left,left+w)]
PHASES=[0,1,2,0];KEYS={1:'Right',2:'Up'}
REFERENCE={}
def expected(phase,width=640,height=480):
 key=(phase,width,height)
 if key not in REFERENCE:
  raw=subprocess.check_output([str(reference),str(args.original/'assets/background1.png'),str(font),str([0,1,8][phase]),str([255,2,8][phase])],env=env);assert len(raw)==width*height*4
  scale=min(width/640,height/480);ox=(width-640*scale)/2;oy=(height-480*scale)/2
  REFERENCE[key]=[list(raw[(round(oy+y*scale)*width+round(ox+x*scale))*4:(round(oy+y*scale)*width+round(ox+x*scale))*4+3]) for x,y in POINTS]
 return REFERENCE[key]
def check(actual,phase,label,width=640,height=480):
 wanted=expected(phase,width,height);delta=max(abs(a-b) for p,q in zip(actual,wanted) for a,b in zip(p,q));print(label,phase,'maximum original-editor-label channel difference',delta,flush=True)
 assert delta<=2,(label,phase,delta,[(POINTS[i],p,q) for i,(p,q) in enumerate(zip(actual,wanted)) if max(abs(a-b) for a,b in zip(p,q))>2][:8])
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-editor-labels-') as temporary:
 root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 def run(*a):
  r=subprocess.run([str(CLI),*map(str,a)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr
 key=root/'key';key.write_text('54'*32)
 font=args.original/'assets/font/static/Roboto-Medium.ttf'
 import hashlib
 assert hashlib.sha256(font.read_bytes()).hexdigest()=='616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'
 assert hashlib.sha256((args.original/'src/rendering.rs').read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
 reference=root/'reference';import ast
 tree=ast.parse((PROJECT/'tests/reference/arkanoo_editor_palette.py').read_text());original_c=next(ast.literal_eval(node.value) for node in tree.body if isinstance(node,ast.Assign) and any(isinstance(target,ast.Name) and target.id=='c' for target in node.targets))
 original_c=original_c.replace('0,0,1280,720,SDL_WINDOW_HIDDEN','0,0,1280,960,SDL_WINDOW_HIDDEN').replace('SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY', 'SDL_RenderSetLogicalSize(r,1280,960);SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY').replace('SDL_RenderCopy(r,bg,NULL,NULL);','SDL_Rect background_dst={0,120,1280,720};SDL_RenderCopy(r,bg,NULL,&background_dst);').replace('SDL_RenderFillRect(r,NULL);','SDL_RenderFillRect(r,&background_dst);').replace('860+index*45,20,40,30','860+index*45,140,40,30').replace('SDL_Rect region={858,18,404,34};unsigned char pixels[404*34*4]','SDL_Rect region={0,0,1280,960};unsigned char pixels[1280*960*4]').replace('pixels,404*4','pixels,1280*4')

 # These are original SDL primitives at original pixel size, including the
 # original instruction line being rendered after the background buttons.
 static_c=r"""
const char*labels[7]={"Save (S)","Clear (C)","Test (T)","Load (L)","Exit (ESC)","< BG Prev","BG Next >"};
for(int i=0;i<7;i++){
 SDL_Rect rect=i<5?(SDL_Rect){225+i*170,780,150,40}:i==5?(SDL_Rect){1020,730,120,35}:(SDL_Rect){1150,730,120,35};
 int hover=(selected==1&&i==0)||(selected==8&&i==5);SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);SDL_SetRenderDrawColor(r,hover?100:60,hover?100:60,hover?150:100,hover?200:180);SDL_RenderFillRect(r,&rect);SDL_SetRenderDrawColor(r,200,200,200,255);SDL_RenderDrawRect(r,&rect);
 SDL_Surface*s=TTF_RenderUTF8_Blended(f,labels[i],(SDL_Color){255,255,255,255});SDL_Texture*t=SDL_CreateTextureFromSurface(r,s);SDL_Rect dst={rect.x+(rect.w-s->w)/2,rect.y+(rect.h-s->h)/2,s->w,s->h};SDL_FreeSurface(s);SDL_RenderCopy(r,t,NULL,&dst);SDL_DestroyTexture(t);
}
const char*words[4]={"LEVEL EDITOR","Colors:","Drag Left: Draw | Drag Right: Erase | Click colors or 0-5 keys","Background 1/6"};int positions[4][2]={{10,130},{760,145},{296,740},{1084,690}};int shades[4]={255,200,180,150};
for(int i=0;i<4;i++){SDL_Surface*s=TTF_RenderUTF8_Blended(f,words[i],(SDL_Color){shades[i],shades[i],shades[i],255});SDL_Texture*t=SDL_CreateTextureFromSurface(r,s);SDL_Rect dst={positions[i][0],positions[i][1],s->w,s->h};SDL_FreeSurface(s);SDL_RenderCopy(r,t,NULL,&dst);SDL_DestroyTexture(t);}
"""
 original_c=original_c.replace('SDL_Rect region={0,0,1280,960};',static_c+'SDL_Rect region={0,0,1280,960};')
 (root/'reference.c').write_text(original_c)
 subprocess.run(['cc',root/'reference.c','-o',reference,*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_image','SDL2_ttf'],text=True).split()],check=True)
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  source=root/suffix;shutil.copytree(PROJECT/'ports/arkanoo',source);entry=source/'source/src/lib.rs';modules=entry.read_text().split('use std::sync::Mutex;')[0]
  entry.write_text(modules+"""pub fn game_tick(input:u32,_:u64)->u64 {
   let phase=if input&gamepkg::HELD_RIGHT!=0 {1} else if input&gamepkg::HELD_UP!=0 {2} else {0};
   gamepkg::draw_clear(0x000000ff);render::draw_image_background(1,153);render::draw_editor_grid();
   render::draw_editor_palette(if phase==1 {1} else if phase==2 {8} else {0},if phase==1 {2} else if phase==2 {8} else {255});
   render::draw_editor_label(0,10,10);render::draw_editor_label(1,760,25);
   for which in 0..7 {render::draw_editor_button(which,if phase==1 {which==0} else if phase==2 {which==5} else {false});}
   render::draw_editor_label(2,296,620);render::draw_editor_label(10,1084,570);
   gamepkg::draw_rect(4.0,4.0,16.0,16.0,if phase==1 {0x00ff00ff} else if phase==2 {0x0000ffff} else {0xff0000ff});phase
  }""")
  manifest=source/'manifest.toml';manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.clip-'+suffix));package=root/(suffix+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.gamepkg.clip-'+suffix,'--target','wasm')
 display=start_display(env,width=1600,height=1200)
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
      subprocess.run(['xdotool','windowsize',window,'1280','960'],env=env,check=True);time.sleep(.15)
      subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
      for phase in PHASES:
       if held:subprocess.run(['xdotool','keyup',held],env=env,check=True);held=False
       if phase:held=KEYS[phase];subprocess.run(['xdotool','keydown',held],env=env,check=True)
       deadline=time.monotonic()+12
       while True:
        path=PROOF/(label+'-'+str(phase)+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);im=Image.open(path).convert('RGB');assert im.size==(1280,960),im.size;scale=min(im.width/640,im.height/480);ox=(im.width-640*scale)/2;oy=(im.height-480*scale)/2;actual=[list(im.getpixel((round(ox+x*scale),round(oy+y*scale)))) for x,y in POINTS]
        if all(all(abs(a-b)<=2 for a,b in zip(pixel,wanted)) for pixel,wanted in zip(actual,expected(phase,im.width,im.height))):break
        if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())

        if time.monotonic()>=deadline:check(actual,phase,label,im.width,im.height)
        time.sleep(.05)
       check(actual,phase,label,im.width,im.height)
     finally:
      if held:subprocess.run(['xdotool','keyup',held],env=env,check=True)
      if app.poll() is None:app.terminate()
      app.communicate(timeout=10)
 finally:display.terminate();display.wait(timeout=10)
