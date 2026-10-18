"""Compare production bonus or rocket rendering with original SDL art in real players."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,math,os,shutil,subprocess,tempfile,threading,time,zlib
from PIL import Image
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--rocket',action='store_true');parser.add_argument('--skip-browser',action='store_true');parser.add_argument('--browser-only',action='store_true');args=parser.parse_args()
assert not(args.skip_browser and args.browser_only)
PROJECT=Path(__file__).resolve().parents[1];PROOF=PROJECT/('target/rocket-proof' if args.rocket else 'target/bonus-proof');PROOF.mkdir(parents=True,exist_ok=True)
REFERENCE=json.loads((PROJECT/'tests/fixtures/arkanoo-bonus.json').read_text());ORIGINAL=[bytes.fromhex(REFERENCE['original_dark_rgba'][str(kind)]) for kind in range(4)]
RELATIVE=[(1,2),(10,10),(0,10),(2,10),(7,7)]
TEX_W=TEX_H=41
if args.rocket:
 REFERENCE=json.loads((PROJECT/'tests/fixtures/arkanoo-rocket.json').read_text());pixels=bytes.fromhex(REFERENCE['rgba_hex']);background=[11,16,32]
 assert all(alpha in [0,255] for alpha in pixels[3::4])
 ORIGINAL=[bytes(value if pixels[index//4*4+3] else background[index%4] if index%4<3 else 255 for index,value in enumerate(pixels))]*4
 TEX_W=10;TEX_H=20;RELATIVE=[(0,3),(2.5,1),(2,4),(0,6),(4,6),(2,8),(2,9)]
BACKGROUND=zlib.decompress(bytes.fromhex(json.loads((PROJECT/'tests/fixtures/arkanoo-image-backgrounds.json').read_text())['cases'][0]['game_rgba_zlib_hex']))
SCENES={shift:[bytes.fromhex(value) for value in values] for shift,values in json.loads((PROJECT/'tests/fixtures/arkanoo-bonus-background.json').read_text())['scenes'].items()}
def background_pixel(x,y):
 at=(y*1280+x)*4;return list(BACKGROUND[at:at+3])
def original_pixel(kind,x,y,shift):
 if args.rocket:
  at=(y*TEX_W+x)*4
  return list(pixels[at:at+3]) if pixels[at+3] else background_pixel((200+kind*50+shift)*2+x,320+y)
 at=(y*TEX_W+x)*4;return list(SCENES[str(shift)][kind][at:at+3])
def background_choices(width,height,x,y):
 scale=min(width/640,height/480);ox=(width-640*scale)/2;oy=(height-480*scale)/2
 sx=(round(ox+x*scale)+.5-ox)*2/scale;sy=((round(oy+y*scale)+.5-oy)/scale-60)*2
 xs=[math.floor(sx)]+([round(sx)-1] if abs(sx-round(sx))<.00001 else []);ys=[math.floor(sy)]+([round(sy)-1] if abs(sy-round(sy))<.00001 else [])
 return [background_pixel(x,y) for x in xs for y in ys]
CLEARING=[((202 if args.rocket else 210)+kind*50,224 if args.rocket else 230) for kind in range(4)]
def check_clearing(actual,width,height):
 for pixel,(x,y) in zip(actual,CLEARING):assert min(max(abs(a-b) for a,b in zip(pixel,choice)) for choice in background_choices(width,height,x,y))<=1,('Stale sprite pixels',pixel,x,y)
def points(shift):return [(kind,200+kind*50+shift+x,220+y) for kind in range(4) for x,y in RELATIVE]
def expected(shift,width,height):
 scale=min(width/640,height/480);ox=(width-640*scale)/2;oy=(height-480*scale)/2;values=[]
 for kind,x,y in points(shift):
  px=round(ox+x*scale);py=round(oy+y*scale);left=ox+(200+kind*50+shift)*scale;top=oy+220*scale
  def texels(coordinate,extent):
   # Interpolated f32 UVs can fall on either side of an exact nearest boundary.
   value=math.floor(coordinate);choices=[value]
   if abs(coordinate-round(coordinate))<0.00001:choices.append(round(coordinate)-1)
   return sorted({max(0,min(extent-1,c)) for c in choices})
  values.append([original_pixel(kind,sx,sy,shift) for sx in texels((px+.5-left)*2/scale,TEX_W) for sy in texels((py+.5-top)*2/scale,TEX_H)])
 return values
def check(actual,wanted,label):
 deltas=[min(max(abs(a-b) for a,b in zip(p,q)) for q in choices) for p,choices in zip(actual,wanted)];delta=max(deltas);print(label,'maximum original SDL channel difference (nearest boundaries accounted):',delta,flush=True)
 assert delta<=3,(label,[(i,a,b) for i,(a,b) in enumerate(zip(actual,wanted)) if deltas[i]>3])
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-bonus-ui-',dir=PROJECT/'target') as directory:
 root=Path(directory);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 display=start_display(env,width=1100,height=850)
 try:
  source=root/'source';shutil.copytree(PROJECT/'ports/arkanoo',source);entry=source/'source/src/lib.rs';modules=entry.read_text().split('use std::sync::Mutex;')[0]
  entry.write_text(modules+'''pub fn game_tick(input:u32,_state:u64)->u64 {let shift=if input&gamepkg::HELD_RIGHT!=0 {20.0} else {0.0};let mut game=physics::Game::new();game.mode=1;game.blocks=[0;200];for kind in 0..4 {game.drops.push(physics::Drop{x:200.0+kind as f32*50.0+shift,y:220.0,kind});}render::draw(&game);u64::from(shift>0.0)}''')
  if args.rocket:entry.write_text(entry.read_text().replace('game.drops.push(physics::Drop{x:200.0+kind as f32*50.0+shift,y:220.0,kind});','game.rockets.push((200.0+kind as f32*50.0+shift,220.0));'))
  base=(source/'manifest.toml').read_text().replace('com.gamepkg.arkanoo','com.gamepkg.bonus-scene');cli=PROJECT/'target/release/gamepkg';gui=PROJECT/'target/release/gamepkg-gui'
  def run(*arguments):
   r=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr
  key=root/'test.key';run('keygen','-o',key)
  for profile in ['core-v1','component-v1']:
   game_id='com.gamepkg.bonus-scene'+('-component' if profile=='component-v1' else '');(source/'manifest.toml').write_text('runtime_profile="'+profile+'"\n'+base.replace('com.gamepkg.bonus-scene',game_id));package=root/(profile+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build',game_id,'--target','wasm')
   for target in ([] if args.browser_only else ['wasm','native'] if profile=='core-v1' else ['wasm']):
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
      for shift in [0,20,0]:
       if held:subprocess.run(['xdotool','keyup','Right'],env=env,check=True);held=False
       if shift:subprocess.run(['xdotool','keydown','Right'],env=env,check=True);held=True
       deadline=time.monotonic()+10
       while True:
        path=PROOF/(label+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);im=Image.open(path).convert('RGB');scale=min(im.width/640,im.height/480);ox=(im.width-640*scale)/2;oy=(im.height-480*scale)/2
        actual=[list(im.getpixel((round(ox+x*scale),round(oy+y*scale)))) for _,x,y in points(shift)]
        if all(min(max(abs(a-b) for a,b in zip(pixel,choice)) for choice in choices)<=3 for pixel,choices in zip(actual,expected(shift,im.width,im.height))):break
        if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
        assert time.monotonic()<deadline,(label,shift,actual);time.sleep(.05)
       check(actual,expected(shift,im.width,im.height),label+'-'+str(shift))
       if shift:check_clearing([im.getpixel((round(ox+x*scale),round(oy+y*scale))) for x,y in CLEARING],im.width,im.height)
     finally:
      if held:subprocess.run(['xdotool','keyup','Right'],env=env,check=True)
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
     for shift in [0,20,0]:
      if held:ActionChains(browser).key_up(Keys.ARROW_RIGHT).perform();held=False
      if shift:ActionChains(browser).key_down(Keys.ARROW_RIGHT).perform();held=True
      wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')==str(int(shift>0)))
      actual=browser.execute_script('const c=document.querySelector("#game").getContext("2d");return arguments[0].map(p=>Array.from(c.getImageData(p[1],p[2],1,1).data).slice(0,3));',points(shift));check(actual,expected(shift,640,480),'browser-'+profile+'-'+str(shift))
      if shift:check_clearing(browser.execute_script('const c=document.querySelector("#game").getContext("2d");return arguments[0].map(p=>Array.from(c.getImageData(p[0],p[1],1,1).data).slice(0,3));',CLEARING),640,480)
    finally:
     if held:ActionChains(browser).key_up(Keys.ARROW_RIGHT).perform()
     browser.quit();server.shutdown();server.server_close()
 finally:display.terminate();display.wait(timeout=10)
