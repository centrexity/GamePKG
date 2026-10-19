"""Verify fresh-frame backgrounds and disappearing geometry in native/browser players."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import argparse, os, shutil, subprocess, tempfile, threading, time
from PIL import Image
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--browser-only',action='store_true');parser.add_argument('--skip-browser',action='store_true');args=parser.parse_args()
assert not(args.browser_only and args.skip_browser)
PROJECT=Path(__file__).resolve().parents[1];PROOF=PROJECT/'target/frame-reset-proof';PROOF.mkdir(parents=True,exist_ok=True)
POINTS=[(12,12),(100,100),(300,240),(580,430)]
MARKERS=[(255,0,255),(0,255,0),(0,0,255),(0,255,255),(16,23,29)]
COLORS=[(200,48,32),(16,23,29),(64,32,16),(0,0,0),(16,23,29)]
SEQUENCE=[0,1,2,3,4,2,1,0]
def check(samples,phase,label):
 expected=[MARKERS[phase],(255,255,255) if phase==1 else COLORS[phase],COLORS[phase],COLORS[phase]]
 delta=max(abs(a-b) for actual,wanted in zip(samples,expected) for a,b in zip(actual,wanted))
 assert delta<=1,(label,phase,samples,expected)
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-frame-reset-',dir=PROJECT/'target') as directory:
 root=Path(directory);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 display=start_display(env,width=1100,height=850)
 try:
  source=root/'source';shutil.copytree(PROJECT/'examples/minimal',source)
  (source/'source/src/lib.rs').write_text('''pub fn game_tick(input:u32,_state:u64)->u64 {
 let phase=if input&gamepkg::HELD_DOWN!=0 {4u32} else if input&gamepkg::HELD_UP!=0 {3} else if input&gamepkg::HELD_LEFT!=0 {2} else if input&gamepkg::HELD_RIGHT!=0 {1} else {0};
 if phase==0 {gamepkg::draw_clear(0xc83020ff);} if phase==2 {gamepkg::draw_clear(0x80402080);} if phase==3 {gamepkg::draw_clear(0x00000000);}
 if phase<4 {let marker=if phase==0 {0xff00ffff} else if phase==1 {0x00ff00ff} else if phase==2 {0x0000ffff} else {0x00ffffff};gamepkg::draw_rect(4.0,4.0,16.0,16.0,marker);}
 if phase==1 {gamepkg::draw_rect(80.0,80.0,40.0,40.0,0xffffffff);}u64::from(phase)}''')
  base=(source/'manifest.toml').read_text().replace('com.gamepkg.minimal','com.gamepkg.frame-reset');cli=PROJECT/'target/release/gamepkg';gui=PROJECT/'target/release/gamepkg-gui'
  def run(*arguments):
   result=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stderr
  key=root/'test.key';run('keygen','-o',key)
  for profile in ['core-v1','component-v1']:
   game_id='com.gamepkg.frame-reset'+('-component' if profile=='component-v1' else '');(source/'manifest.toml').write_text('runtime_profile="'+profile+'"\n'+base.replace('com.gamepkg.frame-reset',game_id));package=root/(profile+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build',game_id,'--target','wasm')
   if not args.browser_only:
    for target in (['wasm','native'] if profile=='core-v1' else ['wasm']):
     if target=='native':run('build',game_id,'--target','native')
     for renderer in ['vulkan','opengl']:
      app=subprocess.Popen([str(gui),'--play',game_id,'--renderer',renderer],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);active=None;label=profile+'-'+target+'-'+renderer
      try:
       deadline=time.monotonic()+20
       while True:
        if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
        found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
        if found.returncode==0:window=found.stdout.splitlines()[0];break
        assert time.monotonic()<deadline;time.sleep(.05)
       subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
       def capture():
        path=PROOF/(label+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);im=Image.open(path).convert('RGB');scale=min(im.width/640,im.height/480);ox=(im.width-640*scale)/2;oy=(im.height-480*scale)/2
        return [im.getpixel((round(ox+x*scale),round(oy+y*scale))) for x,y in POINTS]
       for phase in SEQUENCE:
        key_name=[None,'Right','Left','Up','Down'][phase]
        if active:subprocess.run(['xdotool','keyup',active],env=env,check=True)
        active=key_name
        if active:subprocess.run(['xdotool','keydown',active],env=env,check=True)
        deadline=time.monotonic()+10
        while True:
         samples=capture()
         if samples[0]==MARKERS[phase]:break
         if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
         assert time.monotonic()<deadline,(label,phase,samples);time.sleep(.05)
        check(samples,phase,label);time.sleep(.25);check(capture(),phase,label+'-stable')
       print(label,'fresh/absent/translucent/transparent/empty frames and revisits passed',flush=True)
      finally:
       if active:subprocess.run(['xdotool','keyup',active],env=env,check=True)
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
    driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');assert Path(driver).is_file(),'Install the matching ChromeDriver for browser checks'
    browser=webdriver.Chrome(service=Service(driver),options=options);active=None
    try:
     browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();wait.until(lambda b:b.find_element('id','game').get_attribute('data-state'));browser.find_element('id','game').click()
     def pixels():return browser.execute_script('const c=document.querySelector("#game").getContext("2d");return arguments[0].map(p=>Array.from(c.getImageData(p[0],p[1],1,1).data));',POINTS)
     for phase in SEQUENCE:
      if active:ActionChains(browser).key_up(active).perform()
      active=[None,Keys.ARROW_RIGHT,Keys.ARROW_LEFT,Keys.ARROW_UP,Keys.ARROW_DOWN][phase]
      if active:ActionChains(browser).key_down(active).perform()
      wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')==str(phase));samples=pixels();check([s[:3] for s in samples],phase,'browser-'+profile);assert all(s[3]==255 for s in samples),samples
      time.sleep(.25);samples=pixels();check([s[:3] for s in samples],phase,'browser-'+profile+'-stable');assert all(s[3]==255 for s in samples),samples
     print('browser-'+profile,'fresh/absent/translucent/transparent/empty frames and revisits passed',flush=True)
    finally:
     if active:ActionChains(browser).key_up(active).perform()
     browser.quit();server.shutdown();server.server_close()
 finally:display.terminate();display.wait(timeout=10)
