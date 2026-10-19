"""Delayed storage completion must not revive stopped or restarted games."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import os,shutil,subprocess,tempfile,threading
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
project=Path(__file__).resolve().parents[1];cli=project/'target/release/gamepkg'
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-progress-lifecycle-') as temporary:
 root=Path(temporary);(root/'key').write_text('35'*32);(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*args):
  result=subprocess.run([str(cli),*map(str,args)],env=env,capture_output=True,text=True,timeout=180)
  assert result.returncode==0,result.stderr
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  source=root/suffix;shutil.copytree(project/'examples/minimal',source);manifest=source/'manifest.toml';id='com.gamepkg.progress-lifecycle-'+suffix
  manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.minimal',id).replace('save = false','save = true').replace('audio = false','audio = true'))
  (source/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,state:u64)->u64 {gamepkg::save_write(&[state as u8]);gamepkg::audio_submit_stereo(&[12345i16;1600]);gamepkg::draw_clear(if state&1==0 {0x193249ff}else{0x824029ff});state+1}''')
  archive=root/(suffix+'.gamepkg');run('pack',source,'-o',archive,'--key',root/'key');run('install',archive,'--trust-creator');run('build',id,'--target','wasm')
  output=root/(suffix+'-web');run('web-export',id,'--output',output)
  server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start()
  options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert options.binary_location
  for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--autoplay-policy=no-user-gesture-required']:options.add_argument(flag)
  driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
  browser=webdriver.Chrome(service=Service(driver),options=options)
  browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':'''window.workerSerial=0;window.lifecycleFrames=[];window.lifecycleCommits=[];window.audioBufferCount=0;
   const buffer=AudioContext.prototype.createBuffer;AudioContext.prototype.createBuffer=function(...args){window.audioBufferCount++;return buffer.apply(this,args);};
   const Original=Worker;window.Worker=class extends Original{constructor(...args){super(...args);const instance=++window.workerSerial;this.addEventListener('message',event=>{const m=event.data;if(m.type==='frame'){window.lifecycleFrames.push({instance,epoch:m.epoch,serial:m.serial,state:m.state});if(window.lifecycleFrames.length>64)window.lifecycleFrames.shift();}});const send=this.postMessage.bind(this);this.postMessage=(m,...rest)=>{if(m.type==='window-commit'){window.lifecycleCommits.push({...m,instance});if(window.lifecycleCommits.length>64)window.lifecycleCommits.shift();}return send(m,...rest);};}};'''})
  try:
   browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play')
   result=browser.execute_async_script('''const done=arguments[arguments.length-1];import('./progress-store.js').then(({ProgressStore})=>{window.storageClass=ProgressStore;window.originalLoad=ProgressStore.prototype.load;window.loadGate=new Promise(resolve=>window.releaseLoad=resolve);ProgressStore.prototype.load=function(...args){window.loadEntered=true;return window.loadGate.then(()=>window.originalLoad.apply(this,args)).finally(()=>window.loadFinished=true);};done(true);}).catch(error=>done(String(error)));''');assert result is True,result
   browser.find_element('id','play').click();wait.until(lambda b:b.execute_script('return !!window.loadEntered'))
   browser.find_element('id','stop').click();browser.execute_script('window.releaseLoad()');wait.until(lambda b:b.execute_script('return !!window.loadFinished'))
   assert browser.execute_script('return window.workerSerial===0&&window.audioBufferCount===0')
   assert browser.find_element('id','status').text=='Stopped'
   browser.execute_script('window.storageClass.prototype.load=window.originalLoad')
   def start():
    prior=browser.execute_script('return window.workerSerial');browser.find_element('id','play').click()
    wait.until(lambda b:b.execute_script('return window.workerSerial>arguments[0]&&window.lifecycleCommits.some(c=>c.instance===window.workerSerial&&c.success)',prior))
   def hold_commit():
    browser.execute_script('''window.originalCommit=window.storageClass.prototype.commit;window.commitGate=new Promise(resolve=>window.releaseCommit=resolve);window.commitStored=false;window.commitReleased=false;let first=true;
     window.storageClass.prototype.commit=async function(...args){const hold=first;first=false;const result=await window.originalCommit.apply(this,args);if(hold){window.commitStored=true;window.heldFrame=window.lifecycleFrames.at(-1);await window.commitGate;window.commitReleased=true;}return result;};''')
    wait.until(lambda b:b.execute_script('return window.commitStored'))
   start();hold_commit()
   before=browser.execute_script('return {state:document.querySelector("#game").dataset.state,audio:window.audioBufferCount,instance:window.workerSerial}')
   browser.find_element('id','stop').click();browser.execute_script('window.releaseCommit()');wait.until(lambda b:b.execute_script('return window.commitReleased'))
   assert browser.execute_script('return document.querySelector("#game").dataset.state===arguments[0].state&&window.audioBufferCount===arguments[0].audio&&window.workerSerial===arguments[0].instance',before)
   assert browser.find_element('id','status').text=='Stopped'
   assert browser.execute_script('const f=window.heldFrame;return !window.lifecycleCommits.some(c=>c.instance===f.instance&&c.epoch===f.epoch&&c.serial===f.serial)')
   browser.execute_script('window.storageClass.prototype.commit=window.originalCommit')
   start();hold_commit();browser.find_element('id','restart').click()
   wait.until(lambda b:b.execute_script('return window.lifecycleCommits.some(c=>c.instance===window.workerSerial&&c.epoch===1&&c.success)'))
   baseline=browser.execute_script('return window.lifecycleCommits.filter(c=>c.instance===window.workerSerial&&c.epoch===1).at(-1).serial')
   browser.execute_script('window.releaseCommit()');wait.until(lambda b:b.execute_script('return window.commitReleased'))
   wait.until(lambda b:b.execute_script('return window.lifecycleCommits.some(c=>c.instance===window.workerSerial&&c.epoch===1&&c.serial>=arguments[0]+2&&c.success)',baseline))
   assert browser.execute_script('const held=window.heldFrame,last=window.lifecycleCommits.filter(c=>c.instance===window.workerSerial&&c.epoch===1).at(-1),frame=window.lifecycleFrames.find(f=>f.instance===last.instance&&f.epoch===last.epoch&&f.serial===last.serial);return !window.lifecycleCommits.some(c=>c.instance===held.instance&&c.epoch===held.epoch&&c.serial===held.serial)&&document.querySelector("#game").dataset.state===frame.state')
   assert browser.find_element('id','status').text=='Playing'
   browser.execute_script('window.storageClass.prototype.commit=window.originalCommit')
   print(suffix,'delayed startup cannot revive Stop; delayed checkpoint cannot publish after Stop/Restart; fresh generations continue.',flush=True)
  finally:browser.quit();server.shutdown();server.server_close()
