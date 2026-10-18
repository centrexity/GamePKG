"""Compare opaque linear sampling and nearest controls with SDL at real pixel sizes."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,os,shutil,subprocess,tempfile,threading,time,math
from PIL import Image
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--baseline-core',action='store_true');parser.add_argument('--renderer',choices=['vulkan','opengl']);parser.add_argument('--skip-browser',action='store_true');args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1];PROOF=PROJECT/'target/filtering-proof';PROOF.mkdir(parents=True,exist_ok=True)
POINTS=[(100+image*280+col*40,150+row*90) for image in range(2) for row in range(3) for col in range(5)]+[(120+image*280+col*120,50) for image in range(2) for col in range(2)]+[(x,440) for x in [0,20,60,100,140,240,260,280,300,320,340,360]]
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-filtering-',dir=PROJECT/'target') as temporary:
 root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 display=start_display(env,width=1100,height=850)
 try:
  flags=subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split();reference=root/'reference'
  subprocess.run(['cc',str(PROJECT/'tests/reference/sdl_filtering.c'),'-o',str(reference),*flags,'-lm'],check=True)
  references={};report={'sdl':{},'desktop':{},'browser':{}}
  def expected(width,height,shift=0,swap=False):
   key=(width,height,shift,swap)
   if key not in references:
    output=subprocess.check_output([str(reference),str(width),str(height),str(shift),str(int(swap))],env=env,text=True).splitlines();records=[json.loads(line) for line in output if line.startswith('{')];assert len(records)==1,output;references[key]=records[0];report['sdl'][f'{width}x{height}:{shift}:{swap}']=references[key]
    for line in output:
     if not line.startswith('{'):print('SDL diagnostic:',line,flush=True)
   return references[key]['samples']
  def compare(actual,reference,label):
   assert len(actual)==len(reference);delta=max(abs(a-b) for p,q in zip(actual,reference) for a,b in zip(p,q));print(label,'maximum 8-bit channel difference:',delta,flush=True)
   (PROOF/'results.json').write_text(json.dumps(report,indent=2));assert delta<=2,(label,[(i,a,e) for i,(a,e) in enumerate(zip(actual,reference)) if max(abs(x-y) for x,y in zip(a,e))>2][:6])
  source=root/'source';shutil.copytree(PROJECT/'examples/minimal',source)
  Image.frombytes('RGBA',(2,1),bytes([0,0,0,255,255,255,255,255])).save(source/'assets/gray.png')
  Image.frombytes('RGBA',(2,2),bytes([255,0,0,255,0,255,0,255,0,0,255,255,255,255,255,255])).save(source/'assets/colors.png')
  (source/'source/src/lib.rs').write_text('''pub fn game_tick(input:u32,_state:u64)->u64 {
 gamepkg::draw_clear(0x10171dff);let shift=if input&gamepkg::HELD_RIGHT!=0 {20.0} else {0.0};let swap=input&gamepkg::HELD_DOWN!=0;let marker=if swap {0x00ffffff} else if shift>0.0 {0x00ff00ff} else {0xff00ffff};gamepkg::draw_rect(4.0,4.0,16.0,16.0,marker);
 let paths=if swap {["colors.png","gray.png"]} else {["gray.png","colors.png"]};for i in 0..2 {let x=80.0+i as f32*280.0;gamepkg::draw_asset_image(paths[i],x,40.0,200.0,20.0);gamepkg::draw_asset_image_linear(paths[i],x+shift,80.0,200.0,320.0);}1}
''')
  Image.frombytes('RGBA',(512,1),bytes(component for pixel in range(512) for component in [255 if pixel%2 else 0]*3+[255])).save(source/'assets/stripes.png')
  entry=source/'source/src/lib.rs';entry.write_text(entry.read_text().replace('}1}', '}gamepkg::draw_asset_image_linear("gray.png",-40.0,420.0,200.0,40.0);gamepkg::draw_asset_image_linear("stripes.png",240.0,420.0,128.0,40.0);1}'))
  manifest=(source/'manifest.toml').read_text().replace('com.gamepkg.minimal','com.gamepkg.filtering-probe');cli=PROJECT/'target/release/gamepkg';gui=PROJECT/'target/release/gamepkg-gui'
  def run(*arguments):
   result=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stderr
  key=root/'test.key';run('keygen','-o',key)
  for profile in (['core-v1'] if args.baseline_core else ['core-v1','component-v1']):
   game_id='com.gamepkg.filtering-probe'+('-component' if profile=='component-v1' else '');(source/'manifest.toml').write_text(f'runtime_profile="{profile}"\n'+manifest.replace('com.gamepkg.filtering-probe',game_id));package=root/(profile+'.gamepkg')
   run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build',game_id,'--target','wasm')
   for target in (['wasm','native'] if profile=='core-v1' and not args.baseline_core else ['wasm']):
    if target=='native':run('build',game_id,'--target','native')
    for renderer in ([args.renderer] if args.renderer else ['vulkan'] if args.baseline_core else ['vulkan','opengl']):
     app=subprocess.Popen([str(gui),'--play',game_id,'--renderer',renderer],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
     try:
      deadline=time.monotonic()+20
      while True:
       if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
       found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
       if found.returncode==0:
        path=PROOF/(profile+'-'+target+'-'+renderer+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',found.stdout.splitlines()[0],str(path)],env=env,check=True,timeout=10);image=Image.open(path).convert('RGB');scale=min(image.width/640,image.height/480);ox=(image.width-640*scale)/2;oy=(image.height-480*scale)/2
        if image.getpixel((math.floor(ox+12*scale+.5),math.floor(oy+12*scale+.5)))==(255,0,255):break
       assert time.monotonic()<deadline;time.sleep(.05)
      actual=[list(image.getpixel((math.floor(ox+x*scale+.5),math.floor(oy+y*scale+.5)))) for x,y in POINTS];label=profile+'-'+target+'-'+renderer;report['desktop'][label]=actual;compare(actual,expected(image.width,image.height),label)
      window=found.stdout.splitlines()[0]
      for key_name,marker,shift,swap in [('Right',(0,255,0),20,False),('Down',(0,255,255),0,True)]:
       subprocess.run(['xdotool','windowfocus',window,'keydown',key_name],env=env,check=True)
       try:
        deadline=time.monotonic()+10;dynamic_label=label+('-moved' if shift else '-swapped')
        while True:
         if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
         path=PROOF/(dynamic_label+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);image=Image.open(path).convert('RGB');scale=min(image.width/640,image.height/480);ox=(image.width-640*scale)/2;oy=(image.height-480*scale)/2
         if image.getpixel((math.floor(ox+12*scale+.5),math.floor(oy+12*scale+.5)))==marker:break
         assert time.monotonic()<deadline,'Dynamic sampler frame did not become ready';time.sleep(.05)
        points=[(x+(shift if i<30 else 0),y) for i,(x,y) in enumerate(POINTS)];actual=[list(image.getpixel((math.floor(ox+x*scale+.5),math.floor(oy+y*scale+.5)))) for x,y in points];report['desktop'][dynamic_label]=actual;compare(actual,expected(image.width,image.height,shift,swap),dynamic_label)
       finally:subprocess.run(['xdotool','keyup',key_name],env=env,check=True)

     finally:
      if app.poll() is None:app.terminate()
      app.communicate(timeout=10)
   if not args.baseline_core and not args.skip_browser:
    from selenium import webdriver
    from selenium.webdriver.chrome.options import Options
    from selenium.webdriver.chrome.service import Service
    from selenium.webdriver.support.ui import WebDriverWait
    output=root/(profile+'-web');run('web-export',game_id,'--output',output);server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();options.binary_location='/usr/bin/chromium'
    for value in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader']:options.add_argument(value)
    browser=webdriver.Chrome(service=Service('/usr/bin/chromedriver'),options=options)
    try:
     browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();wait.until(lambda b:b.find_element('id','game').get_attribute('data-state'))
     actual=browser.execute_script('const c=document.querySelector("#game").getContext("2d");return arguments[0].map(p=>Array.from(c.getImageData(p[0],p[1],1,1).data).slice(0,3));',POINTS);report['browser'][profile]=actual;compare(actual,expected(640,480),'browser-'+profile)
     from selenium.webdriver.common.action_chains import ActionChains
     from selenium.webdriver.common.keys import Keys
     browser.find_element('id','game').click()
     for key_name,marker,shift,swap in [(Keys.ARROW_RIGHT,[0,255,0],20,False),(Keys.ARROW_DOWN,[0,255,255],0,True)]:
      ActionChains(browser).key_down(key_name).perform()
      try:
       wait.until(lambda b:b.execute_script('return Array.from(document.querySelector("#game").getContext("2d").getImageData(12,12,1,1).data).slice(0,3)',)==marker)
       points=[(x+(shift if i<30 else 0),y) for i,(x,y) in enumerate(POINTS)];actual=browser.execute_script('const c=document.querySelector("#game").getContext("2d");return arguments[0].map(p=>Array.from(c.getImageData(p[0],p[1],1,1).data).slice(0,3));',points);dynamic_label=profile+('-moved' if shift else '-swapped');report['browser'][dynamic_label]=actual;compare(actual,expected(640,480,shift,swap),'browser-'+dynamic_label)
      finally:ActionChains(browser).key_up(key_name).perform()

    finally:browser.quit();server.shutdown();server.server_close()
 finally:display.terminate();display.wait(timeout=10)
