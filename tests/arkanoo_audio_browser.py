"""Actual WebAudio output from signed production mixer/physics methods.
Run after tests/arkanoo.py --keep-fixtures --runtime-only; pass that fixture folder.
Python/Selenium are maintainer test tools, never player dependencies.
"""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,struct,os,shutil,subprocess,tempfile,threading
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
parser=argparse.ArgumentParser();parser.add_argument('fixtures',type=Path);args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg'
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-mixer-browser-') as temporary:
 root=Path(temporary);env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'));(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n');key=root/'key';key.write_text('62'*32)
 def run(*arguments):
  result=subprocess.run([str(CLI),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stderr
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  source=root/suffix;shutil.copytree(args.fixtures/'modes',source);manifest=source/'manifest.toml';text=manifest.read_text();text=text.replace('com.gamepkg.arkanoo-modes','com.gamepkg.arkanoo-audio-'+suffix)
  if 'runtime_profile' in text:text=text.replace('runtime_profile = "core-v1"','runtime_profile = "'+profile+'"')
  else:text='runtime_profile="'+profile+'"\n'+text
  manifest.write_text(text);id='com.gamepkg.arkanoo-audio-'+suffix;package=root/(suffix+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build',id,'--target','wasm');output=root/(suffix+'-web');run('web-export',id,'--output',output)
  server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)));threading.Thread(target=server.serve_forever,daemon=True).start();options=Options();options.binary_location=shutil.which('chromium')
  for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--autoplay-policy=no-user-gesture-required']:options.add_argument(flag)
  browser=webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options)
  try:
   browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play')
   browser.execute_script("window.mixerCase=0;window.mixerBuffers=[];const post=Worker.prototype.postMessage;Worker.prototype.postMessage=function(data,...rest){if(data.type==='tick')data={...data,input:0xfb0000|window.mixerCase};return post.call(this,data,...rest);};const p=AudioContext.prototype,old=p.createBuffer;p.createBuffer=function(...args){const b=old.apply(this,args);window.mixerBuffers.push(b);if(window.mixerBuffers.length>60)window.mixerBuffers.shift();return b;};")
   browser.find_element('id','play').click()
   launch=struct.unpack('<4h',(PROJECT/'ports/arkanoo/assets/audio/launch.wav').read_bytes()[9644:9652]);bounce=struct.unpack('<4h',(PROJECT/'ports/arkanoo/assets/audio/bounce.wav').read_bytes()[9644:9652]);overlap=[max(-32768,min(32767,int(a/2)+int(b/2))) for a,b in zip(launch,bounce)]
   for case,wanted in [(0,[4600,-2600,4600,-2600]),(1,[11800,-11050,1800,-1050]),(3,[1800,-1050,1800,-1050]),(4,overlap),(5,overlap)]:
    browser.execute_script('window.mixerCase=arguments[0];window.mixerBuffers=[];',case)
    wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')==str(case+1))
    def matches(b):
     return b.execute_script('return window.mixerBuffers.some(b=>b.numberOfChannels===2&&b.length===800&&b.sampleRate===48000&&arguments[0].every((s,i)=>b.getChannelData(i%2)[Math.floor(i/2)]===s/32768));',wanted)
    wait.until(matches)
   print(profile,'actual signed Wasm/WebAudio: stereo tails, free-channel reuse, muted progress and keyboard/controller collision overlap passed.',flush=True)
   recordings=json.loads((PROJECT/'tests/fixtures/arkanoo-recorded-effects.json').read_text())['effects']
   for recording in recordings:
    which=recording['kind']-1
    browser.get(f'http://127.0.0.1:{server.server_port}/');wait.until(lambda b:b.find_element('id','status').text=='Ready to play')
    browser.execute_script("window.recordBuffers=[];window.firstRecording=true;const which=arguments[0],post=Worker.prototype.postMessage;Worker.prototype.postMessage=function(data,...rest){if(data.type==='tick'){data={...data,input:0xfc0000|which|(window.firstRecording?768:0)};window.firstRecording=false;}return post.call(this,data,...rest);};const p=AudioContext.prototype,old=p.createBuffer;p.createBuffer=function(...args){const b=old.apply(this,args);window.recordBuffers.push(b);return b;};",which)
    browser.find_element('id','play').click();wait.until(lambda b:b.execute_script('return window.recordBuffers.length>0'));wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')=='0')
    result=browser.execute_async_script("const frames=arguments[0],done=arguments[arguments.length-1],count=frames*2,buffers=window.recordBuffers;if(buffers.length!==Math.ceil(count/1600)){done({error:'buffer count',count:buffers.length});return;}const pcm=new Uint8Array(count*2),view=new DataView(pcm.buffer);let at=0;for(const b of buffers){if(b.numberOfChannels!==2||b.length!==800||b.sampleRate!==48000){done({error:'audio format'});return;}for(let i=0;i<b.length;i++)for(let c=0;c<2;c++){const sample=Math.round(b.getChannelData(c)[i]*32768);if(at<count)view.setInt16(at*2,sample,true);else if(sample!==0){done({error:'nonzero tail'});return;}at++;}}crypto.subtle.digest('SHA-256',pcm).then(hash=>done({sha256:Array.from(new Uint8Array(hash),v=>v.toString(16).padStart(2,'0')).join('')}));",recording['frames'])
    assert result=={'sha256':recording['pcm_sha256']},(profile,recording['asset'],result)
    print(profile,recording['asset'],'entire original PCM matches real WebAudio buffers, including stereo, duration and zero-padded ending.',flush=True)

  finally:browser.quit();server.shutdown();server.server_close()
