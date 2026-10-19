"""Generated clock bindings in signed native/core/component builds and real browsers."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import json,os,shutil,subprocess,tempfile,threading
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg'
compiled=subprocess.run(['cargo','test','--release','--test','frame_clock_runtime','--no-run','--message-format=json'],capture_output=True,text=True,timeout=300);assert compiled.returncode==0,compiled.stderr
executables=[v['executable'] for line in compiled.stdout.splitlines() if (v:=json.loads(line)).get('reason')=='compiler-artifact' and v.get('target',{}).get('name')=='frame_clock_runtime' and v.get('executable')];assert len(executables)==1
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-frame-clock-') as temporary:
 root=Path(temporary);env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'));(root/'settings.toml').write_text('automatic_update_checks=false\n');key=root/'key';key.write_text('48'*32)
 def run(*args):
  value=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert value.returncode==0,value.stderr
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  source=root/suffix;shutil.copytree(PROJECT/'examples/minimal',source);manifest=source/'manifest.toml';manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.minimal','com.gamepkg.clock-'+suffix).replace('input = false','input = true'))
  (source/'source/src/lib.rs').write_text('pub fn game_tick(input:u32,_:u64)->u64 {if input&128!=0 {loop {gamepkg::frame_time_us();}}let now=gamepkg::frame_time_us();for _ in 0..64 {if gamepkg::frame_time_us()!=now {return u64::MAX;}}(now<<1)|1}')
  package=root/(suffix+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.gamepkg.clock-'+suffix,'--target','wasm')
 test_env=dict(env,GAMEPKG_FRAME_CLOCK_ROOT=str(root/'library'));subprocess.run([executables[0],'--ignored'],env=test_env,check=True,timeout=120)
 run('build','com.gamepkg.clock-core','--target','native');subprocess.run([executables[0],'--ignored'],env=test_env,check=True,timeout=120)
 from selenium import webdriver
 from selenium.webdriver.chrome.options import Options
 from selenium.webdriver.chrome.service import Service
 from selenium.webdriver.support.ui import WebDriverWait
 for suffix in ['core','component']:
  output=root/(suffix+'-web');run('web-export','com.gamepkg.clock-'+suffix,'--output',output);server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();options.binary_location=shutil.which('chromium')
  for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage']:options.add_argument(flag)
  browser=webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options)
  try:
   browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click()
   def sample():
    text=browser.find_element('id','game').get_attribute('data-state');return int(text) if text else 0
   wait.until(lambda b:sample()>1);first=sample();assert first!=2**64-1 and first&1
   wait.until(lambda b:sample()>first);assert sample()!=2**64-1
   browser.execute_async_script("const done=arguments[arguments.length-1],c=document.querySelector('#game'),o=new MutationObserver(()=>{if(c.dataset.state==='1'){o.disconnect();done(true);}});o.observe(c,{attributes:true,attributeFilter:['data-state']});document.querySelector('#restart').click();")
   wait.until(lambda b:sample()>1);assert sample()!=2**64-1
   print(suffix,'browser clock advances across ticks and restarts with a new origin.',flush=True)
  finally:browser.quit();server.shutdown();server.server_close()
