"""Original backdrop and implemented menu controls; missing controls remain separate gates."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,base64,hashlib,io,json,os,platform,shutil,subprocess,tempfile,threading,time,tomllib
from PIL import Image,ImageChops
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('--candidate',type=Path,help='Separate staged recipe; defaults to the active port');parser.add_argument('--source-resolution',action='store_true',help='Compare scaled photographic backgrounds directly to original SDL');parser.add_argument('--original',type=Path,default=Path('/tmp/gamepkg-candidate-arkanoo'));parser.add_argument('--flat-background',action='store_true',help='Compare scaled controls directly against SDL without background resampling');parser.add_argument('--proof-output',type=Path);parser.add_argument('--browser-repeats',type=int,choices=range(1,6),default=1,help='Repeat the complete browser control cycle for input/worker diagnostics');args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1];CANDIDATE=args.candidate or PROJECT/'ports/arkanoo';DIRECT_SCALED=args.flat_background or args.source_resolution or tomllib.loads((CANDIDATE/'manifest.toml').read_text())['limits'].get('render_profile')=='large-images-v1';BIN=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'));CLI=BIN/'gamepkg';GUI=BIN/'gamepkg-gui';PROOF=args.proof_output or PROJECT/'target/menu-controls-proof'
if PROOF.exists():PROOF.rename(PROJECT/'target'/('menu-controls-previous-'+str(time.time_ns())))
PROOF.mkdir(parents=True)
fixture=json.loads((PROJECT/'tests/fixtures/arkanoo-menu-controls.json').read_text());PHASES=[0,1,2,3,4,0];KEYS={4:'Left',1:'Right',2:'Up',3:'Down'};REFERENCES={};SMALL={};SMALL_REFERENCES={}
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
def fail_start(label,message,env):
 # Capture only this test's private display, including native error dialogs.
 try:
  subprocess.run(['import','-window','root',str(PROOF/(label+'-startup.png'))],env=env,capture_output=True,timeout=5)
  titles=subprocess.run(['xdotool','search','--onlyvisible','--name','.','getwindowname','%@'],env=env,capture_output=True,text=True,timeout=5)
  details={'message':message,'visible_windows':titles.stdout,'window_error':titles.stderr}
 except Exception as error:details={'message':message,'capture_error':str(error)}
 (PROOF/(label+'-startup.json')).write_text(json.dumps(details,indent=2)+'\n')
 raise RuntimeError(label+': '+message+'; '+details.get('visible_windows',''))
def browser_image(browser):
 # Native lossless encoding avoids serializing 921,600 individual JS values
 # while the main thread owns a pending AudioWorklet response deadline.
 encoded=browser.execute_script('const began=performance.now();setTimeout(()=>{window.menuPNGDelay=Math.max(window.menuPNGDelay||0,performance.now()-began);},0);return document.querySelector("#game").toDataURL("image/png");')
 assert encoded.startswith('data:image/png;base64,')
 with Image.open(io.BytesIO(base64.b64decode(encoded.split(',',1)[1],validate=True))) as image:
  assert image.size==(640,480)
  return image.convert('RGB').crop((0,60,640,420))
def pixel_report(actual,expected):
 a=actual.tobytes();b=expected.tobytes();points=[];channels=0
 for index in range(0,len(a),3):
  errors=[abs(a[index+c]-b[index+c]) for c in range(3)];channels+=sum(v>2 for v in errors)
  if max(errors)>2 and len(points)<32:points.append(dict(x=(index//3)%actual.width,y=(index//3)//actual.width,actual=list(a[index:index+3]),expected=list(b[index:index+3])))
 return dict(size=actual.size,actual_sha256=hashlib.sha256(a).hexdigest(),expected_sha256=hashlib.sha256(b).hexdigest(),channels_over_limit=channels,first_differing_pixels=points)
with tempfile.TemporaryDirectory(prefix='gamepkg-menu-controls-gui-') as temp:
 root=Path(temp);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n');env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
 def run(*args):
  r=subprocess.run([CLI,*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr
 font=args.original/'assets/font/static/Roboto-Medium.ttf';assert hashlib.sha256(font.read_bytes()).hexdigest()=='616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'
 assert hashlib.sha256((args.original/'src/rendering.rs').read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
 reference_source=(PROJECT/'tests/reference/arkanoo_menu_controls.py').read_text().split("c=r'''",1)[1].split("'''",1)[0];(root/'reference.c').write_text(reference_source);reference=root/'reference';subprocess.run(['cc',root/'reference.c','-o',reference,*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf','SDL2_image'],text=True).split()],check=True)
 key=root/'key';key.write_text('54'*32)
 background=args.original/'assets/background1.png'
 if args.flat_background:
  background=root/'flat-background.png';Image.new('RGBA',(1,1),(18,52,86,255)).save(background)
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  source=root/suffix;shutil.copytree(CANDIDATE,source);entry=source/'source/src/lib.rs';modules=entry.read_text().split('use std::sync::Mutex;')[0]
  if args.flat_background:
   # Keep the original control code and font. Only the private backdrop is flat.
   render=source/'source/src/render.rs';text=render.read_text();begin=text.index('pub fn draw_menu_background()');end=text.index('\nfn ',begin)+1
   render.write_text(text[:begin]+'pub fn draw_menu_background()->bool {let accepted=gamepkg::draw_rect(0.0,60.0,640.0,360.0,0x123456ff);if !gamepkg::draw_rect(0.0,60.0,640.0,360.0,150){return false;}accepted}\n'+text[end:])
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
   raw=subprocess.check_output([reference,font,background,*map(str,(*state,*defaults))],env=env);REFERENCES[phase]=Image.frombytes('RGBA',(1280,720),raw).convert('RGB')
   REFERENCES[phase].save(PROOF/('original-1280-'+str(phase)+'.png'))
   if DIRECT_SCALED:
    raw=subprocess.check_output([reference,font,background,*map(str,(*state,*defaults,640,360))],env=env);SMALL_REFERENCES[phase]=Image.frombytes('RGBA',(640,360),raw).convert('RGB');SMALL_REFERENCES[phase].save(PROOF/('original-640-'+str(phase)+'.png'))
  def delta(actual,expected):return max(p[1] for p in ImageChops.difference(actual,expected).getextrema())
  for profile,suffix in [('core-v1','core'),('component-v1','component')]:
   game_id='com.gamepkg.menu-'+suffix
   for target in (['wasm','native'] if suffix=='core' else ['wasm']):
    run('build',game_id,'--target',target)
    for renderer in ['vulkan','opengl']:
     label=profile+'-'+target+'-'+renderer;app=subprocess.Popen([GUI,'--play',game_id,'--target',target,'--renderer',renderer],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);held=False
     try:
      deadline=time.monotonic()+20
      while True:
       if app.poll() is not None:fail_start(label,f'exited with {app.returncode}: '+app.stderr.read().decode(),env)
       found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
       if found.returncode==0:window=found.stdout.splitlines()[0];break
       if time.monotonic()>=deadline:fail_start(label,'game window did not appear within 20 seconds',env)
       time.sleep(.05)
      sizes=[(1280,960)]+([(640,480)] if DIRECT_SCALED or (suffix,target,renderer)==('core','native','vulkan') else [])
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
          if marker==wanted and (not DIRECT_SCALED or delta(actual,SMALL_REFERENCES[phase])<=2):
           if (suffix,target,renderer)==('core','native','vulkan'):SMALL[phase]=actual.copy();actual.save(PROOF/('native-640-'+str(phase)+'.png'))
           if DIRECT_SCALED:actual.save(PROOF/(label+'-640-'+str(phase)+'-canvas.png'))
           break
         elif delta(actual,REFERENCES[phase])<=2:break
         if app.poll() is not None:fail_start(label,f'exited with {app.returncode}: '+app.stderr.read().decode(),env)
         assert time.monotonic()<deadline,(label,phase,delta(actual,REFERENCES[phase]) if width==1280 else (marker,delta(actual,SMALL_REFERENCES[phase])) if DIRECT_SCALED else marker);time.sleep(.05)
        if width==1280:print(label,phase,'complete implemented menu maximum difference',delta(actual,REFERENCES[phase]),flush=True)
        elif DIRECT_SCALED:print(label,phase,'scaled controls/original SDL maximum difference',delta(actual,SMALL_REFERENCES[phase]),flush=True)
     finally:
      if held:subprocess.run(['xdotool','keyup',held],env=env,check=True)
      if app.poll() is None:app.terminate()
      stdout,stderr=app.communicate(timeout=10);(PROOF/(label+'-stdout.log')).write_bytes(stdout);(PROOF/(label+'-stderr.log')).write_bytes(stderr)
   from selenium import webdriver
   from selenium.webdriver.chrome.options import Options
   from selenium.webdriver.chrome.service import Service
   from selenium.webdriver.common.action_chains import ActionChains
   from selenium.webdriver.common.keys import Keys
   from selenium.webdriver.support.ui import WebDriverWait
   output=root/(suffix+'-web');run('web-export',game_id,'--output',output);server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert options.binary_location
   for value in ['--headless=new','--mute-audio','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader']:options.add_argument(value)
   driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');browser=webdriver.Chrome(service=Service(driver),options=options);held=False
   browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':"""window.menuAudioTrace=[];window.menuMaxAudioReply=0;const BaseOutput=window.AudioWorkletNode;window.AudioWorkletNode=class extends BaseOutput{constructor(...args){super(...args);const port=this.port,post=port.postMessage.bind(port),pending=new Map(),record=value=>{window.menuAudioTrace.push({at:performance.now(),...value});if(window.menuAudioTrace.length>64)window.menuAudioTrace.shift();};port.postMessage=(data,...rest)=>{const key=data?.session+':'+data?.id;pending.set(key,performance.now());if(pending.size>32)pending.delete(pending.keys().next().value);record({direction:'sent',kind:data?.kind,id:data?.id,session:data?.session});return post(data,...rest);};port.addEventListener('message',({data})=>{const key=data?.session+':'+data?.id,begin=pending.get(key),elapsed=begin===undefined?null:performance.now()-begin;pending.delete(key);if(elapsed!==null)window.menuMaxAudioReply=Math.max(window.menuMaxAudioReply,elapsed);record({direction:'received',id:data?.id,session:data?.session,ok:data?.ok,elapsed});});}};window.menuTrace=[];const note=value=>{window.menuTrace.push({at:performance.now(),...value});if(window.menuTrace.length>48)window.menuTrace.shift();};for(const kind of ['keydown','keyup','focus','blur'])document.addEventListener(kind,event=>note({kind,key:event.key,code:event.code,repeat:event.repeat,target:event.target?.id}),true);const NativeWorker=window.Worker;window.Worker=class extends NativeWorker{constructor(...args){super(...args);this.addEventListener('message',({data})=>note({kind:'reply',type:data?.type,state:String(data?.state),serial:data?.serial,error:data?.error}));this.addEventListener('error',event=>note({kind:'worker-error',error:event.message}));}postMessage(data,...args){if(data?.type==='tick'&&data.input)note({kind:'tick',input:data.input});return super.postMessage(data,...args);}};"""})
   try:
    browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();wait.until(lambda b:b.find_element('id','game').get_attribute('data-state'));browser.find_element('id','game').click()
    for phase in PHASES*args.browser_repeats:
     if held:ActionChains(browser).key_up(held).perform();held=False
     if phase:held={1:Keys.ARROW_RIGHT,2:Keys.ARROW_UP,3:Keys.ARROW_DOWN,4:Keys.ARROW_LEFT}[phase];ActionChains(browser).key_down(held).perform()
     wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')==str(phase));actual=browser_image(browser);difference=delta(actual,SMALL[phase]);actual.save(PROOF/(profile+'-web-'+str(phase)+'.png'));print(profile,phase,'implemented menu browser/native maximum difference',difference,flush=True)
     if difference>2:
      report=pixel_report(actual,SMALL[phase]);report.update(maximum_difference=difference,profile=profile,phase=phase,browser_version=browser.capabilities.get('browserVersion'),driver_version=browser.capabilities.get('chrome',{}).get('chromedriverVersion'),platform=platform.platform())
      cpu=Path('/proc/cpuinfo')
      if cpu.exists():report['cpu']=next((line.split(':',1)[1].strip() for line in cpu.read_text().splitlines() if line.startswith('model name')),None)
      try:report['webgl']=browser.execute_script('const g=new OffscreenCanvas(1,1).getContext("webgl2");if(!g)return null;const ext=g.getExtension("WEBGL_debug_renderer_info"),out={vendor:g.getParameter(ext?ext.UNMASKED_VENDOR_WEBGL:g.VENDOR),renderer:g.getParameter(ext?ext.UNMASKED_RENDERER_WEBGL:g.RENDERER),version:g.getParameter(g.VERSION)};g.getExtension("WEBGL_lose_context")?.loseContext();return out;')
      except Exception as error:report['webgl_error']=str(error)
      (PROOF/(profile+'-web-'+str(phase)+'-difference.json')).write_text(json.dumps(report,indent=2)+'\n')
     assert difference<=2,difference
     if DIRECT_SCALED:
      difference=delta(actual,SMALL_REFERENCES[phase]);print(profile,phase,'scaled browser controls/original SDL maximum difference',difference,flush=True);assert difference<=2,difference
    # Verify the serialization change against the old exact byte capture after
    # stopping playback, so the reference check cannot itself starve audio RPCs.
    browser.find_element('id','stop').click();wait.until(lambda b:b.find_element('id','status').text=='Stopped')
    png=browser_image(browser);before=time.monotonic()
    raw=browser.execute_script('const began=performance.now();setTimeout(()=>{window.menuRawDelay=performance.now()-began;},0);return Array.from(document.querySelector("#game").getContext("2d").getImageData(0,60,640,360).data);')
    legacy=Image.frombytes('RGBA',(640,360),bytes(raw)).convert('RGB')
    assert png.tobytes()==legacy.tobytes(),'PNG capture changed original pixels'
    print(profile,'lossless PNG capture matches every legacy RGB byte; stopped raw readback seconds',time.monotonic()-before,'timing',browser.execute_script('return {png:window.menuPNGDelay,raw:window.menuRawDelay,audioReply:window.menuMaxAudioReply};'),flush=True)
   except Exception as error:
    details={'error':str(error),'profile':profile,'phase':phase if 'phase' in locals() else None}
    try:
     details.update(browser.execute_script('return {status:document.querySelector("#status")?.textContent,state:document.querySelector("#game")?.dataset.state,active:document.activeElement?.id,focused:document.hasFocus(),trace:window.menuTrace,audio:window.menuAudioTrace,maxAudioReplyMS:window.menuMaxAudioReply,pngReadbackMS:window.menuPNGDelay};'))
     details['console']=browser.get_log('browser');browser.save_screenshot(str(PROOF/(profile+'-browser-failure.png')))
    except Exception as diagnostic:details['diagnostic_error']=str(diagnostic)
    (PROOF/(profile+'-browser-failure.json')).write_text(json.dumps(details,indent=2)+'\n')
    print('Browser menu failure:',json.dumps(details),flush=True)
    raise
   finally:
    if held:ActionChains(browser).key_up(held).perform()
    browser.quit();server.shutdown();server.server_close()
 finally:display.terminate();display.wait(timeout=10)
