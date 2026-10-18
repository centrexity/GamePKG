"""Compare game RGBA rectangles/sprites with real SDL, desktop and browser pixels."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,os,shutil,subprocess,tempfile,threading,time
from PIL import Image
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--baseline-core',action='store_true',help='Only measure core Wasm on Vulkan');parser.add_argument('--skip-browser',action='store_true',help='Run desktop checks without Chromium/Selenium');args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1];PROOF=PROJECT/'target/blending-proof';PROOF.mkdir(parents=True,exist_ok=True)
BACKGROUNDS=[(0,0,0,255),(11,16,32,255),(80,80,100,255),(255,255,255,255)]
COLORS=[(255,255,255,30),(0,200,255,200),(255,0,0,128),(40,150,220,80),(130,70,200,0),(14,92,167,255)]
def packed(color):return int.from_bytes(bytes(color),'big')
def sample_points():return [(56+col*100,47+row*112+kind*30) for row in range(4) for col in range(6) for kind in range(3)]
def samples(image):
 scale=min(image.width/640,image.height/480);ox=(image.width-640*scale)/2;oy=(image.height-480*scale)/2
 return [list(image.getpixel((round(ox+x*scale),round(oy+y*scale))))[:3] for x,y in sample_points()]
def compare(actual,expected,label):
 flat=[sample for cell in expected for sample in cell]
 delta=max(abs(a-b) for p,q in zip(actual,flat) for a,b in zip(p,q))
 assert len(actual)==len(flat)
 print(label,'maximum 8-bit channel difference:',delta,flush=True)
 assert delta<=2,(label,[(i,a,e) for i,(a,e) in enumerate(zip(actual,flat)) if max(abs(x-y) for x,y in zip(a,e))>2][:6])
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-blending-',dir=PROJECT/'target') as temporary:
 root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 display=start_display(env,width=1100,height=850)
 try:
  flags=subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split();reference=root/'reference'
  subprocess.run(['cc',str(PROJECT/'tests/reference/sdl_blending.c'),'-o',str(reference),*flags],check=True)
  original=json.loads(subprocess.check_output([str(reference),'0'],env=env));software=json.loads(subprocess.check_output([str(reference),'1'],env=env));print('SDL reference:',original['driver'],'/ software:',software['driver'],flush=True)
  report={'sdl':original,'sdl_software':software,'desktop':{},'browser':{}}
  source=root/'source';shutil.copytree(PROJECT/'examples/minimal',source)
  for index,color in enumerate(COLORS):Image.new('RGBA',(1,1),color).save(source/'assets'/f'c{index}.png')
  game='''pub fn game_tick(_input:u32,_state:u64)->u64 {
 gamepkg::draw_clear(0x10171dff);gamepkg::draw_rect(4.0,4.0,16.0,16.0,0xff00ffff);
 let backgrounds=['''+','.join(hex(packed(c)) for c in BACKGROUNDS)+'''];let colors=['''+','.join(hex(packed(c)) for c in COLORS)+'''];let paths=["c0.png","c1.png","c2.png","c3.png","c4.png","c5.png"];
 for row in 0..4 {for col in 0..6 {let x=16.0+col as f32*100.0;let y=32.0+row as f32*112.0;
 gamepkg::draw_rect(x,y,80.0,90.0,backgrounds[row]);gamepkg::draw_rect(x,y,80.0,30.0,colors[col]);
 gamepkg::draw_asset_image(paths[col],x,y+30.0,80.0,30.0);gamepkg::draw_rect(x,y+60.0,80.0,30.0,colors[col]);gamepkg::draw_asset_image_linear(paths[(col+1)%6],x,y+60.0,80.0,30.0);
 }}1}
'''
  (source/'source/src/lib.rs').write_text(game);manifest=(source/'manifest.toml').read_text().replace('com.gamepkg.minimal','com.gamepkg.blending-probe')
  cli=PROJECT/'target/release/gamepkg';gui=PROJECT/'target/release/gamepkg-gui'
  def run(*arguments):
   result=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stderr;return result
  key=root/'test.key';run('keygen','-o',key)
  profiles=['core-v1'] if args.baseline_core else ['core-v1','component-v1']
  for profile in profiles:
   game_id='com.gamepkg.blending-probe'+('-component' if profile=='component-v1' else '')
   (source/'manifest.toml').write_text(f'runtime_profile="{profile}"\n'+manifest.replace('com.gamepkg.blending-probe',game_id))
   package=root/(profile+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build',game_id,'--target','wasm')
   targets=['wasm','native'] if profile=='core-v1' and not args.baseline_core else ['wasm']
   for target in targets:
    if target=='native':run('build',game_id,'--target','native')
    for renderer in (['vulkan'] if args.baseline_core else ['vulkan','opengl']):
     app=subprocess.Popen([str(gui),'--play',game_id,'--renderer',renderer],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
     try:
      deadline=time.monotonic()+20;window=None;image=None
      while True:
       if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
       found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
       if found.returncode==0:
        window=found.stdout.splitlines()[0];path=PROOF/(profile+'-'+target+'-'+renderer+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);image=Image.open(path).convert('RGB')
        scale=min(image.width/640,image.height/480);ox=(image.width-640*scale)/2;oy=(image.height-480*scale)/2
        if image.getpixel((round(ox+12*scale),round(oy+12*scale)))==(255,0,255):break
       assert time.monotonic()<deadline,'Game frame did not become ready';time.sleep(.05)
      actual=samples(image);label=profile+'-'+target+'-'+renderer;report['desktop'][label]=actual;(PROOF/'results.json').write_text(json.dumps(report,indent=2));compare(actual,original['samples'],label)
     finally:
      if app.poll() is None:app.terminate()
      app.communicate(timeout=10)
   if not args.baseline_core and not args.skip_browser:
    from selenium import webdriver
    from selenium.webdriver.chrome.options import Options
    from selenium.webdriver.chrome.service import Service
    from selenium.webdriver.support.ui import WebDriverWait
    output=root/(profile+'-web');run('web-export',game_id,'--output',output)
    server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();options.binary_location='/usr/bin/chromium'
    for value in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader']:options.add_argument(value)
    browser=webdriver.Chrome(service=Service('/usr/bin/chromedriver'),options=options)
    try:
     browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click()
     wait.until(lambda b:b.find_element('id','game').get_attribute('data-state'))
     actual=browser.execute_script('const c=document.querySelector("#game").getContext("2d");return arguments[0].map(p=>Array.from(c.getImageData(p[0],p[1],1,1).data).slice(0,3));',sample_points())
     report['browser'][profile]=actual;(PROOF/'results.json').write_text(json.dumps(report,indent=2));compare(actual,original['samples'],'browser-'+profile)
    finally:browser.quit();server.shutdown();server.server_close()
 finally:
  display.terminate();display.wait(timeout=10)
