"""Actual bounded live audio FIFO and output clock against independent SDL hashes."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import json,os,shutil,sys,tempfile,threading
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
 for name in ['recorded-decoder.js','recorded-mixer.js','recorded-queue.js','recorded-stream.js']:shutil.copy2(project/'web-player'/name,root/name)
 shutil.copy2(project/'tests/fixtures/recorded-stream-worklet.js',root/'stream-worklet.js');shutil.copy2(companion,root/'converter.wasm')
 shutil.copy2(project/'tests/fixtures/sdl-live-audio-reference.json',root/'reference.json')
 (root/'index.html').write_text('<script type="module">window.mixerReady=true;</script>')
 server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
 options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert options.binary_location
 for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage']:options.add_argument(flag)
 driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
 browser=webdriver.Chrome(service=Service(driver),options=options)
 try:
  browser.get(f'http://127.0.0.1:{server.server_port}/');WebDriverWait(browser,20).until(lambda b:b.execute_script('return !!window.mixerReady'))
  browser.set_script_timeout(30)
  result=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const reference=await(await fetch('reference.json')).json(),wasm=new Uint8Array(await(await fetch('converter.wasm')).arrayBuffer()),results=[];
   for(const record of [...reference.cases,{...reference.cases[6],music_volume:128,mode:'pause'},{...reference.cases[0],effect_volume:128,mode:'pause-effects'},{...reference.cases[6],music_volume:128,mode:'fail'},{...reference.cases[6],music_volume:128,mode:'stop'},{...reference.cases[6],music_volume:128,mode:'underrun'},{...reference.cases[3],masterMuted:true}]){
    const context=new OfflineAudioContext(2,record.frames+128,48000);await context.audioWorklet.addModule('stream-worklet.js');
    const node=new AudioWorkletNode(context,'reference-live-stream',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{wasm,music_volume:record.music_volume,effect_volume:record.effect_volume,mode:record.mode}});
    const finished=new Promise((resolve,reject)=>{node.port.onmessage=({data})=>{if(data.error)reject(Error(data.error));else resolve(data);};node.onprocessorerror=()=>reject(Error('AudioWorklet processor failed'));});
    const master=context.createGain();master.gain.value=record.masterMuted?0:1;node.connect(master);master.connect(context.destination);const rendered=await context.startRendering(),report=await finished;
    if(report.frames!==record.frames||report.buffers>65536)throw Error('Wrong live output count or buffer bound');
    const quiet=['pause','fail','stop','underrun'].includes(record.mode),status={pause:'paused',fail:'failed',stop:'stopped',underrun:'loading'}[record.mode]||'ended';
    if(report.played!==(quiet?0:6000)||report.status!==status||report.queued!==(record.mode==='pause'?4800:0)||report.active!==(record.mode==='pause-effects'?4:0))throw Error('Live state mismatch: '+JSON.stringify(report));
    const pcm=new Uint8Array(record.frames*4),view=new DataView(pcm.buffer),channels=[rendered.getChannelData(0),rendered.getChannelData(1)];
    for(let i=0;i<record.frames;i++)for(let c=0;c<2;c++){const sample=channels[c][i]*32768;if(!Number.isInteger(sample)||sample<-32768||sample>32767)throw Error('Output lost S16 precision');view.setInt16(i*4+c*2,sample,true);}
    for(const channel of channels)if(!channel.subarray(record.frames).every(value=>value===0))throw Error('Stale output after mixer finished');
    const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',pcm)),byte=>byte.toString(16).padStart(2,'0')).join('');if(record.masterMuted){if(!pcm.every(value=>value===0))throw Error('Muted master leaked output');}else if(hash!==record.sha256)throw Error('SDL worklet hash mismatch at volume '+record.effect_volume+': '+hash);
    node.disconnect();master.disconnect();node.port.close();results.push({mode:record.mode||'playing',frames:record.frames,status:report.status,played:report.played,sha256:hash,masterMuted:!!record.masterMuted});
   }
   return results;
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in result,result
  assert len(result['value'])==14,result
  print('Actual Chromium live audio: fourteen exact SDL/zero-output prefixes; fixed FIFO, completion, pause/mute, failure/stop/underrun with continuing effects, stale generations and clean tails passed.',json.dumps(result['value']),flush=True)
 finally:
  browser.quit();server.shutdown();server.server_close()
