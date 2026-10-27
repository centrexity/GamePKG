"""Original backdrop and implemented menu controls; missing controls remain separate gates."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,hashlib,json,os,shutil,subprocess,tempfile,threading,time
from PIL import Image,ImageChops
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--original',type=Path,default=Path('/tmp/gamepkg-candidate-arkanoo'));args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg';GUI=PROJECT/'target/release/gamepkg-gui';PROOF=PROJECT/'target/menu-controls-proof';PROOF.mkdir(parents=True,exist_ok=True)
fixture=json.loads((PROJECT/'tests/fixtures/arkanoo-menu-controls.json').read_text());PHASES=[0,1,2,3,4,0];KEYS={4:'Left',1:'Right',2:'Up',3:'Down'};REFERENCES={};SMALL={}
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-menu-controls-gui-') as temp:
 root=Path(temp);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n');env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 def run(*args):
  r=subprocess.run([CLI,*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr
 font=args.original/'assets/font/static/Roboto-Medium.ttf';assert hashlib.sha256(font.read_bytes()).hexdigest()=='616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'
 assert hashlib.sha256((args.original/'src/rendering.rs').read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
 reference_source=(PROJECT/'tests/reference/arkanoo_menu_controls.py').read_text().split("c=r'''",1)[1].split("'''",1)[0];(root/'reference.c').write_text(reference_source);reference=root/'reference';subprocess.run(['cc',root/'reference.c','-o',reference,*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf','SDL2_image'],text=True).split()],check=True)
 key=root/'key';key.write_text('54'*32)
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  source=root/suffix;shutil.copytree(PROJECT/'ports/arkanoo',source);entry=source/'source/src/lib.rs';modules=entry.read_text().split('use std::sync::Mutex;')[0]
  entry.write_text(modules+"""pub fn game_tick(input:u32,_:u64)->u64 {
 let phase=if input&gamepkg::HELD_RIGHT!=0 {1} else if input&gamepkg::HELD_UP!=0 {2} else if input&gamepkg::HELD_DOWN!=0 {3} else if input&gamepkg::HELD_LEFT!=0 {4} else {0};let mut game=physics::Game::new();game.mode=0;game.started=phase==1;game.menu.settings=phase==2||phase==3;game.menu.hover=if phase==1 {3} else if phase==2 {10} else if phase==3 {7} else if phase==4 {9} else {0};game.menu.sfx_volume=if phase==2 {127} else if phase==3 {1} else {64};game.menu.sfx_muted=phase==2;game.menu.fullscreen=phase==3;render::draw_menu(&game);
 gamepkg::draw_rect(4.0,4.0,16.0,16.0,if phase==1 {0x00ff00ff} else if phase==2 {0x0000ffff} else if phase==3 {0xffff00ff} else if phase==4 {0xff00ffff} else {0xff0000ff});phase
}""")
  manifest=source/'manifest.toml';manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.menu-'+suffix));package=root/(suffix+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.gamepkg.menu-'+suffix,'--target','wasm')
 display=start_display(env,width=1600,height=1200)
 try:
  states=[(0,0,0,64,0,0),(0,1,3,64,0,0),(1,0,10,127,1,0),(1,0,7,1,0,1),(0,0,9,64,0,0)]
  for phase,state in enumerate(states):
   # The original oracle also takes resolution, confirmation, VSync and music
   # settings. These match the untouched Menu::new defaults in this fixture.
   defaults=(0,0,0,0,64)
   raw=subprocess.check_output([reference,font,args.original/'assets/background1.png',*map(str,(*state,*defaults))],env=env);REFERENCES[phase]=Image.frombytes('RGBA',(1280,720),raw).convert('RGB')
  def delta(actual,expected):return max(p[1] for p in ImageChops.difference(actual,expected).getextrema())
  for profile,suffix in [('core-v1','core'),('component-v1','component')]:
   game_id='com.gamepkg.menu-'+suffix
   for target in (['wasm','native'] if suffix=='core' else ['wasm']):
    run('build',game_id,'--target',target)
    for renderer in ['vulkan','opengl']:
     label=profile+'-'+target+'-'+renderer;app=subprocess.Popen([GUI,'--play',game_id,'--renderer',renderer],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);held=False
     try:
      deadline=time.monotonic()+20
      while True:
       if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
       found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
       if found.returncode==0:window=found.stdout.splitlines()[0];break
       assert time.monotonic()<deadline;time.sleep(.05)
      sizes=[(1280,960)]+([(640,480)] if (suffix,target,renderer)==('core','native','vulkan') else [])
      for width,height in sizes:
       subprocess.run(['xdotool','windowsize',window,str(width),str(height),'windowfocus',window],env=env,check=True);time.sleep(.15)
       for phase in PHASES:
        if held:subprocess.run(['xdotool','keyup',held],env=env,check=True);held=False
        if phase:held=KEYS[phase];subprocess.run(['xdotool','keydown',held],env=env,check=True)
        deadline=time.monotonic()+12
        while True:
         path=PROOF/(label+'-'+str(width)+'-'+str(phase)+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);im=Image.open(path).convert('RGB');assert im.size==(width,height),im.size
         actual=im.crop((0,height//8,width,height*7//8))
         if width==640:
          marker=im.getpixel((8,8));wanted={0:(255,0,0),1:(0,255,0),2:(0,0,255),3:(255,255,0),4:(255,0,255)}[phase]
          if marker==wanted:SMALL[phase]=actual.copy();break
         elif delta(actual,REFERENCES[phase])<=2:break
         if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
         assert time.monotonic()<deadline,(label,phase,delta(actual,REFERENCES[phase]) if width==1280 else marker);time.sleep(.05)
        if width==1280:print(label,phase,'complete implemented menu maximum difference',delta(actual,REFERENCES[phase]),flush=True)
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
     wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')==str(phase));raw=browser.execute_script('return Array.from(document.querySelector("#game").getContext("2d").getImageData(0,60,640,360).data);');actual=Image.frombytes('RGBA',(640,360),bytes(raw)).convert('RGB');difference=delta(actual,SMALL[phase]);actual.save(PROOF/(profile+'-web-'+str(phase)+'.png'));print(profile,phase,'implemented menu browser/native maximum difference',difference,flush=True);assert difference<=2,difference
   finally:
    if held:ActionChains(browser).key_up(held).perform()
    browser.quit();server.shutdown();server.server_close()
 finally:display.terminate();display.wait(timeout=10)
