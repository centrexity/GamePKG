"""Actual AudioWorklet output-clock mixing against independent pinned SDL hashes."""
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
 for name in ['recorded-decoder.js','recorded-mixer.js']:shutil.copy2(project/'web-player'/name,root/name)
 shutil.copy2(project/'tests/fixtures/recorded-mixer-worklet.js',root/'mixer-worklet.js');shutil.copy2(companion,root/'converter.wasm')
 shutil.copy2(project/'tests/fixtures/sdl-s16-mix-reference.json',root/'reference.json')
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
   for(const record of [...reference.combined_streams,{...reference.combined_streams[0],volume:128,paused:true},{...reference.combined_streams[3],masterMuted:true}]){
    const context=new OfflineAudioContext(2,record.frames+128,48000);await context.audioWorklet.addModule('mixer-worklet.js');
    const node=new AudioWorkletNode(context,'reference-mixer',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{wasm,volume:record.volume,paused:!!record.paused}});
    const finished=new Promise((resolve,reject)=>{node.port.onmessage=({data})=>{if(data.error)reject(Error(data.error));else resolve(data);};node.onprocessorerror=()=>reject(Error('AudioWorklet processor failed'));});
    const master=context.createGain();master.gain.value=record.masterMuted?0:1;node.connect(master);master.connect(context.destination);const rendered=await context.startRendering(),report=await finished;
    if(report.frames!==record.frames)throw Error('Wrong output-clock frame count');
    const pcm=new Uint8Array(record.frames*4),view=new DataView(pcm.buffer),channels=[rendered.getChannelData(0),rendered.getChannelData(1)];
    for(let i=0;i<record.frames;i++)for(let c=0;c<2;c++){const sample=channels[c][i]*32768;if(!Number.isInteger(sample)||sample<-32768||sample>32767)throw Error('Output lost S16 precision');view.setInt16(i*4+c*2,sample,true);}
    for(const channel of channels)if(!channel.subarray(record.frames).every(value=>value===0))throw Error('Stale output after mixer finished');
    const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',pcm)),byte=>byte.toString(16).padStart(2,'0')).join('');if(record.masterMuted){if(!pcm.every(value=>value===0))throw Error('Muted master leaked output');}else if(hash!==record.sha256)throw Error('SDL worklet hash mismatch at volume '+record.volume+': '+hash);
    node.disconnect();master.disconnect();node.port.close();results.push({volume:record.volume,frames:record.frames,sha256:hash,masterMuted:!!record.masterMuted});
   }
   return results;
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in result,result
  assert len(result['value'])==8,result
  print('Actual Chromium AudioWorklet: six complete SDL music/effect conversion hashes plus paused-effect music and output lengths match without game ticks; muted-master advancement and clean output tails.',json.dumps(result['value']),flush=True)
 finally:
  browser.quit();server.shutdown();server.server_close()
