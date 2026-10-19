"""Signed generated SDK stereo bindings in native, core and component runtimes."""
from pathlib import Path
import json,os,shutil,subprocess,tempfile,threading
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg'
compiled=subprocess.run(['cargo','test','--release','--test','stereo_audio_sdk','--no-run','--message-format=json'],capture_output=True,text=True,timeout=300)
assert compiled.returncode==0,compiled.stderr
executables=[v['executable'] for line in compiled.stdout.splitlines() if (v:=json.loads(line)).get('reason')=='compiler-artifact' and v.get('target',{}).get('name')=='stereo_audio_sdk' and v.get('executable')];assert len(executables)==1
with tempfile.TemporaryDirectory(prefix='gamepkg-stereo-') as temporary:
 root=Path(temporary);env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'));(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n');key=root/'key';key.write_text('48'*32)
 def run(*args):
  value=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert value.returncode==0,value.stderr
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  for enabled in [True,False]:
   id='com.gamepkg.stereo-'+suffix+('-on' if enabled else '-off');source=root/id;shutil.copytree(PROJECT/'examples/minimal',source);manifest=source/'manifest.toml';manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.minimal',id).replace('audio = false','audio = '+str(enabled).lower()))
   (source/'source/src/lib.rs').write_text('pub fn game_tick(_:u32,_:u64)->u64 {let mut result=0;if gamepkg::audio_submit(&[]) {result|=1;}if gamepkg::audio_submit_stereo(&[32767,-32768,16384,-16384]) {result|=2;}if !gamepkg::audio_submit(&[9]) {result|=4;}if !gamepkg::audio_submit_stereo(&[9]) {result|=8;}if !gamepkg::audio_submit_stereo(&[0;1602]) {result|=16;}result}')
   package=root/(id+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build',id,'--target','wasm')
 test_env=dict(env,GAMEPKG_STEREO_ROOT=str(root/'library'));subprocess.run([executables[0],'--ignored'],env=test_env,check=True,timeout=120)
 for enabled in [True,False]:run('build','com.gamepkg.stereo-core-'+('on' if enabled else 'off'),'--target','native')
 subprocess.run([executables[0],'--ignored'],env=test_env,check=True,timeout=120)
 print('Signed stereo SDK: native/core/component channels, format, malformed length, permission and per-tick reset passed.')

 # Browser exports run the same signed source through actual Wasm and WebAudio.
 from selenium import webdriver
 from selenium.webdriver.chrome.options import Options
 from selenium.webdriver.chrome.service import Service
 from selenium.webdriver.support.ui import WebDriverWait
 class Quiet(SimpleHTTPRequestHandler):
  def log_message(self,*args):pass
 for suffix in ['core','component']:
  output=root/(suffix+'-web');run('web-export','com.gamepkg.stereo-'+suffix+'-on','--output',output)
  server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();options.binary_location=shutil.which('chromium')
  for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--autoplay-policy=no-user-gesture-required']:options.add_argument(flag)
  browser=webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options)
  try:
   browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play')
   browser.execute_script("window.stereoBuffers=[];const p=AudioContext.prototype,old=p.createBuffer;p.createBuffer=function(...args){const b=old.apply(this,args);window.stereoBuffers.push(b);return b;};")
   browser.find_element('id','play').click()
   wait.until(lambda b:b.execute_script('return window.stereoBuffers.length>0'))
   values=browser.execute_script('const b=window.stereoBuffers[0];return [b.numberOfChannels,b.length,b.sampleRate,...b.getChannelData(0),...b.getChannelData(1)];')
   assert values==[2,2,48000,32767/32768,.5,-1,-.5],values
   assert browser.find_element('id','game').get_attribute('data-state')=='31'
   print(suffix,'actual browser Wasm and WebAudio preserve stereo channel data.',flush=True)
  finally:browser.quit();server.shutdown();server.server_close()
