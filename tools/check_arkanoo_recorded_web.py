"""Full private original-assets browser qualification; no artifacts are published."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,os,shutil,subprocess,tempfile,threading
from PIL import Image
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.common.action_chains import ActionChains
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.support.ui import WebDriverWait
parser=argparse.ArgumentParser();parser.add_argument('candidate',type=Path);parser.add_argument('--optimized-images',type=Path,required=True);args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-original-recorded-web-') as temporary:
 root=Path(temporary);(root/'key').write_text('73'*32);(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*parts):
  result=subprocess.run([str(project/'target/release/gamepkg'),*map(str,parts)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stdout+'\n'+result.stderr
 for profile in ['core-v1','component-v1']:
  recipe=root/(profile+'-source');shutil.copytree(args.candidate,recipe)
  for path in (project/'ports/arkanoo/assets').glob('*.png'):
   replacement=args.optimized_images/path.name
   with Image.open(path) as original,Image.open(replacement) as optimized:
    assert original.size==optimized.size and original.convert('RGBA').tobytes()==optimized.convert('RGBA').tobytes(),path.name
   shutil.copy2(replacement,recipe/'assets'/path.name)
  manifest=recipe/'manifest.toml';id='com.gamepkg.arkanoo-web-'+profile;manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.arkanoo',id))
  run('pack',recipe,'-o',root/(profile+'.gamepkg'),'--key',root/'key');run('install',root/(profile+'.gamepkg'),'--trust-creator');run('build',id,'--target','wasm');web=root/profile;run('web-export',id,'-o',web)
  metadata=json.loads((web/'game.json').read_text());encoded=sum(len(value if isinstance(value,str) else value['zlib'])//2 for value in metadata['assets'].values())
  assert encoded<=33554432 and len([name for name in metadata['assets'] if name.startswith('music/') and name.endswith('.mp3')])==9
  assert len(metadata['exact_png_images'])==76 and metadata['precise_images']=={}
  clips=[value for name,value in metadata['assets'].items() if name.endswith('.wav')];assert len(clips)==5 and all(isinstance(value,dict) for value in clips)
  scratch=max(value['length'] for value in clips)+263168;assert metadata['asset_scratch_bytes']==scratch
  print(profile,'complete dataset exported:',encoded,'bytes; all 76 images, five effects and nine tracks retained; scratch per side:',scratch,flush=True)
  server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(web)));threading.Thread(target=server.serve_forever,daemon=True).start()
  options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('chromium') or shutil.which('google-chrome-stable')
  for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--autoplay-policy=no-user-gesture-required']:options.add_argument(flag)
  options.set_capability('goog:loggingPrefs',{'browser':'ALL'});driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');browser=webdriver.Chrome(service=Service(driver),options=options)
  browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':'''window.inputTrace=[];window.frameTrace=[];const Base=Worker;window.Worker=class extends Base{constructor(...args){super(...args);this.addEventListener('message',({data})=>{if(data.type==='frame'){window.lastFrame={serial:data.serial,commands:data.commands.length,controls:data.recordedControls,window:data.window,balls:data.commands.filter(c=>c[0]==='linear-image'&&c[3]===6&&c[4]===6).map(c=>c.slice(0,5))};window.frameTrace.push(window.lastFrame);if(window.frameTrace.length>8)window.frameTrace.shift();}});}postMessage(data,...args){if(data.type==='tick'&&data.events?.length){window.inputTrace.push(data.events);if(window.inputTrace.length>20)window.inputTrace.shift();}return super.postMessage(data,...args);}};'''})
  try:
   browser.set_window_size(1100,1000);browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play')
   hooked=browser.execute_async_script('''const done=arguments[arguments.length-1];Promise.all([import('./recorded-session.js'),import('./progress-store.js')]).then(([{RecordedSession},{ProgressStore}])=>{const load=ProgressStore.prototype.load,commit=ProgressStore.prototype.commit;ProgressStore.prototype.load=async function(...args){const result=await load.apply(this,args);window.lastLoaded=Array.from(result.save);return result;};ProgressStore.prototype.commit=async function(save,...args){const result=await commit.call(this,save,...args);if(save)window.lastSaved=Array.from(save);return result;};const open=RecordedSession.open,snapshot=RecordedSession.prototype.snapshot;window.roots=[];window.states=[];RecordedSession.open=async(...args)=>{window.roots.push(args[4]);const session=await open(...args);window.guestBytes=session.guestBytes;return session;};RecordedSession.prototype.snapshot=async function(){const value=await snapshot.call(this);window.lastAudio=value;window.states.push(value);if(value.activeEffects)window.sawEffect=true;return value;};done(true);}).catch(error=>done(String(error)));''');assert hooked is True,hooked
   browser.find_element('id','play').click();wait.until(lambda b:b.execute_script('return window.lastAudio?.musicStatus==="playing"'))
   assert browser.execute_script('return window.guestBytes')==16777216-3145728-7340032-2*scratch
   assert browser.execute_script('return window.roots.at(-1).used')==16777216
   canvas=browser.find_element('id','game');canvas.send_keys(Keys.ENTER)
   def click(x,y):
    box=canvas.rect;ActionChains(browser).move_to_element_with_offset(canvas,(x-320)*box['width']/640,(y-240)*box['height']/480).click().perform()
   click(320,285);click(320,180);wait.until(lambda b:b.execute_script('return window.lastAudio?.musicStatus==="paused"'))
   played=browser.execute_script('return window.lastAudio.playedFrames');count=browser.execute_script('return window.states.length');wait.until(lambda b:b.execute_script('return window.states.length')>=count+3);assert browser.execute_script('return window.lastAudio.playedFrames')==played
   box=canvas.rect;ActionChains(browser).move_to_element_with_offset(canvas,0,-40*box['height']/480).click_and_hold().move_by_offset(-25*box['width']/640,0).release().perform()
   wait.until(lambda b:b.execute_script('return window.lastSaved?.length>=20&&((new DataView(new Uint8Array(window.lastSaved).buffer).getUint32(12,true)>>>10)&255)!==64'))
   selected_gain=browser.execute_script('return (new DataView(new Uint8Array(window.lastSaved).buffer).getUint32(12,true)>>>10)&255')
   click(320,450);click(320,185);wait.until(lambda b:b.execute_script('return window.lastFrame?.window?.confined===true'));ActionChains(browser).send_keys(Keys.SPACE).perform();wait.until(lambda b:b.execute_script('return window.sawEffect===true'))
   ActionChains(browser).send_keys(Keys.ESCAPE).perform();wait.until(lambda b:b.execute_script('return window.lastFrame?.window?.confined===false'))
   previous_session=browser.execute_script('return window.lastAudio.session');browser.find_element('id','restart').click();wait.until(lambda b:b.execute_script('return window.roots.length>=2&&window.lastAudio?.session!==arguments[0]&&window.lastAudio?.musicStatus==="paused"',previous_session))
   assert browser.execute_script('return window.roots.slice(0,-1).every(root=>root.used===0)')
   assert browser.execute_script('return (new DataView(new Uint8Array(window.lastLoaded).buffer).getUint32(12,true)>>>10)&255')==selected_gain
   canvas.send_keys(Keys.ENTER);click(320,285);click(320,180);wait.until(lambda b:b.execute_script('return window.lastAudio?.musicStatus==="playing"'))
   browser.find_element('id','stop').click();wait.until(lambda b:b.execute_script('return window.roots.every(root=>root.used===0)'))
   print(profile,'real original music, mute clock freeze, compressed collision effect, saved restart/resume and parent-memory release passed.',flush=True)
  except Exception:
   print('Full recorded web failure:',browser.find_element('id','status').text,browser.execute_script('return {input:window.inputTrace,frames:window.frameTrace,audio:window.lastAudio,state:document.querySelector("canvas").dataset.state}'),browser.get_log('browser'),flush=True);raise
  finally:browser.quit();server.shutdown();server.server_close()
