"""Actual signed compressed WAV reads/playback with scratch charged to the parent."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import json,os,shutil,subprocess,tempfile,threading,wave
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
project=Path(__file__).resolve().parents[1]
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-compressed-assets-') as temporary:
 root=Path(temporary);(root/'key').write_text('71'*32);(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*args):
  result=subprocess.run([str(project/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stdout+'\n'+result.stderr
 for profile in ['core-v1','component-v1']:
  recipe=root/(profile+'-source');shutil.copytree(project/'examples/minimal',recipe);id='com.gamepkg.compressed-'+profile
  manifest=recipe/'manifest.toml';manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.minimal',id).replace('audio = false','audio = true').replace('[limits]','[limits]\neffects_memory_bytes=3145728'))
  with wave.open(str(recipe/'assets/effect.wav'),'wb') as output:
   output.setnchannels(2);output.setsampwidth(2);output.setframerate(44100);output.writeframes(bytes([57,48,96,164])*234253)
  original=(recipe/'assets/effect.wav').read_bytes()
  source='''pub fn game_tick(_:u32,state:u64)->u64 {
   if state==0 {let mut block=[0u8;65536];let mut offset=0u32;let mut sum=0u64;
    loop {let Some(count)=gamepkg::asset_read("effect.wav",offset,&mut block) else {return 2;};if count==0 {break;}for byte in &block[..count as usize] {sum+=u64::from(*byte);}offset+=count;}
    if offset!=EXPECTED_LENGTH || sum!=EXPECTED_SUM || !gamepkg::effect_play("effect.wav") {gamepkg::draw_clear(0xff0000ff);return 2;}
   }
   gamepkg::draw_clear(0x00ff00ff);1
  }'''.replace('EXPECTED_LENGTH',str(len(original))).replace('EXPECTED_SUM',str(sum(original)))
  (recipe/'source/src/lib.rs').write_text(source)
  run('pack',recipe,'-o',root/(profile+'.gamepkg'),'--key',root/'key');run('install',root/(profile+'.gamepkg'),'--trust-creator');run('build',id,'--target','wasm');web=root/profile;run('web-export',id,'-o',web)
  metadata=json.loads((web/'game.json').read_text());descriptor=metadata['assets']['effect.wav'];assert descriptor['length']==len(original);scratch=len(original)+263168;assert metadata['asset_scratch_bytes']==scratch
  module=(web/'game.wasm').read_bytes();cursor=8;maxima=[]
  def leb():
   global cursor
   value=0;shift=0
   while True:
    byte=module[cursor];cursor+=1;value|=(byte&127)<<shift
    if byte<128:return value
    shift+=7
  while cursor<len(module):
   section=module[cursor];cursor+=1;length=leb();end=cursor+length
   if section==5:
    for _ in range(leb()):
     flags=leb();minimum=leb();maximum=leb() if flags&1 else None;maxima.append(maximum)
   cursor=end
  assert maxima and all(value==(16777216-3145728-2*scratch)//65536 for value in maxima),maxima
  server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(web)));threading.Thread(target=server.serve_forever,daemon=True).start()
  options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('chromium') or shutil.which('google-chrome-stable')
  for flag in ['--headless=new','--mute-audio','--no-sandbox','--disable-dev-shm-usage','--autoplay-policy=no-user-gesture-required']:options.add_argument(flag)
  options.set_capability('goog:loggingPrefs',{'browser':'ALL'});driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');browser=webdriver.Chrome(service=Service(driver),options=options)
  try:
   # Keep only timing and bounded framing metadata; never copy WAV, guest-memory or frame data.
   browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':'''window.assetWorkerTrace=[];window.assetFirstTicks=[];let assetWorkerId=0;const Base=Worker;window.Worker=class extends Base{constructor(...args){super(...args);this.assetTraceName=new URL(String(args[0]),location.href).pathname.split('/').pop();this.assetTraceId=++assetWorkerId;this.assetTickAt=null;this.addEventListener('message',({data})=>{if(this.assetTraceName!=='worker.js')return;const now=performance.now();if(['ready','frame','error'].includes(data.type)){const row={id:this.assetTraceId,direction:'received',type:data.type,at:now,elapsed:this.assetTickAt===null?null:now-this.assetTickAt,serial:Number.isSafeInteger(data.serial)?data.serial:null,epoch:Number.isSafeInteger(data.epoch)?data.epoch:null,state:typeof data.state==='string'?data.state.slice(0,24):null};window.assetWorkerTrace.push(row);if(window.assetWorkerTrace.length>48)window.assetWorkerTrace.shift();if(data.type==='frame'&&data.serial===1){window.assetFirstTicks.push(row);if(window.assetFirstTicks.length>8)window.assetFirstTicks.shift();}}});}postMessage(data,...args){if(this.assetTraceName==='worker.js'&&['start','tick','restart','window-commit'].includes(data.type)){const now=performance.now();if(data.type==='tick')this.assetTickAt=now;window.assetWorkerTrace.push({id:this.assetTraceId,direction:'sent',type:data.type,at:now,epoch:Number.isSafeInteger(data.epoch)?data.epoch:null,serial:Number.isSafeInteger(data.serial)?data.serial:null});if(window.assetWorkerTrace.length>48)window.assetWorkerTrace.shift();}return super.postMessage(data,...args);}terminate(){if(this.assetTraceName==='worker.js'){window.assetWorkerTrace.push({id:this.assetTraceId,direction:'terminated',at:performance.now(),tickAge:this.assetTickAt===null?null:performance.now()-this.assetTickAt});if(window.assetWorkerTrace.length>48)window.assetWorkerTrace.shift();}return super.terminate();}};'''})
   # Exercise cold processor readiness independently of a fast local mixer compile.
   # Only the initial acknowledgement is delayed; ordinary requests stay untouched.
   browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':'''window.assetOutputStartup=[];const NativeOutput=window.AudioWorkletNode,onmessage=Object.getOwnPropertyDescriptor(MessagePort.prototype,'onmessage');window.AudioWorkletNode=new Proxy(NativeOutput,{construct(target,args,newTarget){const node=Reflect.construct(target,args,newTarget),at=performance.now();let first=true;Object.defineProperty(node.port,'onmessage',{configurable:true,get(){return onmessage.get.call(this);},set(handler){onmessage.set.call(this,event=>{if(first){first=false;const received=performance.now();setTimeout(()=>{window.assetOutputStartup.push({received:received-at,delivered:performance.now()-at});if(window.assetOutputStartup.length>8)window.assetOutputStartup.shift();handler.call(this,event);},400);}else handler.call(this,event);});}});return node;}});'''})
   browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play')
   hooked=browser.execute_async_script('''const done=arguments[arguments.length-1];import('./recorded-session.js').then(({RecordedSession})=>{const open=RecordedSession.open,snapshot=RecordedSession.prototype.snapshot;RecordedSession.open=async(...args)=>{window.parentBudget=args[4];const session=await open(...args);window.guestBytes=session.guestBytes;return session;};RecordedSession.prototype.snapshot=async function(){const result=await snapshot.call(this);if(result.activeEffects)window.effectPlayed=true;return result;};done(true);}).catch(error=>done(String(error)));''');assert hooked is True,hooked
   browser.find_element('id','play').click();wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')=='1');wait.until(lambda b:b.execute_script('return window.effectPlayed===true'))
   assert browser.execute_script('return window.guestBytes')==16777216-3145728-2*scratch
   assert browser.execute_script('return window.parentBudget.used')==16777216
   browser.find_element('id','stop').click();wait.until(lambda b:b.execute_script('return window.parentBudget.used===0'))
   gated=browser.execute_async_script('''const done=arguments[arguments.length-1];Promise.all([import('./recorded-output-client.js'),import('./asset-store.js')]).then(([{RecordedOutputClient},{PackageAssets}])=>{window.originalLoad=RecordedOutputClient.prototype.loadWav;const borrow=PackageAssets.prototype.borrow;PackageAssets.prototype.borrow=function(...args){window.borrowedAssets=this;return borrow.apply(this,args);};RecordedOutputClient.prototype.loadWav=async function(...args){window.loanEntered=true;await new Promise(resolve=>window.releaseLoan=resolve);return window.originalLoad.apply(this,args);};window.outputType=RecordedOutputClient;window.effectPlayed=false;done(true);}).catch(error=>done(String(error)));''');assert gated is True,gated
   browser.find_element('id','play').click();wait.until(lambda b:b.execute_script('return window.loanEntered===true'))
   assert browser.execute_script('return window.borrowedAssets.allocatedScratchBytes')==scratch
   browser.find_element('id','stop').click();wait.until(lambda b:b.execute_script('return window.borrowedAssets.closed'))
   assert browser.execute_script('return window.parentBudget.used')==3145728+scratch
   assert browser.execute_script('return window.borrowedAssets.allocatedScratchBytes')==scratch
   browser.execute_script('window.releaseLoan()');wait.until(lambda b:b.execute_script('return window.parentBudget.used===0'))
   assert browser.execute_script('return window.borrowedAssets.allocatedScratchBytes')==0
   browser.execute_script('window.outputType.prototype.loadWav=window.originalLoad;window.effectPlayed=false;')
   browser.find_element('id','play').click();wait.until(lambda b:b.execute_script('return window.effectPlayed===true'));browser.find_element('id','stop').click();wait.until(lambda b:b.execute_script('return window.parentBudget.used===0'))
   startup=browser.execute_script('return window.assetOutputStartup');assert len(startup)==3 and all(row['delivered']>=400 for row in startup),startup
   print(profile,'initial output acknowledgements (ms):',startup,flush=True)
   print(profile,'full original WAV reads/playback, guest/parent ceilings, pending-loan close retention, complete release and fresh recovery passed.',flush=True)
   print(profile,'first-tick round-trip timings:',browser.execute_script('return window.assetFirstTicks'),flush=True)
  except Exception:
   diagnostic={'status':browser.find_element('id','status').text[:2048],
    'browser_version':browser.capabilities.get('browserVersion'),
    'audio_startup':browser.execute_script('return window.assetOutputStartup'),
    'trace':browser.execute_script('return {events:window.assetWorkerTrace,firstTicks:window.assetFirstTicks}'),
    'console':[{'level':row.get('level'),'message':row.get('message','')[:2048]} for row in browser.get_log('browser')[-8:]]}
   print('Compressed asset failure:',json.dumps(diagnostic),flush=True)
   (project/'target/browser-compressed-assets-failure.json').write_text(json.dumps(diagnostic,indent=2))
   raise
  finally:browser.quit();server.shutdown();server.server_close()
