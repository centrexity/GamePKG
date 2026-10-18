"""Verify original campaign backgrounds in native/core/component desktop and browser players."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,math,os,shutil,subprocess,tempfile,threading,time
from PIL import Image
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--fire-sea',action='store_true');parser.add_argument('--procedural',action='store_true');parser.add_argument('--skip-browser',action='store_true');args=parser.parse_args();assert not(args.procedural and args.fire_sea);args.procedural=args.procedural or args.fire_sea
PROJECT=Path(__file__).resolve().parents[1];PROOF=PROJECT/('target/fire-sea-proof' if args.fire_sea else 'target/procedural-proof' if args.procedural else 'target/background-proof');PROOF.mkdir(parents=True,exist_ok=True)
import zlib
REFERENCE=json.loads((PROJECT/('tests/fixtures/arkanoo-fire-sea-background.json' if args.fire_sea else 'tests/fixtures/arkanoo-procedural.json' if args.procedural else 'tests/fixtures/arkanoo-background.json')).read_text())['cases']
ORIGINAL=[zlib.decompress(bytes.fromhex(case['sdl_rgba_zlib_hex'])) for case in REFERENCE]
PHASES=[0,3,7,10,14,17,0]
KEYS={3:'Right',7:'Up',10:'z',14:'Left',17:'Down'}
if args.procedural:PHASES=[0,3,11,14,22,25,0];KEYS={3:'Right',11:'Up',14:'z',22:'Left',25:'Down'}
if args.fire_sea:PHASES=[0,3,11,14,0]
def source_pixel(case,x,y):
 if 0<=x<1280 and 0<=y<720:
  at=(y*1280+x)*4;return list(ORIGINAL[case][at:at+3])
 return [11,16,32]
from functools import lru_cache
@lru_cache(maxsize=None)
def points(case):
 raw=ORIGINAL[case];bg=REFERENCE[case]['background'].to_bytes(4,'big')[:3]
 visible=[i for i in range(1280*720) if raw[i*4:i*4+3]!=bg]
 import struct
 if args.procedural:left,top,_,_=REFERENCE[case]['bounds']
 else:
  dots=[(x,y) for x,y,color in struct.iter_unpack('<iiI',zlib.decompress(bytes.fromhex(REFERENCE[case]['trace_zlib_hex']))) if color&255 and 0<=x<1280 and 0<=y<720]
  left=min(x for x,y in dots);top=min(y for x,y in dots)
 # Sample inside geometry tiles; clipping/raster edges have their own open audit.
 visible=[i for i in visible if 5<i%1280<1275 and 5<i//1280<715 and 4<((i%1280-left)%64)<60 and 4<((i//1280-top)%64)<60]
 assert visible
 positions=[(i%1280,i//1280) for i in visible[::max(1,len(visible)//64)]]
 positions += [(640,360),(480,200),(800,520)]
 return [(0,x*.5,60+y*.5) for x,y in positions]
def expected(case,width,height):
 scale=min(width/640,height/480);ox=(width-640*scale)/2;oy=(height-480*scale)/2;values=[]
 for _,x,y in points(case):
  px=round(ox+x*scale);py=round(oy+y*scale)
  def texels(value):
   choices=[math.floor(value)]
   if abs(value-round(value))<0.00001:choices.append(round(value)-1)
   return choices
  values.append([source_pixel(case,sx,sy) for sx in texels((px+.5-ox)*2/scale) for sy in texels(((py+.5-oy)/scale-60)*2)])
 return values
def check(actual,wanted,label):
 deltas=[min(max(abs(a-b) for a,b in zip(p,q)) for q in choices) for p,choices in zip(actual,wanted)];delta=max(deltas);print(label,'maximum original SDL channel difference (nearest boundaries accounted):',delta,flush=True)
 assert delta<=(5 if args.procedural else 3),(label,[(i,a,b) for i,(a,b) in enumerate(zip(actual,wanted)) if deltas[i]>(5 if args.procedural else 3)])
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-background-ui-',dir=PROJECT/'target') as directory:
 root=Path(directory);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 display=start_display(env,width=1100,height=850)
 try:
  source=root/'source';shutil.copytree(PROJECT/'ports/arkanoo',source);entry=source/'source/src/lib.rs';modules=entry.read_text().split('use std::sync::Mutex;')[0]
  entry.write_text(modules+'pub fn game_tick(input:u32,_:u64)->u64 {let which=if input&gamepkg::HELD_RIGHT!=0 {3} else if input&gamepkg::HELD_UP!=0 {7} else if input&gamepkg::HELD_B!=0 {10} else if input&gamepkg::HELD_LEFT!=0 {14} else if input&gamepkg::HELD_DOWN!=0 {17} else {0};let cases='+str([[c['level'],c['frame']] for c in REFERENCE]).replace(' ', '')+';gamepkg::draw_clear(0x0b1020ff);render::draw_background(cases[which][0] as u32,cases[which][1]);which as u64}')
  if args.procedural:entry.write_text(entry.read_text().replace('{7} else','{11} else').replace('{10} else','{14} else').replace('{14} else if input&gamepkg::HELD_DOWN','{22} else if input&gamepkg::HELD_DOWN').replace('{17} else','{25} else'))
  base=(source/'manifest.toml').read_text().replace('com.gamepkg.arkanoo','com.gamepkg.background-scene');cli=PROJECT/'target/release/gamepkg';gui=PROJECT/'target/release/gamepkg-gui'
  def run(*arguments):
   r=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr
  key=root/'test.key';run('keygen','-o',key)
  for profile in ['core-v1','component-v1']:
   game_id='com.gamepkg.background-scene'+('-component' if profile=='component-v1' else '');(source/'manifest.toml').write_text('runtime_profile="'+profile+'"\n'+base.replace('com.gamepkg.background-scene',game_id));package=root/(profile+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build',game_id,'--target','wasm')
   for target in (['wasm','native'] if profile=='core-v1' else ['wasm']):
    if target=='native':run('build',game_id,'--target','native')
    for renderer in ['vulkan','opengl']:
     label=profile+'-'+target+'-'+renderer;app=subprocess.Popen([str(gui),'--play',game_id,'--renderer',renderer],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);held=False
     try:
      deadline=time.monotonic()+20
      while True:
       if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
       found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
       if found.returncode==0:window=found.stdout.splitlines()[0];break
       assert time.monotonic()<deadline;time.sleep(.05)
      subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
      for shift in PHASES:
       if held:subprocess.run(['xdotool','keyup',held],env=env,check=True);held=False
       if shift:held=KEYS[shift];subprocess.run(['xdotool','keydown',held],env=env,check=True)
       deadline=time.monotonic()+10
       while True:
        path=PROOF/(label+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);im=Image.open(path).convert('RGB');scale=min(im.width/640,im.height/480);ox=(im.width-640*scale)/2;oy=(im.height-480*scale)/2
        actual=[list(im.getpixel((round(ox+x*scale),round(oy+y*scale)))) for _,x,y in points(shift)]
        if all(min(max(abs(a-b) for a,b in zip(pixel,choice)) for choice in choices)<=(5 if args.procedural else 3) for pixel,choices in zip(actual,expected(shift,im.width,im.height))):break
        if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
        assert time.monotonic()<deadline,(label,shift,actual);time.sleep(.05)
       check(actual,expected(shift,im.width,im.height),label+'-'+str(shift))
     finally:
      if held:subprocess.run(['xdotool','keyup',held],env=env,check=True)
      if app.poll() is None:app.terminate()
      app.communicate(timeout=10)
   if not args.skip_browser:
    from selenium import webdriver
    from selenium.webdriver.chrome.options import Options
    from selenium.webdriver.chrome.service import Service
    from selenium.webdriver.support.ui import WebDriverWait
    from selenium.webdriver.common.action_chains import ActionChains
    from selenium.webdriver.common.keys import Keys
    output=root/(profile+'-web');run('web-export',game_id,'--output',output);server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();chrome=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert chrome,'Install Chrome or Chromium for browser checks';options.binary_location=chrome
    for value in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader']:options.add_argument(value)
    driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');assert Path(driver).is_file(),'Install matching ChromeDriver'
    browser=webdriver.Chrome(service=Service(driver),options=options);held=False
    try:
     browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();wait.until(lambda b:b.find_element('id','game').get_attribute('data-state'));browser.find_element('id','game').click()
     for shift in PHASES:
      if held:ActionChains(browser).key_up(held).perform();held=False
      if shift:held=({3:Keys.ARROW_RIGHT,11:Keys.ARROW_UP,14:'z',22:Keys.ARROW_LEFT,25:Keys.ARROW_DOWN} if args.procedural else {3:Keys.ARROW_RIGHT,7:Keys.ARROW_UP,10:'z',14:Keys.ARROW_LEFT,17:Keys.ARROW_DOWN})[shift];ActionChains(browser).key_down(held).perform()
      wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')==str(shift))
      actual=browser.execute_script('const c=document.querySelector("#game").getContext("2d");return arguments[0].map(p=>Array.from(c.getImageData(p[1],p[2],1,1).data).slice(0,3));',[(kind,round(x),round(y)) for kind,x,y in points(shift)]);check(actual,expected(shift,640,480),'browser-'+profile+'-'+str(shift))
    finally:
     if held:ActionChains(browser).key_up(held).perform()
     browser.quit();server.shutdown();server.server_close()
 finally:display.terminate();display.wait(timeout=10)
