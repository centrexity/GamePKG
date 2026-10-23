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
parser=argparse.ArgumentParser();parser.add_argument('candidate',type=Path);parser.add_argument('--optimized-images',type=Path,required=True);parser.add_argument('--unicode-name',help='Repeat this UTF-8 sample in the maximum-size active name');parser.add_argument('--selector-run',action='store_true',help='Use one maximum-length identical-selector run');parser.add_argument('--mixed-selector-run',action='store_true',help='Use one maximum-length alternating-selector cluster');parser.add_argument('--missing-mark-run',action='store_true',help='Use one maximum-size unconsumed combining-mark cluster');parser.add_argument('--mixed-missing-mark-run',action='store_true',help='Use a maximum same-class mixed missing-mark cluster');parser.add_argument('--mixed-class-missing-mark-run',action='store_true',help='Qualify one maximum-size mixed-class missing-mark cluster');parser.add_argument('--late-composing-mark-run',action='store_true',help='Qualify a maximum-size S name with a late composing cedilla');parser.add_argument('--playlist-case',choices=['empty','invalid-utf8','invalid-paths'],help='Verify recorded effects survive unusable packaged music');parser.add_argument('--corrupt-saves',action='store_true',help='Also verify actual IndexedDB retains malformed ARK1-ARK4 records');parser.add_argument('--corrupt-save-repeats',type=int,choices=range(1,4),default=1,help='Repeat the complete six-record restart matrix for diagnostics');args=parser.parse_args();assert not(args.corrupt_saves and args.playlist_case),'Choose corruption or playlist qualification separately'
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
  if args.playlist_case:(recipe/'assets/audio/music-list.txt').write_bytes({'empty':b'', 'invalid-utf8':b'\xff', 'invalid-paths':b'../outside.mp3\nmusic/../outside.mp3\nmusic/file.wav\n'}[args.playlist_case])
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
  browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':'''window.decoderTrace=[];const NativeWorker=Worker;window.Worker=class extends NativeWorker{constructor(...args){super(...args);if(String(args[0]).includes('recorded-decoder-worker')){this.decoderRecord={kind:'startup',sentAt:performance.now(),encodedBytes:0};this.addEventListener('message',({data})=>{if(data?.ok===false){window.decoderTrace.push({...this.decoderRecord,error:data.error,elapsed:performance.now()-this.decoderRecord.sentAt});if(window.decoderTrace.length>40)window.decoderTrace.shift();}});}}postMessage(data,...args){if(this.decoderRecord){this.decoderRecord.kind=data.kind;this.decoderRecord.sentAt=performance.now();if(data.kind==='reserve'){this.decoderRecord.encodedBytes=data.length;this.decoderRecord.reserveAt=performance.now();this.decoderRecord.inputChunks=0;}if(data.kind==='input')this.decoderRecord.inputChunks++;}return super.postMessage(data,...args);}terminate(){if(this.decoderRecord){window.decoderTrace.push({...this.decoderRecord,terminated:true,elapsed:performance.now()-this.decoderRecord.sentAt,loadElapsed:performance.now()-(this.decoderRecord.reserveAt||this.decoderRecord.sentAt)});if(window.decoderTrace.length>40)window.decoderTrace.shift();}return super.terminate();}};window.inputTrace=[];window.frameTrace=[];const Base=Worker;window.Worker=class extends Base{constructor(...args){super(...args);this.addEventListener('message',({data})=>{if(data.type==='frame'){if(data.audio?.length)window.sawGuestPCM=true;window.frameCount=(window.frameCount||0)+1;window.lastFrame={saveWarning:data.commands.some(c=>c[0]==='image'&&c[2]===330),serial:data.serial,commands:data.commands.length,controls:data.recordedControls,window:data.window,balls:data.commands.filter(c=>c[0]==='linear-image'&&c[3]===6&&c[4]===6).map(c=>c.slice(0,5))};window.frameTrace.push(window.lastFrame);if(window.frameTrace.length>8)window.frameTrace.shift();}});}postMessage(data,...args){if(data.type==='tick'&&data.events?.length){window.inputTrace.push(data.events);if(window.inputTrace.length>20)window.inputTrace.shift();}return super.postMessage(data,...args);}};'''})
  try:
   browser.set_window_size(1100,1000);browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play')
   hooked=browser.execute_async_script('''const done=arguments[arguments.length-1];Promise.all([import('./recorded-session.js'),import('./progress-store.js'),import('./recorded-decoder-client.js')]).then(([{RecordedSession},{ProgressStore},{RecordedDecoderClient}])=>{const note=entry=>{window.decoderTrace.push(entry);if(window.decoderTrace.length>40)window.decoderTrace.shift();},openDecoder=RecordedDecoderClient.open,readDecoder=RecordedDecoderClient.prototype.read;RecordedDecoderClient.open=async function(...args){try{return await openDecoder.apply(this,args);}catch(error){note({operation:'open',encodedBytes:args[1]?.length,error:String(error)});throw error;}};RecordedDecoderClient.prototype.read=async function(...args){try{return await readDecoder.apply(this,args);}catch(error){note({operation:'read',error:String(error)});throw error;}};const load=ProgressStore.prototype.load,commit=ProgressStore.prototype.commit;ProgressStore.prototype.load=async function(...args){window.progressSaveKey=this.saveKey;let result=await load.apply(this,args);if(window.seedSave){result={...result,save:new Uint8Array(window.seedSave)};window.seedSave=null;}window.lastLoaded=Array.from(result.save);return result;};ProgressStore.prototype.commit=async function(save,...args){const result=await commit.call(this,save,...args);if(save)window.lastSaved=Array.from(save);return result;};const open=RecordedSession.open,snapshot=RecordedSession.prototype.snapshot;window.roots=[];window.states=[];RecordedSession.open=async(...args)=>{window.roots.push(args[4]);const session=await open(...args);window.guestBytes=session.guestBytes;return session;};RecordedSession.prototype.snapshot=async function(){const value=await snapshot.call(this);window.lastAudio=value;window.states.push(value);if(value.activeEffects)window.sawEffect=true;return value;};done(true);}).catch(error=>done(String(error)));''');assert hooked is True,hooked
   browser.find_element('id','play').click();wait.until(lambda b:b.execute_script('return window.lastAudio?.musicStatus===arguments[0]', 'stopped' if args.playlist_case else 'playing'))
   assert browser.execute_script('return window.guestBytes')==16777216-3145728-7340032-2*scratch
   assert browser.execute_script('return window.roots.at(-1).used')==16777216
   canvas=browser.find_element('id','game');canvas.send_keys(Keys.ENTER)
   def click(x,y):
    box=canvas.rect;ActionChains(browser).move_to_element_with_offset(canvas,(x-320)*box['width']/640,(y-240)*box['height']/480).click().perform()
   if args.playlist_case:
    for attempt in range(2):
     if attempt:
      previous_session=browser.execute_script('return window.lastAudio.session')
      browser.find_element('id','restart').click();wait.until(lambda b:b.execute_script('return window.lastAudio?.session!==arguments[0]&&window.lastAudio?.musicStatus==="stopped"',previous_session))
      assert browser.execute_script('return window.roots.slice(0,-1).every(root=>root.used===0)')
      canvas.send_keys(Keys.ENTER)
     browser.execute_script('window.sawEffect=false;')
     click(320,185);wait.until(lambda b:b.execute_script('return window.lastFrame?.window?.confined===true'))
     ActionChains(browser).send_keys(Keys.SPACE).perform();wait.until(lambda b:b.execute_script('return window.sawEffect===true'))
     assert browser.execute_script('return window.lastAudio.musicStatus==="stopped"&&!window.sawGuestPCM')
     ActionChains(browser).send_keys(Keys.ESCAPE).perform();wait.until(lambda b:b.execute_script('return window.lastFrame?.window?.confined===false'))
    browser.find_element('id','stop').click();wait.until(lambda b:b.execute_script('return window.roots.every(root=>root.used===0)'))
    print(profile,args.playlist_case,'retains original host effects, no guest PCM, stopped music and fresh restart/released budgets.',flush=True)
    continue
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
   for shape in ['single-name','collection']:
    previous_session=browser.execute_script('return window.lastAudio.session')
    browser.execute_script("""const shape=arguments[0],bytes=new Uint8Array(1048576),view=new DataView(bytes.buffer);bytes.set(new TextEncoder().encode('ARK4'));for(const [offset,value] of [[4,42],[8,9],[12,(64<<1)|(64<<10)|(1<<18)]])view.setUint32(offset,value,true);if(shape==='single-name'){view.setUint32(16,1,true);view.setUint32(20,1048576-224,true);bytes.fill(120,24,1048576-200);if(arguments[4]){bytes[24]=arguments[7]?83:65;const mark=arguments[7]?[204,149]:arguments[6]?[204,167,204,149]:arguments[5]?[204,167,204,168]:[204,167];for(let offset=25;offset+2<1048576-200;offset+=2)bytes.set(mark.slice((offset-25)%mark.length,(offset-25)%mark.length+2),offset);if(arguments[7])bytes.set([204,167],1048576-203);}else if(arguments[2]){bytes[24]=65;const selector=new TextEncoder().encode(arguments[3]?'\uFE0E\uFE0F':'\uFE0F');for(let offset=25;offset+3<1048576-200;offset+=3)bytes.set(selector.subarray(((offset-25)/3)% (selector.length/3)*3,(((offset-25)/3)% (selector.length/3)+1)*3),offset);}else if(arguments[1]){const sample=new TextEncoder().encode(arguments[1]);for(let offset=24;offset+sample.length<1048576-200;offset+=sample.length)bytes.set(sample,offset);}bytes.fill(6,1048576-200);}else{const count=Math.floor((1048576-20)/208),extra=1048576-20-count*208;view.setUint32(16,count,true);let offset=20;for(let index=0;index<count;index++){const name=String(index).padStart(4,'0')+(index+1===count?'x'.repeat(extra):'');view.setUint32(offset,name.length,true);offset+=4;bytes.set(new TextEncoder().encode(name),offset);offset+=name.length;bytes.fill(6,offset,offset+200);offset+=200;}if(offset!==bytes.length)throw Error('fixture length');}window.seedSave=bytes;window.expectedSave=bytes;window.lastSaved=null;window.lastLoaded=null;""",shape,args.unicode_name,args.selector_run or args.mixed_selector_run,args.mixed_selector_run,args.missing_mark_run or args.mixed_missing_mark_run or args.mixed_class_missing_mark_run or args.late_composing_mark_run,args.mixed_missing_mark_run,args.mixed_class_missing_mark_run,args.late_composing_mark_run)
    if args.late_composing_mark_run and shape=='single-name':
     assert browser.execute_script('return window.expectedSave[24]===83&&window.expectedSave[1048576-203]===204&&window.expectedSave[1048576-202]===167&&window.expectedSave[1048576-201]===120&&window.expectedSave.subarray(25,1048576-203).every((value,index)=>value===[204,149][index%2])'),'single full-size late-composing fixture'
    if args.mixed_class_missing_mark_run and shape=='single-name':
     assert browser.execute_script('return window.expectedSave[24]===65&&window.expectedSave[1048576-201]===120&&window.expectedSave.subarray(25,1048576-201).every((value,index)=>value===[204,167,204,149][index%4])'),'single full-size mixed-class missing-mark cluster'
    if args.mixed_missing_mark_run and shape=='single-name':
     assert browser.execute_script('return window.expectedSave[24]===65&&window.expectedSave[1048576-201]===120&&window.expectedSave.subarray(25,1048576-201).every((value,index)=>value===[204,167,204,168][index%4])'),'single full-size mixed missing-mark cluster'
    if args.missing_mark_run and shape=='single-name':
     assert browser.execute_script('return window.expectedSave[24]===65&&window.expectedSave[1048576-201]===120&&window.expectedSave.subarray(25,1048576-201).every((value,index)=>value===[204,167][index%2])'),'single full-size missing-mark cluster'
    if args.mixed_selector_run and shape=='single-name':
     assert browser.execute_script('return window.expectedSave[24]===65&&window.expectedSave[1048576-201]===120&&window.expectedSave.subarray(25,1048576-201).every((value,index)=>value===[239,184,142,239,184,143][index%6])'),'single full-size mixed-selector fixture'
    if args.selector_run and shape=='single-name':
     assert browser.execute_script('return window.expectedSave[24]===65&&window.expectedSave[1048576-201]===120&&window.expectedSave.subarray(25,1048576-201).every((value,index)=>value===[239,184,143][index%3])'),'single full-size selector-run fixture'
    browser.find_element('id','play').click()
    wait.until(lambda b:b.execute_script('return window.lastLoaded?.length===1048576&&window.lastAudio?.session!==arguments[0]&&window.lastAudio?.musicStatus==="paused"',previous_session))
    assert browser.execute_script('return window.guestBytes')==16777216-3145728-7340032-2*scratch
    canvas.send_keys(Keys.ENTER);click(320,285);click(320,180)
    wait.until(lambda b:b.execute_script('return window.lastSaved?.length===1048576'))
    assert browser.execute_script('return window.lastSaved.every((value,index)=>index>=12&&index<16||value===window.expectedSave[index])'),shape
    assert browser.execute_script('return (new DataView(new Uint8Array(window.lastSaved).buffer).getUint32(12,true)&(1<<18))===0'),shape
    browser.execute_script('window.expectedEdited=new Uint8Array(window.lastSaved);if(arguments[0]==="collection")window.expectedEdited[28]=255;window.lastSaved=null;',shape)
    click(320,450);canvas.send_keys('e');canvas.send_keys('l');canvas.send_keys(Keys.ENTER)
    if shape=='single-name':
     canvas.send_keys('n');canvas.send_keys(Keys.BACKSPACE);canvas.send_keys('y');canvas.send_keys(Keys.BACKSPACE);canvas.send_keys('x');canvas.send_keys(Keys.ESCAPE)
    if shape=='collection':
     box=canvas.rect;ActionChains(browser).move_to_element_with_offset(canvas,(35-320)*box['width']/640,(105-240)*box['height']/480).context_click().perform()
    canvas.send_keys('s');wait.until(lambda b:b.execute_script('return window.lastSaved?.length===1048576'))
    assert browser.execute_script('return window.lastSaved.every((value,index)=>value===window.expectedEdited[index])'),'maximum browser editor '+shape
    print(profile,shape,'maximum archive opens in the editor and commits exactly the expected bytes.',flush=True)
    browser.find_element('id','stop').click();wait.until(lambda b:b.execute_script('return window.roots.every(root=>root.used===0)'))
    previous_session=browser.execute_script('window.expectedReload=new Uint8Array(window.lastSaved);window.lastLoaded=null;return window.lastAudio.session;')
    browser.find_element('id','play').click()
    wait.until(lambda b:b.execute_script('return window.lastLoaded?.length===1048576&&window.lastAudio?.session!==arguments[0]&&window.lastAudio?.musicStatus==="playing"',previous_session))
    assert browser.execute_script('return window.lastLoaded.every((value,index)=>value===window.expectedReload[index])'),'maximum archive persistent reload'
    browser.find_element('id','stop').click();wait.until(lambda b:b.execute_script('return window.roots.every(root=>root.used===0)'))
    print(profile,shape,'maximum browser archive loads, commits and reloads intact under the combined audio shares.',flush=True)
   if args.corrupt_saves:
    for magic,flags in [('ARK1',2),('ARK1',4294967295),('ARK2',2),('ARK3',2048),('ARK4',129<<10),('ARK4',1<<19)]*args.corrupt_save_repeats:
     damaged=list(magic.encode())+list((17).to_bytes(4,'little'))+list((7).to_bytes(4,'little'))+list(flags.to_bytes(4,'little'))
     if magic!='ARK1':damaged += [0]*4
     result=browser.execute_async_script("""const bytes=new Uint8Array(arguments[0]),done=arguments[arguments.length-1],request=indexedDB.open('gamepkg-progress-v1',1);request.onerror=()=>done(String(request.error));request.onsuccess=()=>{const db=request.result,tx=db.transaction('saves','readwrite');tx.objectStore('saves').put(bytes,window.progressSaveKey);tx.oncomplete=()=>{db.close();window.lastSaved=null;window.lastLoaded=null;done(true);};tx.onabort=()=>{db.close();done(String(tx.error));};};""",damaged)
     assert result is True,result
     previous_session=browser.execute_script('return window.lastAudio.session')
     browser.find_element('id','play').click();wait.until(lambda b:b.execute_script('return window.lastLoaded?.length===arguments[0]&&window.lastAudio?.session!==arguments[1]&&window.lastAudio?.musicStatus==="playing"',len(damaged),previous_session))
     assert browser.execute_script('return window.lastLoaded.every((value,index)=>value===arguments[0][index])',damaged)
     canvas.send_keys(Keys.ENTER);click(320,285);wait.until(lambda b:b.execute_script('return window.lastFrame.saveWarning===true'))
     click(320,180);wait.until(lambda b:b.execute_script('return window.lastAudio.musicStatus==="paused"'))
     box=canvas.rect;ActionChains(browser).move_to_element_with_offset(canvas,0,5*box['height']/480).click_and_hold().move_by_offset(-25*box['width']/640,0).release().perform()
     click(320,180);wait.until(lambda b:b.execute_script('return window.lastAudio.musicStatus==="playing"'))
     assert browser.execute_script('return window.lastSaved===null'),'Guest attempted to overwrite corrupt save'
     browser.find_element('id','stop').click();wait.until(lambda b:b.execute_script('return window.roots.every(root=>root.used===0)'))
     stored=browser.execute_async_script("""const done=arguments[arguments.length-1],request=indexedDB.open('gamepkg-progress-v1',1);request.onerror=()=>done(String(request.error));request.onsuccess=()=>{const db=request.result,tx=db.transaction('saves','readonly'),get=tx.objectStore('saves').get(window.progressSaveKey);let bytes;get.onsuccess=()=>{bytes=Array.from(get.result);};tx.oncomplete=()=>{db.close();done(bytes);};tx.onabort=()=>{db.close();done(String(tx.error));};};""")
     assert stored==damaged,(magic,flags,stored)
     print(profile,magic,hex(flags),'actual IndexedDB bytes preserved; visible warning and music mute/resume remain functional.',flush=True)
   failures=browser.execute_script('return window.states.filter(state=>state.musicStatus==="failed")')
   assert not failures,('Valid original music entered Failed',failures,browser.execute_script('return window.decoderTrace'))
   print(profile,'real original music, mute clock freeze, compressed collision effect, saved restart/resume and parent-memory release passed.',flush=True)
  except Exception:
   print('Full recorded web failure:',browser.find_element('id','status').text,browser.execute_script('return {decoder:window.decoderTrace,input:window.inputTrace,frames:window.frameTrace,audio:window.lastAudio,state:document.querySelector("canvas").dataset.state}'),browser.get_log('browser'),flush=True);raise
  finally:browser.quit();server.shutdown();server.server_close()
