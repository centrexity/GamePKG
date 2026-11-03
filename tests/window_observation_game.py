"""Real port/SDK observation, actual OS fullscreen changes and browser exits."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import json,os,re,shutil,subprocess,tempfile,threading,time
from contextlib import contextmanager
from PIL import Image
from gui_test_support import start_display
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg';GUI=PROJECT/'target/release/gamepkg-gui'
compiled=subprocess.run(['cargo','test','--release','--test','window_observation_runtime','--no-run','--message-format=json'],capture_output=True,text=True,timeout=300);assert compiled.returncode==0,compiled.stderr
executables=[v['executable'] for line in compiled.stdout.splitlines() if(v:=json.loads(line)).get('reason')=='compiler-artifact' and v.get('target',{}).get('name')=='window_observation_runtime' and v.get('executable')];assert len(executables)==1
vendor=subprocess.run(['cargo','build','--locked','--features','gui','--lib','--message-format=json'],capture_output=True,text=True,timeout=300);assert vendor.returncode==0,vendor.stderr
libraries={}
for line in vendor.stdout.splitlines():
 value=json.loads(line)
 if value.get('reason')=='compiler-artifact' and value.get('target',{}).get('name') in ['winit','raw_window_handle','x11_dl']:
  libraries[value['target']['name']]=next(Path(name) for name in value['filenames'] if name.endswith('.rlib'))
assert len(libraries)==3,libraries
vendor_test=PROJECT/'target/gamepkg-window-observation-test'
command=['rustc','--edition=2021','--test',str(PROJECT/'vendor/egui-winit/src/gamepkg_window.rs'),'--cfg','feature="x11"','-L','dependency='+str(libraries['winit'].parent),'-o',str(vendor_test)]
for name,library in libraries.items():command+=['--extern',name+'='+str(library)]
subprocess.run(command,check=True)
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-window-observation-') as temporary:
 root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n');env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None);key=root/'key';key.write_text('36'*32)
 def run(*args):
  value=subprocess.run(list(map(str,args)),env=env,capture_output=True,text=True,timeout=180);assert value.returncode==0,(args,value.stderr);return value.stdout
 for profile,suffix in [('core-v1','core'),('component-v1','component'),('core-v1','core-denied'),('component-v1','component-denied')]:
  source=root/suffix;allowed=not suffix.endswith('denied');shutil.copytree(PROJECT/('ports/arkanoo' if allowed else 'examples/minimal'),source);manifest=source/'manifest.toml';text=manifest.read_text().replace('com.gamepkg.arkanoo' if allowed else 'com.gamepkg.minimal','com.gamepkg.window-'+suffix);text='runtime_profile="'+profile+'"\n'+text
  if not allowed:text=text.replace('input = true','input = false')
  manifest.write_text(text);entry=source/'source/src/lib.rs'
  if allowed:
   text=entry.read_text().replace('pub fn game_tick(input: u32, state: u64) -> u64 {','fn production_tick(input: u32, state: u64) -> u64 {')
   text+='''\nstatic RESOLUTION_KEPT:Mutex<bool>=Mutex::new(false);\npub fn game_tick(input:u32,state:u64)->u64 {if input&128!=0 {loop {gamepkg::window_fullscreen();}}production_tick(input,state);let observed=gamepkg::window_fullscreen();for _ in 0..64 {if gamepkg::window_fullscreen()!=observed {return u64::MAX;}}let mut encoded=1u64;let mut color=0x785000ff;if let Some(fullscreen)=observed {encoded=if fullscreen {4}else{2};color=if fullscreen {0x14b450ff}else{0x1464b4ff};if let Ok(slot)=GAME.lock(){if let Some(game)=slot.as_ref(){if game.menu.fullscreen!=fullscreen {encoded=99;color=0xff00ffff;}}}}gamepkg::draw_rect(4.0,4.0,16.0,16.0,color);encoded}\n'''
  else:text='pub fn game_tick(_:u32,_:u64)->u64 {if gamepkg::window_fullscreen().is_some(){99}else{gamepkg::draw_clear(0x785000ff);1}}'
  text=text.replace('pub fn game_tick(input:u32,state:u64)->u64 {','pub fn game_tick(input:u32,state:u64)->u64 {if input==64 {if let Some(size)=gamepkg::window_size(){return u64::from(size[0])|(u64::from(size[1])<<32);}return 0;}') if allowed else text.replace('if gamepkg::window_fullscreen().is_some()', 'if gamepkg::window_fullscreen().is_some()||gamepkg::window_size().is_some()||gamepkg::window_preview_active()')
  if allowed:text=text.replace('game.source_input(event.kind, event.x, event.y, event.code);','if event.kind==5 {if event.code==62 {if gamepkg::window_preview_size(1000,750){if let Ok(mut kept)=RESOLUTION_KEPT.lock(){*kept=false;}}}if event.code==63 {if gamepkg::window_keep_size(){if let Ok(mut kept)=RESOLUTION_KEPT.lock(){*kept=true;}}}if event.code==64 {if gamepkg::window_revert_size(){if let Ok(mut kept)=RESOLUTION_KEPT.lock(){*kept=false;}}}}game.source_input(event.kind, event.x, event.y, event.code);')
  if allowed:text=text.replace('gamepkg::draw_rect(4.0,4.0,16.0,16.0,color);','if let Some(size)=gamepkg::window_size(){gamepkg::draw_rect(28.0,4.0,16.0,16.0,((size[0]&255)<<24)|((size[0]>>8)<<16)|255);gamepkg::draw_rect(52.0,4.0,16.0,16.0,((size[1]&255)<<24)|((size[1]>>8)<<16)|255);}gamepkg::draw_rect(4.0,4.0,16.0,16.0,color);')
  if allowed:text=text.replace('gamepkg::draw_rect(4.0,4.0,16.0,16.0,color);','let kept_color=if let Ok(kept)=RESOLUTION_KEPT.lock(){if *kept {0x00ff00ff}else{0x000000ff}}else{0xff0000ff};gamepkg::draw_rect(76.0,4.0,16.0,16.0,kept_color);gamepkg::draw_rect(4.0,4.0,16.0,16.0,color);')
  if allowed:text=text.replace('gamepkg::draw_rect(4.0,4.0,16.0,16.0,color);','gamepkg::draw_rect(124.0,4.0,16.0,16.0,if gamepkg::window_preview_active(){0xff00ffff}else{0x000000ff});gamepkg::draw_rect(4.0,4.0,16.0,16.0,color);')
  entry.write_text(text);package=root/(suffix+'.gamepkg');run(CLI,'pack',source,'-o',package,'--key',key);run(CLI,'install',package,'--trust-creator');run(CLI,'build','com.gamepkg.window-'+suffix,'--target','wasm')
 test_env=dict(env,GAMEPKG_WINDOW_OBSERVATION_ROOT=str(root/'library'));subprocess.run([executables[0],'--ignored'],env=test_env,check=True,timeout=120)
 for suffix in ['core','core-denied']:run(CLI,'build','com.gamepkg.window-'+suffix,'--target','native')
 subprocess.run([executables[0],'--ignored'],env=test_env,check=True,timeout=120)
 display=start_display(env,width=1400,height=1000);wm=None;app=None
 def wait(check,message,seconds=20):
  end=time.monotonic()+seconds
  while True:
   if check():return
   assert app is None or app.poll() is None,'Test game exited before '+message
   if time.monotonic()>=end:
    names=subprocess.run(['xdotool','search','--onlyvisible','--name','.','getwindowname','%@'],env=env,capture_output=True,text=True)
    subprocess.run(['import','-display',env['DISPLAY'],'-window','root','/tmp/gamepkg-window-size-failure.png'],env=env,check=True)
    raise AssertionError((message,names.stdout))
   time.sleep(.05)
 def capture_window(window):
  # Fullscreen can briefly unmap a client. ImageMagick waits indefinitely on
  # that drawable; the private root stays viewable throughout the transition.
  mapped=subprocess.run(['xwininfo','-id',window],env=env,capture_output=True,text=True,timeout=5)
  if mapped.returncode or 'Map State: IsViewable' not in mapped.stdout:return None
  # xdotool reports the decorated frame origin under this WM. Xwininfo's
  # absolute client origin excludes the title bar, matching the API contract.
  def client_geometry(info):
   patterns=[r'Absolute upper-left X:\s*(-?\d+)',r'Absolute upper-left Y:\s*(-?\d+)',r'^\s*Width:\s*(\d+)',r'^\s*Height:\s*(\d+)']
   return tuple(int(re.search(pattern,info,re.M).group(1)) for pattern in patterns)
  geometry=client_geometry(mapped.stdout)
  x,y,width,height=geometry
  image=root/'root-observation.png'
  value=subprocess.run(['import','-display',env['DISPLAY'],'-window','root',str(image)],env=env,capture_output=True,text=True,timeout=5)
  assert value.returncode==0,value.stderr
  after=subprocess.run(['xwininfo','-id',window],env=env,capture_output=True,text=True,timeout=5)
  if after.returncode or 'Map State: IsViewable' not in after.stdout or client_geometry(after.stdout)!=geometry:return None
  with Image.open(image) as picture:
   if x<0 or y<0 or x+width>picture.width or y+height>picture.height:return None
   return picture.convert('RGB').crop((x,y,x+width,y+height))
 def color_is(window,color):
  picture=capture_window(window)
  if picture is None:return False
  scale=min(picture.width/640,picture.height/480);x=(picture.width-640*scale)/2+10*scale;y=(picture.height-480*scale)/2+10*scale
  return max(abs(a-b) for a,b in zip(picture.getpixel((round(x),round(y))),color))<=1
 def kept_is_visible(window):
  picture=capture_window(window)
  if picture is None:return False
  scale=min(picture.width/640,picture.height/480);ox=(picture.width-640*scale)/2;oy=(picture.height-480*scale)/2
  return picture.getpixel((round(ox+84*scale),round(oy+12*scale)))==(0,255,0)
 def preview_is(window,active):
  picture=capture_window(window)
  if picture is None:return False
  scale=min(picture.width/640,picture.height/480);ox=(picture.width-640*scale)/2;oy=(picture.height-480*scale)/2
  return picture.getpixel((round(ox+132*scale),round(oy+12*scale)))==((255,0,255) if active else (0,0,0))
 def size_is(window):
  picture=capture_window(window)
  if picture is None:return False
  width,height=picture.size
  scale=min(width/640,height/480);ox=(width-640*scale)/2;oy=(height-480*scale)/2
  for x,value in [(36,width),(60,height)]:
   actual=picture.getpixel((round(ox+x*scale),round(oy+12*scale)))
   if max(abs(a-b) for a,b in zip(actual,(value&255,value>>8,0)))>1:return False
  return True
 try:
  wm=subprocess.Popen(['metacity','--sm-disable','--replace','--no-composite'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);wait(lambda:'window id #' in run('xprop','-root','_NET_SUPPORTING_WM_CHECK'),'private window manager')
  subprocess.run([vendor_test,'--ignored','bounded_own_window_properties'],env=env,check=True,timeout=30)
  for suffix,target in [('core','wasm'),('core','native'),('component','wasm')]:
   game_id='com.gamepkg.window-'+suffix
   for iteration,restored in enumerate([False,True,False,False,True,True]):
    config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\nui_scale_percent='+('150' if iteration%2 else '100')+'\n')
    app=subprocess.Popen([GUI,'--play',game_id,'--target',target,'--renderer','opengl'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);box=[]
    def find():
     value=subprocess.run(['xdotool','search','--onlyvisible','--name','^'+game_id.replace('.','\\.')+'$'],env=env,capture_output=True,text=True)
     if value.returncode==0:box[:]=[value.stdout.splitlines()[0]];return True
     return False
    wait(find,'own game window');window=box[0];run('xdotool','windowactivate','--sync',window)
    if not restored and '_NET_WM_STATE_FULLSCREEN' in run('xprop','-id',window,'_NET_WM_STATE'):run('wmctrl','-ir',hex(int(window)),'-b','remove,fullscreen')
    wait(lambda:color_is(window,(20,180,80) if restored else (20,100,180)),'observed restored/normal state')
    wait(lambda:size_is(window),'actual physical client size')
    if iteration==0:
     for width,height in [(1000,750),(1125,800)]:
      run('xdotool','windowsize',window,str(width),str(height));wait(lambda:size_is(window),'resized physical client size')
     def dimensions():
      fields=dict(line.split('=',1) for line in run('xdotool','getwindowgeometry','--shell',window).splitlines() if '=' in line)
      return int(fields['WIDTH']),int(fields['HEIGHT'])
     original=dimensions()
     def maximized():
      value=run('xprop','-id',window,'_NET_WM_STATE')
      return '_NET_WM_STATE_MAXIMIZED_VERT' in value and '_NET_WM_STATE_MAXIMIZED_HORZ' in value
     run('wmctrl','-ir',hex(int(window)),'-b','add,maximized_vert,maximized_horz');wait(maximized,'maximized preview origin')
     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750) and not maximized(),'preview from maximized window')
     run('xdotool','key','--window',window,'F7');wait(maximized,'maximized state restored after preview')
     run('wmctrl','-ir',hex(int(window)),'-b','remove,maximized_vert,maximized_horz');wait(lambda:dimensions()==original,'normal size retained under restored maximization')
     run('wmctrl','-ir',hex(int(window)),'-b','add,maximized_vert,maximized_horz');wait(maximized,'maximized Keep origin')
     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750) and not maximized(),'Keep preview from maximized window')
     run('xdotool','key','--window',window,'F6');wait(lambda:kept_is_visible(window),'Keep acknowledgement from maximized window')
     run('xdotool','key','alt+F4');assert app.wait(timeout=15)==0;app.communicate();app=None
     app=subprocess.Popen([GUI,'--play',game_id,'--target',target,'--renderer','opengl'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);box.clear()
     wait(find,'reopened kept maximized-origin preview');window=box[0];run('xdotool','windowactivate','--sync',window)
     wait(lambda:dimensions()==(1000,750) and not maximized(),'kept maximized-origin size persisted as normal')
     run('xdotool','windowsize',window,str(original[0]),str(original[1]));wait(lambda:size_is(window),'normal baseline after kept preview reopen')

     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750),'preview size')
     run('xdotool','key','--window',window,'F7');wait(lambda:dimensions()==original,'explicit preview revert')
     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750),'timeout preview size')
     wait(lambda:dimensions()==original,'automatic preview revert',seconds=15)
     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750),'kept preview size')
     run('xdotool','key','--window',window,'F6');time.sleep(5.5)
     assert dimensions()==(1000,750),'confirmed preview reverted'

    if iteration==2:
     wait(maximized,'unconfirmed maximized preview restored saved maximization')
     run('wmctrl','-ir',hex(int(window)),'-b','remove,maximized_vert,maximized_horz');wait(lambda:dimensions()==(1125,800),'unconfirmed maximized preview retained normal bounds')
     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750),'normal unconfirmed close preview')
     run('xdotool','key','alt+F4');assert app.wait(timeout=15)==0;app.communicate();app=None
     continue
    if iteration==3:wait(lambda:dimensions()==(1125,800) and not maximized(),'unconfirmed normal preview restored saved normal size')
    if iteration==3:
     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750) and preview_is(window,True),'fullscreen timeout preview')
     run('xdotool','key','--window',window,'F11');wait(lambda:color_is(window,(20,180,80)),'fullscreen during pending preview')
     wait(lambda:preview_is(window,False),'preview expired while fullscreen',seconds=15)
     run('xdotool','key','alt+F4');assert app.wait(timeout=15)==0;app.communicate();app=None
     continue
    if iteration==4:
     run('xdotool','key','--window',window,'F11');wait(lambda:color_is(window,(20,100,180)),'fullscreen timeout reopen exits fullscreen')
     wait(lambda:dimensions()==(1125,800),'fullscreen timeout retained original normal size')
     wait(lambda:size_is(window),'coalesced fullscreen preview baseline')
     run('xdotool','keydown','--window',window,'F5','keyup','--window',window,'F5','keydown','--window',window,'F11','keyup','--window',window,'F11')
     wait(lambda:color_is(window,(20,180,80)) and preview_is(window,True),'coalesced preview/fullscreen before Keep')
     run('xdotool','key','--window',window,'F6');wait(lambda:kept_is_visible(window) and preview_is(window,False),'committed fullscreen Keep')
     run('xdotool','key','alt+F4');assert app.wait(timeout=15)==0;app.communicate();app=None
     continue
    if iteration==5:
     run('xdotool','key','--window',window,'F11');wait(lambda:color_is(window,(20,100,180)),'fullscreen Keep reopen exits fullscreen')
     wait(lambda:dimensions()==(1000,750),'fullscreen Keep preserved preview normal size')
     run('xdotool','windowsize',window,'1125','800');wait(lambda:size_is(window),'fullscreen revert baseline')
     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750) and preview_is(window,True),'fullscreen explicit revert preview')
     run('xdotool','key','--window',window,'F11');wait(lambda:color_is(window,(20,180,80)),'fullscreen before explicit revert')
     run('xdotool','key','--window',window,'F7');wait(lambda:preview_is(window,False),'committed fullscreen explicit revert')
     run('xdotool','key','--window',window,'F11');wait(lambda:color_is(window,(20,100,180)),'fullscreen explicit revert exit')
     wait(lambda:dimensions()==(1125,800),'fullscreen exit applied deferred original size')
     run('xdotool','key','alt+F4');assert app.wait(timeout=15)==0;app.communicate();app=None
     continue
    for enabled in [not restored,restored,True]:
     run('wmctrl','-ir',hex(int(window)),'-b',('add' if enabled else 'remove')+',fullscreen');wait(lambda:color_is(window,(20,180,80) if enabled else (20,100,180)),'external fullscreen observation');wait(lambda:size_is(window),'fullscreen physical client size')
    run('xdotool','key','--window',window,'F11');wait(lambda:color_is(window,(20,100,180)),'F11 exits externally changed fullscreen')
    if iteration==1:
     wait(lambda:dimensions()==(1000,750),'confirmed size survived reopen')
     run('xdotool','windowsize',window,'1125','800');wait(lambda:size_is(window),'new pre-preview normal size')
     run('wmctrl','-ir',hex(int(window)),'-b','add,maximized_vert,maximized_horz');wait(maximized,'scaled maximized timeout origin')
     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750) and not maximized(),'scaled maximized timeout preview')
     wait(maximized,'automatic maximized preview revert',seconds=15)
     run('wmctrl','-ir',hex(int(window)),'-b','remove,maximized_vert,maximized_horz');wait(lambda:dimensions()==(1125,800),'scaled timeout retained normal size')
     run('wmctrl','-ir',hex(int(window)),'-b','add,maximized_vert,maximized_horz');wait(maximized,'unconfirmed maximized close origin')
     run('xdotool','key','--window',window,'F5');wait(lambda:dimensions()==(1000,750) and not maximized(),'unconfirmed maximized close preview')
     run('xdotool','key','alt+F4');assert app.wait(timeout=15)==0;app.communicate();app=None
     continue

    run('xdotool','key','--window',window,'F11');wait(lambda:color_is(window,(20,180,80)),'F11 restores fullscreen')
    run('xdotool','key','alt+F4');assert app.wait(timeout=15)==0;app.communicate();app=None
   print(suffix,target,'real port reports physical resizes/fullscreen at 100% and 150% UI scale and saved-fullscreen reopen.',flush=True)
 finally:
  if app is not None:
   if app.poll() is None:app.terminate()
   stdout,stderr=app.communicate(timeout=10)
   Path('/tmp/gamepkg-window-size-gui-stderr.log').write_bytes(stderr or b'')
  if wm is not None:wm.terminate();wm.wait(timeout=10)
  display.terminate();display.wait(timeout=10)
 from selenium import webdriver
 from selenium.webdriver.chrome.options import Options
 from selenium.webdriver.chrome.service import Service
 from selenium.webdriver.support.ui import WebDriverWait
 from selenium.webdriver.common.keys import Keys
 @contextmanager
 def browser_desktop():
  private_display=start_display(env,width=1400,height=1000);private_wm=None
  try:
   private_wm=subprocess.Popen(['metacity','--sm-disable','--replace','--no-composite'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);wait(lambda:'window id #' in run('xprop','-root','_NET_SUPPORTING_WM_CHECK'),'browser test window manager');yield
  finally:
   if private_wm is not None:private_wm.terminate();private_wm.wait(timeout=10)
   private_display.terminate();private_display.wait(timeout=10)
 with browser_desktop():
  for suffix in ['core','component','core-denied','component-denied']:
   allowed=not suffix.endswith('denied');output=root/(suffix+'-web');run(CLI,'web-export','com.gamepkg.window-'+suffix,'--output',output);server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();options.binary_location=shutil.which('chromium')
   for flag in ['--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader','--ozone-platform=x11','--window-size=1100,800','--use-gl=angle','--use-angle=swiftshader']:options.add_argument(flag)
   browser=webdriver.Chrome(service=Service(shutil.which('chromedriver'),env=env),options=options)
   try:
    browser.get(f'http://127.0.0.1:{server.server_port}/');waiter=WebDriverWait(browser,20);waiter.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();
    try:
     waiter.until(lambda b:b.find_element('id','game').get_attribute('data-state')==('2' if allowed else '1'))
    except Exception as error:
     proof=PROJECT/'target/window-observation-proof';proof.mkdir(parents=True,exist_ok=True);browser.save_screenshot(str(proof/(suffix+'-startup.png')))
     raise AssertionError((suffix,browser.find_element('id','status').text,browser.find_element('id','game').get_attribute('data-state'))) from error

    browser.execute_script("const button=document.createElement('button');button.id='observation-fullscreen';button.textContent='Test fullscreen';button.onclick=()=>document.querySelector('#game').requestFullscreen();document.body.append(button);")
    for exit_method in ['api','escape']:
     browser.find_element('id','observation-fullscreen').click();waiter.until(lambda b:b.execute_script('return document.fullscreenElement===document.querySelector("#game")'));waiter.until(lambda b:b.find_element('id','game').get_attribute('data-state')==('4' if allowed else '1'))
     if exit_method=='api':browser.execute_script('document.exitFullscreen()')
     else:
      chrome_window=run('xdotool','search','--onlyvisible','--name','GamePKG web player').splitlines()[0];run('xdotool','windowactivate','--sync',chrome_window);run('xdotool','key','Escape')
     waiter.until(lambda b:b.execute_script('return document.fullscreenElement===null'));waiter.until(lambda b:b.find_element('id','game').get_attribute('data-state')==('2' if allowed else '1'))
    browser.find_element('id','restart').click();waiter.until(lambda b:b.find_element('id','game').get_attribute('data-state')==('2' if allowed else '1'));print(suffix,'browser actual fullscreen/external exit/restart and permission state passed.',flush=True)
   finally:browser.quit();server.shutdown();server.server_close()
