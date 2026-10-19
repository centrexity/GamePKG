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
 for name in ['recorded-decoder.js','recorded-mixer.js','recorded-queue.js','recorded-stream.js','recorded-transport.js','recorded-decoder-client.js','recorded-decoder-worker.js']:shutil.copy2(project/'web-player'/name,root/name)
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
   const direct=new RecordedDecoder(codec);direct.open(encoded);const pcm=[];for(;;){const block=new Int16Array(1600),n=direct.read(block);if(!n)break;pcm.push(...block.subarray(0,n));}direct.close();
   if(pcm.length<20000||pcm.length>30000)throw Error('Unexpected synthetic MP3 length');
   const frames=16384,baseline=new RecordedStream(wasm);baseline.begin(1);baseline.effectVolume(64);
   for(let slot=0;slot<4;slot++){const count=[1025,2051,4096,777][slot],bytes=new Uint8Array(count*4),view=new DataView(bytes.buffer);for(let i=0;i<count;i++)for(let c=0;c<2;c++)view.setInt16(i*4+c*2,(i*(slot*1543+197)+c*12345+slot*313)%65536-32768,true);baseline.load(slot,bytes);baseline.play(slot);}
   let at=0,ended=false;const expected=new Int16Array(frames*2);
   for(let frame=0;frame<frames;frame+=128){while(at<pcm.length&&baseline.availableFrames>=800){const n=Math.min(1600,pcm.length-at);if(!baseline.push(1,Int16Array.from(pcm.slice(at,at+n))))throw Error('baseline enqueue');at+=n;}if(at===pcm.length&&!ended){baseline.finishMusic(1);ended=true;}baseline.render(expected.subarray(frame*2,(frame+128)*2));}
   if(baseline.musicStatus!=='ended'||baseline.playedFrames!==pcm.length/2)throw Error('baseline incomplete');baseline.close();
   const results=[];for(const mode of ['normal','replacement','stop-restart']){
   const client=await RecordedDecoderClient.open(codec,encoded),context=new AudioContext({sampleRate:48000});await context.audioWorklet.addModule('stream-worklet.js');
   const node=new AudioWorkletNode(context,'transport-audio',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{wasm,volume:64,frames}});
   const channel=new MessageChannel(),sender=new RecordedMusicSender(channel.port1);node.port.postMessage({port:channel.port2},[channel.port2]);
   const finished=new Promise((resolve,reject)=>{node.port.onmessage=({data})=>data.error?reject(Error(data.error)):resolve(data);node.onprocessorerror=()=>reject(Error('decoder worklet failed'));});
   const gain=context.createGain();gain.gain.value=0;node.connect(gain);gain.connect(context.destination);await context.resume();let retired=null;
   if(mode==='normal')sender.start(1,client);else{
    retired=await RecordedDecoderClient.open(codec,encoded);let release;
    const held={async read(size){const values=await retired.read(size);return new Promise(resolve=>release=()=>resolve(values.fill(32767)));},close(){retired.close();}};
    sender.start(1,held);const deadline=performance.now()+2000;while(!release){if(performance.now()>deadline)throw Error('Retired read not admitted');await new Promise(resolve=>setTimeout(resolve,1));}
    if(mode==='stop-restart'){if(!sender.stop(2))throw Error('Stop rejected');sender.start(3,client);}else sender.start(2,client);
    if(!retired.closed)throw Error('Replaced decoder alive');release();
   }
   const report=await finished;
   if(report.status!=='ended'||report.played!==pcm.length/2||!client.closed||report.values.length!==expected.length)throw Error('worker pipeline incomplete');
   for(let i=0;i<expected.length;i++)if(report.values[i]!==expected[i])throw Error('worker pipeline differs at scalar '+i+': '+report.values[i]+' != '+expected[i]);
   sender.close();node.disconnect();node.port.close();gain.disconnect();await context.close();results.push({mode,sourceFrames:report.played,comparedScalars:expected.length,workerClosed:client.closed,retiredClosed:retired?.closed??true});
   }return results;
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in decoded,decoded
  print('Actual supervised MP3 decoder worker -> credited MessagePort -> live AudioWorklet matches every direct mixed scalar:',json.dumps(decoded['value']),flush=True)
 finally:
  browser.quit();server.shutdown();server.server_close()
