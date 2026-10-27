"""Verify original portal stages in native/core/component desktop and browser players."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,math,os,shutil,subprocess,tempfile,threading,time
from PIL import Image
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--skip-browser',action='store_true');parser.add_argument('--candidate',type=Path);parser.add_argument('--tight-memory',action='store_true',help='Generated geometry only: retain the former 16/3/7 MiB partition without unrelated packaged media');parser.add_argument('--proof-output',type=Path);args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1];PROOF=args.proof_output or PROJECT/'target/portal-proof';PROOF.mkdir(parents=True,exist_ok=True)
import zlib
REFERENCE=json.loads((PROJECT/'tests/fixtures/arkanoo-portal.json').read_text())['cases']
ORIGINAL=[zlib.decompress(bytes.fromhex(case['sdl_dark_rgba_zlib_hex'])) for case in REFERENCE]
PHASES=[0,4,8,9,11,6,0]
KEYS={4:'Right',8:'Up',9:'z',11:'Left',6:'Down'}
def source_pixel(case,x,y):
 bounds=REFERENCE[case]['bounds']
 if bounds:
  left,top,w,h=bounds
  if left<=x<left+w and top<=y<top+h:
   at=((y-top)*w+x-left)*4;return list(ORIGINAL[case][at:at+3])
 return [11,16,32]
def points(case):
 selected=case if REFERENCE[case]['bounds'] else 0
 left,top,w,h=REFERENCE[selected]['bounds'];raw=ORIGINAL[selected]
 visible=[i for i in range(w*h) if raw[i*4:i*4+3]!=bytes([11,16,32])]
 positions=[(left+i%w,top+i//w) for i in visible[::max(1,len(visible)//64)]]
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
 assert delta<=5,(label,[(i,a,b) for i,(a,b) in enumerate(zip(actual,wanted)) if deltas[i]>5])
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-portal-ui-',dir=PROJECT/'target') as directory:
 root=Path(directory);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 display=start_display(env,width=1100,height=850)
 try:
  source=root/'source';shutil.copytree(args.candidate or PROJECT/'ports/arkanoo',source);entry=source/'source/src/lib.rs';modules=entry.read_text().split('use std::sync::Mutex;')[0]
  entry.write_text(modules+'pub fn game_tick(input:u32,_:u64)->u64 {let which=if input&gamepkg::HELD_B!=0 {9} else if input&gamepkg::HELD_RIGHT!=0 {4} else if input&gamepkg::HELD_UP!=0 {8} else if input&gamepkg::HELD_LEFT!=0 {11} else if input&gamepkg::HELD_DOWN!=0 {6} else {0};let cases='+str([[c['frame'],c['timer']] for c in REFERENCE[:12]]).replace(' ', '')+';gamepkg::draw_clear(0x0b1020ff);render::draw_portal(cases[which][0],cases[which][1]);which as u64}')
  if args.tight_memory:
   # This extra fixture runs only generated portal geometry. The default gate
   # separately retains the complete playable recipe and every packaged asset.
   # Keep the old six-MiB guest allowance inside its16/3/7MiB signed shares.
   import tomllib
   manifest=source/'manifest.toml';value=tomllib.loads(manifest.read_text());limits=value['limits']
   assert limits['effects_memory_bytes']==3145728 and limits['music_memory_bytes']==7340032
   lines=[line for line in manifest.read_text().splitlines() if not line.startswith('render_profile')]
   lines=['hard_memory_bytes = 16777216' if line.startswith('hard_memory_bytes') else 'recommended_memory_bytes = 16777216' if line.startswith('recommended_memory_bytes') else line for line in lines]
   manifest.write_text('\n'.join(lines)+'\n');shutil.rmtree(source/'assets');(source/'assets').mkdir();(source/'assets/fixture.txt').write_text('Private generated-geometry memory regression; full playable assets are checked separately.\n')
  base=(source/'manifest.toml').read_text().replace('com.gamepkg.arkanoo','com.gamepkg.portal-scene');cli=PROJECT/'target/release/gamepkg';gui=PROJECT/'target/release/gamepkg-gui'
  def run(*arguments):
   r=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr
  key=root/'test.key';run('keygen','-o',key)
  for profile in ['core-v1','component-v1']:
   game_id='com.gamepkg.portal-scene'+('-component' if profile=='component-v1' else '');(source/'manifest.toml').write_text('runtime_profile="'+profile+'"\n'+base.replace('com.gamepkg.portal-scene',game_id));package=root/(profile+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build',game_id,'--target','wasm')
   for target in (['wasm','native'] if profile=='core-v1' else ['wasm']):
    if target=='native':run('build',game_id,'--target','native')
    for renderer in ['vulkan','opengl']:
     label=profile+'-'+target+'-'+renderer;app=subprocess.Popen([str(gui),'--play',game_id,'--target',target,'--renderer',renderer],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);held=False;shift=None;failure=None
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
        if all(min(max(abs(a-b) for a,b in zip(pixel,choice)) for choice in choices)<=5 for pixel,choices in zip(actual,expected(shift,im.width,im.height))):break
        if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
        assert time.monotonic()<deadline,(label,shift,actual);time.sleep(.05)
       check(actual,expected(shift,im.width,im.height),label+'-'+str(shift))
     except Exception as error:
      failure={'label':label,'phase':shift,'error':str(error)[-4096:],'exit_before_stop':app.poll()}
      try:
       path=PROOF/(label+'-failure.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10)
       root_capture=PROOF/(label+'-failure-display.png');subprocess.run(['import','-display',env['DISPLAY'],'-window','root',str(root_capture)],env=env,check=True,timeout=10)
       if shutil.which('tesseract'):
        ocr=subprocess.run(['tesseract',str(root_capture),'stdout','--psm','11'],env=env,capture_output=True,text=True,timeout=10);failure['private_display_text']=ocr.stdout[-4096:]
       failure['window_title']=subprocess.check_output(['xdotool','getwindowname',window],env=env,text=True,timeout=5)[:512]
       failure['geometry']=subprocess.check_output(['xwininfo','-id',window],env=env,text=True,timeout=5)[-2048:]
       if shutil.which('tesseract'):
        ocr=subprocess.run(['tesseract',str(path),'stdout','--psm','11'],env=env,capture_output=True,text=True,timeout=10);failure['visible_text']=ocr.stdout[-4096:]
      except Exception as diagnostic:failure['diagnostic_error']=str(diagnostic)[-1024:]
      raise
     finally:
      if held:subprocess.run(['xdotool','keyup',held],env=env,check=True)
      if app.poll() is None:app.terminate()
      stdout,stderr=app.communicate(timeout=10)
      (PROOF/(label+'-stdout.log')).write_bytes(stdout[-8192:]);(PROOF/(label+'-stderr.log')).write_bytes(stderr[-8192:])
      if failure is not None:
       failure['exit_after_stop']=app.returncode;(PROOF/(label+'-failure.json')).write_text(json.dumps(failure,indent=2)+'\n')
   if not args.skip_browser:
    from selenium import webdriver
    from selenium.webdriver.chrome.options import Options
    from selenium.webdriver.chrome.service import Service
    from selenium.webdriver.support.ui import WebDriverWait
    from selenium.webdriver.common.action_chains import ActionChains
    from selenium.webdriver.common.keys import Keys
    output=root/(profile+'-web');run('web-export',game_id,'--output',output);server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();chrome=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert chrome,'Install Chrome or Chromium for browser checks';options.binary_location=chrome
    for value in ['--headless=new','--mute-audio','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader']:options.add_argument(value)
    driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');assert Path(driver).is_file(),'Install matching ChromeDriver'
    browser=webdriver.Chrome(service=Service(driver),options=options);held=False
    try:
     browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();wait.until(lambda b:b.find_element('id','game').get_attribute('data-state'));browser.find_element('id','game').click()
     for shift in PHASES:
      if held:ActionChains(browser).key_up(held).perform();held=False
      if shift:held={4:Keys.ARROW_RIGHT,8:Keys.ARROW_UP,9:'z',11:Keys.ARROW_LEFT,6:Keys.ARROW_DOWN}[shift];ActionChains(browser).key_down(held).perform()
      wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')==str(shift))
      actual=browser.execute_script('const c=document.querySelector("#game").getContext("2d");return arguments[0].map(p=>Array.from(c.getImageData(p[1],p[2],1,1).data).slice(0,3));',[(kind,round(x),round(y)) for kind,x,y in points(shift)]);check(actual,expected(shift,640,480),'browser-'+profile+'-'+str(shift))
    finally:
     if held:ActionChains(browser).key_up(held).perform()
     browser.quit();server.shutdown();server.server_close()
 finally:display.terminate();display.wait(timeout=10)
