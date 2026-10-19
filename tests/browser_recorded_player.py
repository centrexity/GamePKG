"""Signed game SDK -> exported player -> actual recorded AudioWorklet."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import json,os,shutil,subprocess,tempfile,threading,wave
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
project=Path(__file__).resolve().parents[1];cli=project/'target/release/gamepkg'
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-recorded-player-') as temporary:
 root=Path(temporary);(root/'key').write_text('46'*32);(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*args):
  result=subprocess.run([str(cli),*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stderr
 for profile,suffix in [('core-v1','core'),('component-v1','component')]:
  recipe=root/suffix;shutil.copytree(project/'examples/minimal',recipe);id='com.gamepkg.recorded-player-'+suffix
  manifest=(recipe/'manifest.toml').read_text().replace('com.gamepkg.minimal',id).replace('audio = false','audio = true').replace('save = false','save = true').replace('[limits]','[limits]\neffects_memory_bytes=4194304\nmusic_memory_bytes=8388608')
  (recipe/'manifest.toml').write_text('runtime_profile="'+profile+'"\n'+manifest)
  with wave.open(str(recipe/'assets/tone.wav'),'wb') as output:
   output.setnchannels(2);output.setsampwidth(2);output.setframerate(44100);output.writeframes(b'\x39\x30\x60\xa4'*10000)
  subprocess.run(['ffmpeg','-v','error','-i',str(recipe/'assets/tone.wav'),'-codec:a','libmp3lame','-write_xing','0',str(recipe/'assets/tone.mp3')],check=True)
  (recipe/'source/src/lib.rs').write_text('''pub fn game_tick(input:u32,state:u64)->u64 {
   if state==0 || input&1!=0 {if !gamepkg::effect_play("tone.wav") || !gamepkg::music_play("tone.mp3") {gamepkg::draw_clear(0xff0000ff);return 0;}}
   gamepkg::save_write(&[(state%256) as u8]);
   gamepkg::draw_clear(if gamepkg::music_failed() {0xff00ffff}else if gamepkg::music_finished() {0x00ff00ff}else{0x193249ff});state+1
  }''')
  archive=root/(suffix+'.gamepkg');run('pack',recipe,'-o',archive,'--key',root/'key');run('install',archive,'--trust-creator');run('build',id,'--target','wasm');web=root/(suffix+'-web');run('web-export',id,'-o',web)
  metadata=json.loads((web/'game.json').read_text());assert metadata['recorded_audio']['converter_sha256'];assert metadata['recorded_audio']['decoder_sha256']
  # The actual exported guest must be capped to 16 - 4 - 8 = 4 MiB.
  module=(web/'game.wasm').read_bytes();at=8
  def uint():
   global at
   value=0;shift=0
   while True:
    byte=module[at];at+=1;value|=(byte&127)<<shift
    if byte<128:return value
    shift+=7
  memories=[]
  while at<len(module):
   section=module[at];at+=1;size=uint();end=at+size
   if section==5:
    for _ in range(uint()):
     flags=uint();minimum=uint();maximum=uint() if flags&1 else None;memories.append((minimum,maximum))
   at=end
  assert memories and all(maximum==64 for _,maximum in memories),memories
  server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(web)));threading.Thread(target=server.serve_forever,daemon=True).start()
  options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert options.binary_location
  for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--autoplay-policy=no-user-gesture-required']:options.add_argument(flag)
  options.set_capability('goog:loggingPrefs',{'browser':'ALL'})
  driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');browser=webdriver.Chrome(service=Service(driver),options=options)
  browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':"window.gameWorkers=0;const Base=Worker;window.Worker=class extends Base{constructor(url,...args){super(url,...args);if(String(url)==='worker.js')window.gameWorkers++;if(String(url).includes('recorded-decoder-worker.js'))window.decoderWorkers=(window.decoderWorkers||0)+1;}};"})
  try:
   browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play')
   hooked=browser.execute_async_script('''const done=arguments[arguments.length-1];Promise.all([import('./recorded-session.js'),import('./progress-store.js')]).then(([{RecordedSession},{ProgressStore}])=>{window.audioFrames=[];window.audioFinishes=[];window.storageClass=ProgressStore;window.originalCommit=ProgressStore.prototype.commit;const prepare=RecordedSession.prototype.prepare,finish=RecordedSession.prototype.finish,snapshot=RecordedSession.prototype.snapshot;RecordedSession.prototype.snapshot=function(){return snapshot.call(this).then(value=>{window.lastAudioState=value;return value;});};RecordedSession.prototype.prepare=function(frame){window.activeRecordedSession=this;if(frame.recordedControls.length)window.audioFrames.push({serial:frame.serial,epoch:frame.epoch,commands:frame.recordedControls});return prepare.call(this,frame);};RecordedSession.prototype.finish=function(epoch,serial,success){window.audioFinishes.push({epoch,serial,success});return finish.call(this,epoch,serial,success);};done(true);}).catch(error=>done(String(error)));''');assert hooked is True,hooked
   browser.find_element('id','play').click()
   green='const c=document.querySelector("#game"),p=c.getContext("2d").getImageData(320,240,1,1).data;return p[0]===0&&p[1]===255&&p[2]===0'
   wait.until(lambda b:b.execute_script(green));assert browser.find_element('id','status').text=='Playing'
   before=browser.execute_script('return window.decoderWorkers')
   browser.execute_script('window.audioFrames=[];window.audioFinishes=[];window.storageClass.prototype.commit=function(){return Promise.reject(Error("fixture quota"));};const c=document.querySelector("#game");c.dispatchEvent(new KeyboardEvent("keydown",{key:"ArrowLeft",code:"ArrowLeft"}));c.dispatchEvent(new KeyboardEvent("keyup",{key:"ArrowLeft",code:"ArrowLeft"}));')
   wait.until(lambda b:b.execute_script('return window.audioFrames.some(f=>f.commands.some(c=>c[0]===4)&&window.audioFinishes.some(r=>r.serial===f.serial&&!r.success))'))
   def audio_state():
    result=browser.execute_script('return window.lastAudioState');assert result is not None;return result
   failed=audio_state();assert failed['musicStatus']=='ended' and failed['generation']==1,failed
   assert browser.execute_script('return window.decoderWorkers')==before,'Failed checkpoint started a decoder'
   browser.execute_script('window.storageClass.prototype.commit=window.originalCommit')
   wait.until(lambda b:b.find_element('id','status').text=='Playing');assert browser.execute_script('return window.decoderWorkers')==before,'Rejected Play replayed after recovery'
   browser.execute_script('const c=document.querySelector("#game");c.dispatchEvent(new KeyboardEvent("keydown",{key:"ArrowLeft",code:"ArrowLeft"}));c.dispatchEvent(new KeyboardEvent("keyup",{key:"ArrowLeft",code:"ArrowLeft"}));')
   wait.until(lambda b:b.execute_script('return window.decoderWorkers>arguments[0]',before));wait.until(lambda b:(lambda state:state['generation']==2 and state['musicStatus']=='ended')(audio_state()))
   previous=browser.execute_script('return window.gameWorkers');browser.find_element('id','restart').click();wait.until(lambda b:b.execute_script('return window.gameWorkers>arguments[0]&&document.querySelector("#game").dataset.state!==undefined',previous));wait.until(lambda b:b.execute_script(green));assert browser.find_element('id','status').text=='Playing'
   browser.find_element('id','stop').click();assert browser.find_element('id','status').text=='Stopped';previous=browser.execute_script('return window.gameWorkers');browser.find_element('id','play').click();wait.until(lambda b:b.execute_script('return window.gameWorkers>arguments[0]&&document.querySelector("#game").dataset.state!==undefined',previous));wait.until(lambda b:b.execute_script(green));assert browser.find_element('id','status').text=='Playing';browser.find_element('id','stop').click()
   original=(web/'audio-converter.wasm').read_bytes();damaged=bytearray(original);damaged[-1]^=1;(web/'audio-converter.wasm').write_bytes(damaged)
   browser.refresh();wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();wait.until(lambda b:'integrity check failed' in b.find_element('id','status').text);assert browser.execute_script('return window.gameWorkers')==0,'Corrupt companion started a guest worker'
   (web/'audio-converter.wasm').write_bytes(original);browser.find_element('id','play').click();wait.until(lambda b:b.execute_script('return window.gameWorkers>0&&document.querySelector("#game").dataset.state!==undefined'));wait.until(lambda b:b.execute_script(green));assert browser.find_element('id','status').text=='Playing'
   previous=browser.execute_script('window.savedContextClose=AudioContext.prototype.close;AudioContext.prototype.close=function(){window.failedCloseContext=this;return Promise.reject(Error("fixture close failure"));};return window.gameWorkers')
   browser.find_element('id','stop').click();wait.until(lambda b:b.execute_script('return !!window.failedCloseContext'));browser.find_element('id','play').click();wait.until(lambda b:b.find_element('id','status').text=='Unable to load progress or finish the previous audio session.');assert browser.execute_script('return window.gameWorkers')==previous,'Failed audio close allowed a new game allocation'
   cleaned=browser.execute_async_script('const done=arguments[arguments.length-1];AudioContext.prototype.close=window.savedContextClose;window.savedContextClose.call(window.failedCloseContext).then(()=>done(true)).catch(error=>done(String(error)));');assert cleaned is True,cleaned
   print(profile,'signed recorded SDK exports cap guest to 4 MiB; real player reaches music completion, restarts, stops and relaunches.',flush=True)
  except Exception:
   print('Recorded player failure:',browser.find_element('id','status').text,browser.get_log('browser'),flush=True);raise
  finally:browser.quit();server.shutdown();server.server_close()
