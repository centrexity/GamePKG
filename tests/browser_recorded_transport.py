"""Actual bounded live audio FIFO and output clock against independent SDL hashes."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import json,os,shutil,sys,tempfile,threading,subprocess,wave,struct
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
project=Path(__file__).resolve().parents[1]
companion=Path(sys.argv[1])
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-worklet-mixer-') as temporary:
 root=Path(temporary)
 for name in ['recorded-decoder.js','recorded-mixer.js','recorded-queue.js','recorded-stream.js','recorded-transport.js','recorded-decoder-client.js','recorded-decoder-worker.js','recorded-budget.js','recorded-worklet.js','recorded-output-client.js','recorded-wav.js','recorded-frame-dispatcher.js','recorded-music-loader.js','recorded-session.js']:shutil.copy2(project/'web-player'/name,root/name)
 shutil.copy2(project/'tests/fixtures/recorded-transport-worklet.js',root/'stream-worklet.js');shutil.copy2(companion,root/'converter.wasm')
 shutil.copy2(project/'tests/fixtures/sdl-live-audio-reference.json',root/'reference.json')
 shutil.copy2(Path(sys.argv[2]),root/'decoder.wasm')
 with wave.open(str(root/'tone.wav'),'wb') as audio:
  audio.setnchannels(2);audio.setsampwidth(2);audio.setframerate(44100)
  audio.writeframes(b''.join(struct.pack('<hh',(i*7919)%65536-32768,(i*7919+31337)%65536-32768) for i in range(10000)))
 subprocess.run(['ffmpeg','-v','error','-i',str(root/'tone.wav'),'-codec:a','libmp3lame','-write_xing','0',str(root/'tone.mp3')],check=True)

 (root/'index.html').write_text('<script type="module">window.mixerReady=true;</script>')
 server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
 options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert options.binary_location
 for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--autoplay-policy=no-user-gesture-required']:options.add_argument(flag)
 driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
 browser=webdriver.Chrome(service=Service(driver),options=options)
 try:
  browser.get(f'http://127.0.0.1:{server.server_port}/');WebDriverWait(browser,20).until(lambda b:b.execute_script('return !!window.mixerReady'))
  browser.set_script_timeout(30)
  result=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {RecordedMusicSender}=await import('./recorded-transport.js');
   const reference=await(await fetch('reference.json')).json(),wasm=new Uint8Array(await(await fetch('converter.wasm')).arrayBuffer()),results=[];
   for(const record of reference.cases.slice(0,6)){
    const context=new AudioContext({sampleRate:48000});await context.audioWorklet.addModule('stream-worklet.js');
    const node=new AudioWorkletNode(context,'transport-audio',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{wasm,volume:record.effect_volume}});
    const channel=new MessageChannel(),sender=new RecordedMusicSender(channel.port1);node.port.postMessage({port:channel.port2},[channel.port2]);
    let at=0,reads=0,closed=false;const decoder={async read(size){reads++;const frames=Math.min(size/2,6000-at),values=new Int16Array(frames*2);for(let i=0;i<frames;i++)for(let c=0;c<2;c++)values[i*2+c]=((at+i)*7919+c*31337)%65536-32768;at+=frames;return values;},close(){closed=true;}};
    const finished=new Promise((resolve,reject)=>{node.port.onmessage=({data})=>data.error?reject(Error(data.error)):resolve(data);node.onprocessorerror=()=>reject(Error('worklet failed'));});
    sender.start(1,decoder);const gain=context.createGain();gain.gain.value=0;node.connect(gain);gain.connect(context.destination);await context.resume();
    const report=await finished;if(report.played!==6000||report.status!=='ended'||!closed||reads!==9)throw Error('Transport completion mismatch '+JSON.stringify({played:report.played,status:report.status,closed,reads}));
    const bytes=new Uint8Array(report.values.length*2),view=new DataView(bytes.buffer);for(let i=0;i<report.values.length;i++)view.setInt16(i*2,report.values[i],true);
    const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),v=>v.toString(16).padStart(2,'0')).join('');if(hash!==record.sha256)throw Error('Live MessagePort SDL mismatch '+record.effect_volume+': '+hash);
    sender.close();node.port.close();node.disconnect();gain.disconnect();await context.close();results.push({volume:record.effect_volume,hash,reads});
   }return results;
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in result,result
  assert len(result['value'])==6,result
  print('Actual Chromium live MessagePort to AudioWorklet: all six independent SDL hashes, complete queue consumption and decoder closure passed.',json.dumps(result['value']),flush=True)
  decoded=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {RecordedDecoder}=await import('./recorded-decoder.js'),{RecordedDecoderClient}=await import('./recorded-decoder-client.js'),{RecordedStream}=await import('./recorded-stream.js'),{RecordedMusicSender}=await import('./recorded-transport.js');
   const load=async name=>new Uint8Array(await(await fetch(name)).arrayBuffer());
   const wasm=await load('converter.wasm'),codec=await load('decoder.wasm'),encoded=await load('tone.mp3');
   const {RecordedBudget}=await import('./recorded-budget.js'),charge=6291456+codec.length*2+65536+1600*2*3;
   const budget=new RecordedBudget(charge),reserved=await RecordedDecoderClient.open(codec,encoded,6291456,undefined,budget);
   if(budget.used!==charge)throw Error('Decoder reservation missing');let denied=false;
   try{await RecordedDecoderClient.open(codec,encoded,6291456,undefined,budget);}catch{denied=true;}
   if(!denied||budget.used!==charge)throw Error('Concurrent decoder overcommitted');reserved.close();if(budget.used)throw Error('Decoder close leaked reservation');
   denied=false;try{await RecordedDecoderClient.open(codec,new Uint8Array([1]),6291456,undefined,budget);}catch{denied=true;}
   if(!denied||budget.used)throw Error('Failed decoder open leaked reservation');
   const parent=new RecordedBudget(charge),child=parent.partition(charge),retained=await RecordedDecoderClient.open(codec,encoded,6291456,undefined,child);child.close();
   if(parent.used!==charge)throw Error('Child released capacity with a live decoder');retained.close();if(parent.used)throw Error('Retired child kept capacity');
   const cancellation=new AbortController(),opening=RecordedDecoderClient.open(codec,encoded,6291456,cancellation.signal,budget);cancellation.abort();denied=false;try{await opening;}catch{denied=true;}
   if(!denied||budget.used)throw Error('Cancelled decoder startup leaked reservation');
   const direct=new RecordedDecoder(codec);direct.open(encoded);const pcm=[];for(;;){const block=new Int16Array(1600),n=direct.read(block);if(!n)break;pcm.push(...block.subarray(0,n));}direct.close();
   if(pcm.length<20000||pcm.length>30000)throw Error('Unexpected synthetic MP3 length');
   const frames=16384,baseline=new RecordedStream(wasm);baseline.begin(1);baseline.effectVolume(64);
   for(let slot=0;slot<4;slot++){const count=[1025,2051,4096,777][slot],bytes=new Uint8Array(count*4),view=new DataView(bytes.buffer);for(let i=0;i<count;i++)for(let c=0;c<2;c++)view.setInt16(i*4+c*2,(i*(slot*1543+197)+c*12345+slot*313)%65536-32768,true);baseline.load(slot,bytes);baseline.play(slot);}
   let at=0,ended=false;const expected=new Int16Array(frames*2);
   for(let frame=0;frame<frames;frame+=128){while(at<pcm.length&&baseline.availableFrames>=800){const n=Math.min(1600,pcm.length-at);if(!baseline.push(1,Int16Array.from(pcm.slice(at,at+n))))throw Error('baseline enqueue');at+=n;}if(at===pcm.length&&!ended){baseline.finishMusic(1);ended=true;}baseline.render(expected.subarray(frame*2,(frame+128)*2));}
   if(baseline.musicStatus!=='ended'||baseline.playedFrames!==pcm.length/2)throw Error('baseline incomplete');baseline.close();
   const results=[];for(const mode of ['normal','replacement','stop-restart']){
   const guestBytes=65536,effectsBytes=4194304,musicBytes=charge*2+6400;
   const root=new RecordedBudget(guestBytes+effectsBytes+musicBytes),guest=root.reserve(guestBytes),effects=root.reserve(effectsBytes),liveBudget=root.partition(musicBytes);
   let overcommitted=false;try{root.reserve(1);}catch{overcommitted=true;}if(!overcommitted)throw Error('Independent audio allowance escaped root');
   const client=await RecordedDecoderClient.open(codec,encoded,6291456,undefined,liveBudget),context=new AudioContext({sampleRate:48000});await context.audioWorklet.addModule('stream-worklet.js');
   const node=new AudioWorkletNode(context,'transport-audio',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{wasm,volume:64,frames}});
   const channel=new MessageChannel(),sender=new RecordedMusicSender(channel.port1,liveBudget);node.port.postMessage({port:channel.port2},[channel.port2]);
   const finished=new Promise((resolve,reject)=>{node.port.onmessage=({data})=>data.error?reject(Error(data.error)):resolve(data);node.onprocessorerror=()=>reject(Error('decoder worklet failed'));});
   const gain=context.createGain();gain.gain.value=0;node.connect(gain);gain.connect(context.destination);await context.resume();let retired=null;
   if(mode==='normal')sender.start(1,client);else{
    retired=await RecordedDecoderClient.open(codec,encoded,6291456,undefined,liveBudget);let release;
    const held={async read(size){const values=await retired.read(size);return new Promise(resolve=>release=()=>resolve(values.fill(32767)));},close(){retired.close();}};
    sender.start(1,held);const deadline=performance.now()+2000;while(!release){if(performance.now()>deadline)throw Error('Retired read not admitted');await new Promise(resolve=>setTimeout(resolve,1));}
    if(mode==='stop-restart'){if(!sender.stop(2))throw Error('Stop rejected');sender.start(3,client);}else sender.start(2,client);
    if(!retired.closed)throw Error('Replaced decoder alive');release();
   }
   const report=await finished;
   if(report.status!=='ended'||report.played!==pcm.length/2||!client.closed||liveBudget.used||report.values.length!==expected.length)throw Error('worker pipeline incomplete');
   for(let i=0;i<expected.length;i++)if(report.values[i]!==expected[i])throw Error('worker pipeline differs at scalar '+i+': '+report.values[i]+' != '+expected[i]);
   sender.close();liveBudget.close();if(root.used!==guestBytes+effectsBytes)throw Error('Audio worker release changed guest/effect ownership');node.disconnect();node.port.close();gain.disconnect();await context.close();effects.release();guest.release();if(root.used)throw Error('Root reservation leaked');root.close();results.push({mode,sourceFrames:report.played,comparedScalars:expected.length,workerClosed:client.closed,retiredClosed:retired?.closed??true});
   }return results;
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in decoded,decoded
  print('Actual supervised MP3 decoder worker -> credited MessagePort -> live AudioWorklet matches every direct mixed scalar:',json.dumps(decoded['value']),flush=True)
  endpoint=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {RecordedDecoderClient}=await import('./recorded-decoder-client.js'),{RecordedMusicSender}=await import('./recorded-transport.js');
   const load=async name=>new Uint8Array(await(await fetch(name)).arrayBuffer());
   const context=new AudioContext({sampleRate:48000});await context.audioWorklet.addModule('recorded-worklet.js');
   const node=new AudioWorkletNode(context,'gamepkg-recorded-output',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{session:42,wasm:await load('converter.wasm')}});
   let serial=0;const replies=new Map();node.port.onmessage=({data})=>{const pending=replies.get(data.id);if(pending){replies.delete(data.id);pending(data);}};
   const request=(kind,fields={},transfer=[])=>new Promise(resolve=>{const id=++serial;replies.set(id,resolve);node.port.postMessage({session:42,id,kind,...fields},transfer);});
   const first=await request('snapshot');if(!first.ok||first.musicStatus!=='stopped')throw Error('Initial endpoint state');
   const clip=new Uint8Array(4096*4),view=new DataView(clip.buffer);for(let i=0;i<4096;i++){view.setInt16(i*4,12345,true);view.setInt16(i*4+2,-23456,true);}
   if(!(await request('load',{clip:0,pcm:clip},[clip.buffer])).ok)throw Error('Endpoint clip load');
   const rejected=await request('controls',{commands:[[0,0],[5,129]]});if(rejected.ok||rejected.activeEffects)throw Error('Rejected batch partially published');
   const commands=[[1,128],[0,0]],prepared=await request('prepare',{epoch:0,frame:1,commands});if(!prepared.ok||prepared.activeEffects)throw Error('Prepare published audio');commands[1]=[3,0];
   if((await request('controls',{commands:[[0,0]]})).ok||(await request('unload',{clip:0})).ok)throw Error('Prepared frame did not retain exclusive controls/cache');
   if((await request('finish',{epoch:0,frame:2,success:true})).ok||(await request('prepare',{epoch:0,frame:2,commands:[]})).ok)throw Error('Mismatched or overlapping audio frame accepted');
   const discarded=await request('finish',{epoch:0,frame:1,success:false});if(!discarded.ok||discarded.activeEffects)throw Error('Failed checkpoint published audio');
   if((await request('finish',{epoch:0,frame:1,success:true})).ok||(await request('prepare',{epoch:0,frame:1,commands:[[0,0]]})).ok)throw Error('Failed audio frame replayed');
   const next=await request('prepare',{epoch:0,frame:2,commands:[[1,128],[0,0]]});if(!next.ok||next.activeEffects)throw Error('Fresh audio frame failed validation');
   const played=await request('finish',{epoch:0,frame:2,success:true});if(!played.ok||played.activeEffects!==1)throw Error('Successful checkpoint did not publish effect');
   node.port.postMessage({session:41,id:serial+100,kind:'controls',commands:[[3,0]]});
   node.port.postMessage({session:42,id:serial,kind:'controls',commands:[[3,0]]});
   const unchanged=await request('snapshot');if(!unchanged.ok||unchanged.activeEffects!==1)throw Error('Stale or foreign request changed playback');
   const busy=await request('unload',{clip:0});if(busy.ok||busy.activeEffects!==1)throw Error('Active clip unload changed playback');
   const analyser=context.createAnalyser();analyser.fftSize=256;const gain=context.createGain();gain.gain.value=0;node.connect(analyser);analyser.connect(gain);gain.connect(context.destination);await context.resume();
   let heard=false;const signal=new Float32Array(256),deadline=performance.now()+2000;while(performance.now()<deadline){analyser.getFloatTimeDomainData(signal);if(signal.some(v=>v!==0)){heard=true;break;}await new Promise(r=>setTimeout(r,1));}if(!heard)throw Error('No real output from production endpoint');
   const channel=new MessageChannel(),sender=new RecordedMusicSender(channel.port1);if(!(await request('music-port',{port:channel.port2},[channel.port2])).ok)throw Error('Music port denied');
   const committed=await request('controls',{commands:[[4,1],[6,1]]});if(!committed.ok||committed.musicStatus!=='paused')throw Error('Committed paused Play rejected');
   const decoder=await RecordedDecoderClient.open(await load('decoder.wasm'),await load('tone.mp3'));sender.start(1,decoder);
   let paused;const prefill=performance.now()+2000;do{paused=await request('snapshot');if(paused.queuedFrames===4800)break;if(performance.now()>prefill)throw Error('Committed generation transport stalled');await new Promise(r=>setTimeout(r,5));}while(true);
   if(paused.playedFrames!==0||paused.musicStatus!=='paused')throw Error('Paused committed generation advanced');if(!(await request('controls',{commands:[[6,0]]})).ok)throw Error('Committed music resume rejected');
   let state;const end=performance.now()+3000;do{state=await request('snapshot');if(state.musicStatus==='ended')break;if(performance.now()>end)throw Error('Production endpoint music stalled');await new Promise(r=>setTimeout(r,5));}while(true);
   if(state.playedFrames!==11520||state.activeEffects!==0||!decoder.closed)throw Error('Production output clock incomplete');
   const closed=await request('close');if(!closed.closed||!closed.ok)throw Error('Endpoint close not acknowledged');sender.close();node.port.close();node.disconnect();analyser.disconnect();gain.disconnect();await context.close();return {heard,sourceFrames:state.playedFrames,closed:closed.closed};
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in endpoint,endpoint
  print('Production recorded AudioWorklet: real output, atomic rejected batch, effect completion, supervised MP3 music clock and acknowledged shutdown passed.',json.dumps(endpoint['value']),flush=True)
  supervised=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {RecordedOutputClient}=await import('./recorded-output-client.js'),{RecordedBudget}=await import('./recorded-budget.js'),{RecordedDecoderClient}=await import('./recorded-decoder-client.js'),{RecordedMusicSender}=await import('./recorded-transport.js');
   const load=async name=>new Uint8Array(await(await fetch(name)).arrayBuffer()),wasm=await load('converter.wasm'),budget=new RecordedBudget(4194304+wasm.length*3);
   const output=await RecordedOutputClient.open(wasm,4194304,budget);if(budget.used!==budget.limit)throw Error('Output reservation missing');output.volume(0);await output.resume();
   const inFlight=output.request('snapshot');let denied=false;try{await output.request('snapshot');}catch{denied=true;}if(!denied||!(await inFlight).ok)throw Error('Output backpressure lost active request');
   const channel=new MessageChannel(),sender=new RecordedMusicSender(channel.port1);if(!await output.connectMusic(sender,channel.port2))throw Error('Supervised music connection failed');
   const committed=await output.request('controls',{commands:[[4,1],[6,1]]});if(!committed.ok||committed.musicStatus!=='paused')throw Error('Committed paused Play rejected');
   const decoder=await RecordedDecoderClient.open(await load('decoder.wasm'),await load('tone.mp3'));sender.start(1,decoder);
   let paused;const prefill=performance.now()+2000;do{paused=await output.request('snapshot');if(paused.queuedFrames===4800)break;if(performance.now()>prefill)throw Error('Committed generation transport stalled');await new Promise(r=>setTimeout(r,5));}while(true);
   if(paused.playedFrames!==0||paused.musicStatus!=='paused')throw Error('Paused committed generation advanced');if(!(await output.request('controls',{commands:[[6,0]]})).ok)throw Error('Committed music resume rejected');
   const closing=output.close();if(budget.used!==budget.limit||!decoder.closed)throw Error('Output reservation released before context shutdown or decoder left alive');await closing;if(budget.used||!output.closed)throw Error('Output shutdown leaked reservation');
   const abort=new AbortController(),opening=RecordedOutputClient.open(wasm,4194304,budget,abort.signal);abort.abort();denied=false;try{await opening;}catch{denied=true;}if(!denied||budget.used)throw Error('Cancelled output startup leaked reservation');
   const fresh=await RecordedOutputClient.open(wasm,4194304,budget);if(!(await fresh.request('snapshot')).ok)throw Error('Output did not recover');await fresh.close();if(budget.used)throw Error('Recovered output leaked reservation');const post=MessagePort.prototype.postMessage;MessagePort.prototype.postMessage=function(message,...rest){if(message?.kind==='snapshot')return;return post.call(this,message,...rest);};let timedOut=false;
   try{await RecordedOutputClient.open(wasm,4194304,budget);}catch(error){timedOut=String(error).includes('deadline');}finally{MessagePort.prototype.postMessage=post;}
   if(!timedOut||budget.used)throw Error('Unresponsive output escaped deadline or leaked memory');
   const encoded=await load('tone.wav'),assetBudget=new RecordedBudget(budget.limit),effectOutput=await RecordedOutputClient.open(wasm,4194304,assetBudget);
   const loaded=await effectOutput.loadWav(0,encoded);if(!loaded.ok||encoded.length!==40044||assetBudget.used!==budget.limit)throw Error('WAV load ownership or staging charge');
   const invalid=encoded.slice();invalid[24]^=1;denied=false;try{await effectOutput.loadWav(1,invalid);}catch{denied=true;}if(!denied||assetBudget.used!==budget.limit)throw Error('Malformed WAV escaped admission');
   const playing=await effectOutput.request('controls',{commands:[[0,0]]});if(!playing.ok||playing.activeEffects!==1)throw Error('Admitted WAV did not play');
   const pendingLoad=effectOutput.loadWav(1,encoded);const pendingRejected=pendingLoad.then(()=>false,()=>true);await effectOutput.close();if(!await pendingRejected||assetBudget.used)throw Error('Cancelled WAV admission leaked staging');
   return {backpressure:true,decoderClosed:decoder.closed,reservedUntilContextClosed:true,cancelled:true,recovered:true,timedOut,wavAdmission:true};
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in supervised,supervised
  print('Production output supervisor: one request, shared reservation, immediate startup cancellation, connected decoder teardown and fresh recovery passed.',json.dumps(supervised['value']),flush=True)
  dispatched=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {RecordedOutputClient}=await import('./recorded-output-client.js'),{RecordedBudget}=await import('./recorded-budget.js'),{RecordedFrameDispatcher}=await import('./recorded-frame-dispatcher.js');
   const load=async name=>new Uint8Array(await(await fetch(name)).arrayBuffer()),wasm=await load('converter.wasm'),wav=await load('tone.wav'),invalid=wav.slice();invalid[24]^=1;
   const budget=new RecordedBudget(4194304+wasm.length*3+wav.length-44),output=await RecordedOutputClient.open(wasm,4194304,budget),dispatcher=new RecordedFrameDispatcher(output,new Map([['tone.wav',wav],['bad.wav',invalid],['next.wav',wav]]),{effects:true});
   const frame=(serial,recordedControls)=>({epoch:0,serial,recordedControls,recordedActiveGeneration:null});
   if(!await dispatcher.prepare(frame(1,[[0,'tone.wav',0]])))throw Error('Effect prepare failed');if((await output.request('snapshot')).activeEffects)throw Error('Asset admission started playback');
   if(await dispatcher.finish(0,2,true))throw Error('Wrong dispatcher frame accepted');if(!await dispatcher.finish(0,1,false))throw Error('Discard failed');if((await output.request('unload',{clip:0})).ok)throw Error('Failed frame kept uncommitted cache');
   if(!await dispatcher.prepare(frame(2,[[2,'',1],[0,'tone.wav',0]]))||!await dispatcher.finish(0,2,true))throw Error('Fresh committed effect failed');if((await output.request('snapshot')).activeEffects!==1)throw Error('Fresh effect not published');
   let denied=false;try{await dispatcher.prepare(frame(3,[[3,'',0],[0,'next.wav',0],[0,'bad.wav',0]]));}catch{denied=true;}if(!denied||(await output.request('snapshot')).activeEffects!==1)throw Error('Malformed asset partially stopped playback');if((await output.request('unload',{clip:1})).ok)throw Error('Malformed frame retained an earlier staged admission');
   if(!await dispatcher.prepare(frame(4,[[3,'',0]]))||!await dispatcher.finish(0,4,true)||(await output.request('snapshot')).activeEffects)throw Error('Fresh frame did not recover');
   let release;const admit=output.loadWav.bind(output);output.loadWav=async(...args)=>{await new Promise(resolve=>release=resolve);return admit(...args);};const preparing=dispatcher.prepare(frame(5,[[0,'next.wav',0]])).then(value=>value,()=>false);while(!release)await new Promise(r=>setTimeout(r,1));await dispatcher.close();release();if(await preparing||budget.used)throw Error('Closed dispatcher revived pending admission');return {discardedCache:true,publishedFresh:true,malformedRollback:true,closedAdmission:true};
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in dispatched,dispatched
  print('Production effect frame dispatcher: staged cache rollback, checkpoint-only publication, malformed asset rollback and close during admission passed.',json.dumps(dispatched['value']),flush=True)
  musicFrames=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {RecordedOutputClient}=await import('./recorded-output-client.js'),{RecordedBudget}=await import('./recorded-budget.js'),{RecordedFrameDispatcher}=await import('./recorded-frame-dispatcher.js'),{RecordedMusicLoader}=await import('./recorded-music-loader.js'),{RecordedMusicSender}=await import('./recorded-transport.js');
   const load=async name=>new Uint8Array(await(await fetch(name)).arrayBuffer()),budget=new RecordedBudget(16777216),output=await RecordedOutputClient.open(await load('converter.wasm'),4194304,budget);output.volume(0);await output.resume();
   const channel=new MessageChannel(),sender=new RecordedMusicSender(channel.port1,budget);if(!await output.connectMusic(sender,channel.port2))throw Error('Frame music connection');
   const loader=new RecordedMusicLoader(sender,await load('decoder.wasm'),budget),dispatcher=new RecordedFrameDispatcher(output,new Map([['tone.mp3',await load('tone.mp3')],['tone.wav',await load('tone.wav')],['bad.mp3',new Uint8Array([1])]]),{effects:true,music:true,musicLoader:loader});
   const frame=(serial,recordedControls)=>({epoch:0,serial,recordedControls,recordedActiveGeneration:null}),initial=budget.used;
   const BaseWorker=window.Worker;let created=0,terminated=0;window.Worker=class extends BaseWorker{constructor(...args){super(...args);created++;}terminate(){terminated++;return super.terminate();}};
   if(!await dispatcher.prepare(frame(1,[[4,'tone.mp3',0],[6,'',1]]))||loader.loading||budget.used!==initial||!await dispatcher.finish(0,1,false))throw Error('Discarded Play started decoder');
   if(!await dispatcher.prepare(frame(2,[[2,'',1],[0,'tone.wav',0],[4,'tone.mp3',0],[6,'',1]]))||!await dispatcher.finish(0,2,true))throw Error('Combined committed frame failed');
   const wait=async predicate=>{const end=performance.now()+3000;for(;;){const state=await output.request('snapshot');if(predicate(state))return state;if(performance.now()>end)throw Error('Frame music stalled '+JSON.stringify(state));await new Promise(r=>setTimeout(r,5));}};
   const paused=await wait(s=>s.queuedFrames===4800);if(paused.playedFrames||paused.musicStatus!=='paused'||paused.activeEffects!==1)throw Error('Combined frame pause changed playback');
   if(!await dispatcher.prepare(frame(3,[[7,'',0]]))||!await dispatcher.finish(0,3,false))throw Error('Failed Stop checkpoint');const preserved=await output.request('snapshot');if(preserved.generation!==paused.generation||preserved.queuedFrames!==4800)throw Error('Failed Stop changed music');
   if(!await dispatcher.prepare(frame(4,[[4,'bad.mp3',0]]))||!await dispatcher.finish(0,4,true))throw Error('Async failure Play not committed');await wait(s=>s.musicStatus==='failed');
   if(!await dispatcher.prepare(frame(5,[[3,'',0],[4,'tone.mp3',0]]))||!await dispatcher.finish(0,5,true))throw Error('Fresh music recovery rejected');const ended=await wait(s=>s.musicStatus==='ended');if(ended.playedFrames!==11520||ended.activeEffects)throw Error('Fresh music incomplete');
   if(!await dispatcher.prepare(frame(6,[[4,'tone.mp3',0]]))||!await dispatcher.finish(0,6,true)||!loader.loading)throw Error('Fresh startup not queued');
   if(!await dispatcher.prepare(frame(7,[[7,'',0]]))||!await dispatcher.finish(0,7,true))throw Error('Committed Stop rejected');await wait(s=>s.musicStatus==='stopped'&&s.queuedFrames===0);while(loader.loading)await new Promise(r=>setTimeout(r,1));if(budget.used!==initial)throw Error('Stop kept decoder reservations');
   if(!await dispatcher.prepare(frame(8,[[4,'bad.mp3',0],[4,'tone.mp3',0]]))||!await dispatcher.finish(0,8,true)||!loader.loading)throw Error('Final replacement startup not queued');await dispatcher.close();window.Worker=BaseWorker;if(budget.used||created!==5||terminated!==created)throw Error('Closing music frame leaked reservations/workers or decoded an intermediate Play '+JSON.stringify({created,terminated}));return {discardedPlay:true,combinedPause:true,discardedStop:true,committedStop:true,asyncFailure:true,recoveredFrames:ended.playedFrames,closedDuringLoad:true,created,terminated};
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in musicFrames,musicFrames
  print('Production combined frame dispatcher: discarded Play/Stop, committed paused effects/music, asynchronous codec failure, fresh recovery and close during startup passed.',json.dumps(musicFrames['value']),flush=True)
  sessions=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {RecordedSession}=await import('./recorded-session.js'),{RecordedBudget}=await import('./recorded-budget.js'),load=async name=>new Uint8Array(await(await fetch(name)).arrayBuffer());
   const converter=await load('converter.wasm'),decoder=await load('decoder.wasm'),assets=new Map([['tone.wav',await load('tone.wav')],['tone.mp3',await load('tone.mp3')]]),results=[];
   for(const [effects,music] of [[4194304,8388608],[4194304,0],[0,8388608]]){
    const root=new RecordedBudget(16777216),manifest={permissions:{audio:true},limits:{hard_memory_bytes:16777216,...(effects?{effects_memory_bytes:effects}:{}),...(music?{music_memory_bytes:music}:{})}},session=await RecordedSession.open(manifest,assets,converter,decoder,root);
    if(session.guestBytes!==16777216-effects-music||root.used!==root.limit)throw Error('Independent session allowance escaped parent');
    const script=URL.createObjectURL(new Blob(['const memory=new WebAssembly.Memory({initial:1,maximum:'+Math.floor(session.guestBytes/65536)+'});postMessage(memory.buffer.byteLength);'],{type:'application/javascript'})),guest=new Worker(script);let terminated=false;const terminate=guest.terminate.bind(guest);guest.terminate=()=>{terminated=true;terminate();};
    if(!session.attachGuest(guest))throw Error('Guest ownership rejected');const allocated=await new Promise(resolve=>guest.onmessage=event=>resolve(event.data));URL.revokeObjectURL(script);if(allocated!==65536)throw Error('Guest memory not instantiated');session.volume(0);await session.resume();
    const commands=[...(effects?[[2,'',1],[0,'tone.wav',0]]:[]),...(music?[[4,'tone.mp3',0],[6,'',1]]:[])];if(!await session.prepare({epoch:0,serial:1,recordedControls:commands})||!await session.finish(0,1,true))throw Error('Declared session frame rejected');
    let state;const end=performance.now()+3000;do{state=await session.snapshot();if(!music||state.queuedFrames===4800)break;if(performance.now()>end)throw Error('Session prefill stalled');await new Promise(r=>setTimeout(r,5));}while(true);
    if(!state.ready||state.activeEffects!==(effects?1:0)||state.playedFrames||state.musicStatus!==(music?'paused':'stopped'))throw Error('Declared session state mismatch');
    const closing=session.close();if(!terminated||root.used>effects+music)throw Error('Guest reservation outlived termination ordering');await closing;if(root.used)throw Error('Session retained parent capacity');results.push({effects,music,guestBytes:session.guestBytes,terminated});
   }
   const manifest={permissions:{audio:true},limits:{hard_memory_bytes:16777216,effects_memory_bytes:4194304,music_memory_bytes:8388608}},root=new RecordedBudget(16777216),abort=new AbortController(),opening=RecordedSession.open(manifest,assets,converter,decoder,root,abort.signal);abort.abort();let denied=false;try{await opening;}catch{denied=true;}if(!denied||root.used)throw Error('Session startup cancellation leaked capacity');
   for(const bad of [{...manifest,permissions:{audio:false}},{...manifest,limits:{...manifest.limits,music_memory_bytes:16777216}},{...manifest,limits:{hard_memory_bytes:16777216,effects_memory_bytes:65536}}]){denied=false;try{await RecordedSession.open(bad,assets,converter,decoder,root);}catch{denied=true;}if(!denied||root.used)throw Error('Invalid share/permission admitted or leaked');}
   const {RecordedOutputClient}=await import('./recorded-output-client.js'),connect=RecordedOutputClient.prototype.connectMusic;let release,entered=false;
   RecordedOutputClient.prototype.connectMusic=async function(...args){entered=true;await new Promise(resolve=>release=resolve);return connect.apply(this,args);};const lateAbort=new AbortController(),late=RecordedSession.open(manifest,assets,converter,decoder,root,lateAbort.signal);const observed=late.then(()=>false,()=>true);
   while(!entered)await new Promise(r=>setTimeout(r,1));lateAbort.abort();release();const cancelled=await observed;RecordedOutputClient.prototype.connectMusic=connect;if(!cancelled||root.used)throw Error('Music connection cancellation leaked session ownership');
   return results;
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in sessions,sessions
  print('Production declared-share sessions: combined, effects-only and music-only real guest workers, one parent ceiling, paused playback, ordered shutdown and cancelled/invalid startup passed.',json.dumps(sessions['value']),flush=True)
 finally:
  browser.quit();server.shutdown();server.server_close()
