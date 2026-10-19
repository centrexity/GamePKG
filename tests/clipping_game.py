"""Signed clip bindings, per-frame reset and actual six-desktop/two-browser rendering."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,os,shutil,subprocess,tempfile,threading,time
from PIL import Image
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--runtime-only',action='store_true');args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg';GUI=PROJECT/'target/release/gamepkg-gui'
compiled=subprocess.run(['cargo','test','--release','--test','drawing_clip_runtime','--no-run','--message-format=json'],capture_output=True,text=True,timeout=300);assert compiled.returncode==0,compiled.stderr
executables=[v['executable'] for line in compiled.stdout.splitlines() if (v:=json.loads(line)).get('reason')=='compiler-artifact' and v.get('target',{}).get('name')=='drawing_clip_runtime' and v.get('executable')];assert len(executables)==1
PROOF=PROJECT/'target/clipping-proof';PROOF.mkdir(parents=True,exist_ok=True)
POINTS=[(70,100),(100,100),(280,100),(340,100),(300,210),(100,210),(400,150),(350,150),(510,360),(30,410),(200,160),(150,170),(330,180),(600,450)]
BG=[16,32,48];DEFAULT=[16,23,29];WHITE=[255,255,255];GREEN=[0,255,0];BLUE=[0,0,255]
def expected(phase):
 result=[BG[:] for _ in POINTS]
 if phase==0:
  for i,color in {1:[255,0,0],2:[255,192,0],4:GREEN,5:BLUE,6:[153,0,255],9:WHITE,10:[255,0,0],11:BLUE}.items():result[i]=color
 elif phase==1:result[4]=BLUE;result[10]=BLUE
 elif phase==2:result[9]=WHITE
 elif phase in [3,4]:
  result=[(DEFAULT if phase==4 else BG)[:] for _ in POINTS];result[8]=GREEN;result[9]=WHITE
 return result
def check(actual,phase,label):
 delta=max(abs(a-b) for pixel,wanted in zip(actual,expected(phase)) for a,b in zip(pixel,wanted));print(label,phase,'maximum channel difference',delta,flush=True);assert delta<=1,(label,phase,actual,expected(phase))
PHASES=[0,1,4,2,3,0];KEYS={1:'Right',2:'Up',3:'Down',4:'Left'}
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-clipping-') as temporary:
 root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 def run(*a):
  r=subprocess.run([str(CLI),*map(str,a)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr
 key=root/'key';key.write_text('54'*32)
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  source=root/suffix;shutil.copytree(PROJECT/'examples/minimal',source);(source/'source/src/lib.rs').write_text((PROJECT/'tests/fixtures/drawing-clip.rs').read_text());(source/'shaders/main.slang').write_text('[shader("fragment")]\nfloat4 fragmentMain():SV_Target{return float4(0,0,1,1);}\n');(source/'assets').mkdir(exist_ok=True);Image.new('RGBA',(8,8),(255,192,0,255)).save(source/'assets/gold.png')
  manifest=source/'manifest.toml';manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.minimal','com.gamepkg.clip-'+suffix));package=root/(suffix+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.gamepkg.clip-'+suffix,'--target','wasm')
 probe_env=dict(env,GAMEPKG_CLIP_TEST_ROOT=str(root/'library'))
 subprocess.run([executables[0],'--ignored'],env=probe_env,check=True,timeout=120)
 run('build','com.gamepkg.clip-core','--target','native');subprocess.run([executables[0],'--ignored'],env=probe_env,check=True,timeout=120)
 if args.runtime_only:raise SystemExit(0)
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
        if all(all(abs(a-b)<=1 for a,b in zip(pixel,wanted)) for pixel,wanted in zip(actual,expected(phase))):break
        if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
        assert time.monotonic()<deadline,(label,phase,actual,expected(phase));time.sleep(.05)
       check(actual,phase,label)
      # Resize the actual client window. Require its captured dimensions before
      # checking logical samples, so an old pre-resize frame cannot pass.
      for width,height in [(960,720),(853,641),(700,800)]:
       subprocess.run(['xdotool','windowsize',window,str(width),str(height)],env=env,check=True)
       deadline=time.monotonic()+12
       while True:
        path=PROOF/(label+'-resize-'+str(width)+'x'+str(height)+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);im=Image.open(path).convert('RGB')
        scale=min(im.width/640,im.height/480);ox=(im.width-640*scale)/2;oy=(im.height-480*scale)/2;actual=[list(im.getpixel((round(ox+x*scale),round(oy+y*scale)))) for x,y in POINTS]
        if im.size==(width,height) and all(all(abs(a-b)<=1 for a,b in zip(pixel,wanted)) for pixel,wanted in zip(actual,expected(0))):break
        if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
        assert time.monotonic()<deadline,(label,im.size,(width,height),actual,expected(0));time.sleep(.05)
       check(actual,0,label+' resized '+str(width)+'x'+str(height))
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
   for value in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader']:options.add_argument(value)
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
