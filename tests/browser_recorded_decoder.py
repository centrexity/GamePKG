"""Actual Chromium decoder ABI/fault checks and a complete private original track."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import json,os,shutil,sys,tempfile,threading
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
project=Path(__file__).resolve().parents[1]
fixture,music,companion=map(Path,sys.argv[1:4])
record=json.loads((fixture/'reference.json').read_text())[0]
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-chromium-decoder-') as temporary:
 root=Path(temporary)
 for name in ['recorded-decoder.js','recorded-decoder-client.js','recorded-decoder-worker.js']:shutil.copy2(project/'web-player'/name,root/name)
 for file in fixture.glob('*.wasm'):shutil.copy2(file,root/file.name)
 shutil.copy2(companion,root/'decoder.wasm');shutil.copy2(music/record['name'],root/'track.mp3')
 (root/'reference.json').write_text(json.dumps(record))
 (root/'index.html').write_text('<script type="module">import * as api from "./recorded-decoder.js";window.decoderAPI=api;</script>')
 server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
 options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert options.binary_location
 for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage']:options.add_argument(flag)
 driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
 browser=webdriver.Chrome(service=Service(driver),options=options)
 try:
  browser.get(f'http://127.0.0.1:{server.server_port}/');WebDriverWait(browser,20).until(lambda b:b.execute_script('return !!window.decoderAPI'))
  browser.set_script_timeout(30)
  result=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {RecordedDecoder}=window.decoderAPI,assert=(value,message)=>{if(!value)throw Error(message);};
   const load=async name=>new Uint8Array(await(await fetch(name+'.wasm')).arrayBuffer());
   for(const name of ['signature','import','table','unbounded','memory-max','start','multi-memory','memory-export']){
    let denied=false;try{new RecordedDecoder(await load(name));}catch{denied=true;}assert(denied,'accepted invalid ABI: '+name);
   }
   for(const name of ['negative','odd','oversize','range','work','open','rewind']){
    const decoder=new RecordedDecoder(await load(name)),output=new Int16Array(2).fill(2345);let denied=false;
    try{decoder.open(new Uint8Array([1]));if(name==='rewind')decoder.rewind();else decoder.read(output);}catch{denied=true;}
    assert(denied&&decoder.failed&&output[0]===2345&&output[1]===2345,'invalid output escaped: '+name);decoder.close();
   }
   const reference=await(await fetch('reference.json')).json(),wasm=await load('decoder'),encoded=new Uint8Array(await(await fetch('track.mp3')).arrayBuffer());
   const decoder=new RecordedDecoder(wasm);assert(decoder.maximumMemoryBytes===6291456,'memory maximum changed');decoder.open(encoded);
   const pcm=new Uint8Array(reference.samples*2),view=new DataView(pcm.buffer),output=new Int16Array(1600);let at=0,step=0,maximumReadMs=0,first;
   for(;;){output.fill(2345);const requested=[2,226,14,1598,1600][step++%5],start=performance.now(),count=decoder.read(output.subarray(0,requested));maximumReadMs=Math.max(maximumReadMs,performance.now()-start);
    assert(output.subarray(count).every(value=>value===2345),'output ownership');if(count===0)break;if(!first)first=output.slice(0,count);assert(at+count*2<=pcm.length,'too many samples');for(let i=0;i<count;i++)view.setInt16(at+i*2,output[i],true);at+=count*2;
   }
   assert(at===pcm.length,'incomplete track');assert(maximumReadMs<=100,'trusted track exceeded existing per-read decoder deadline');
   const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',pcm)),byte=>byte.toString(16).padStart(2,'0')).join('');assert(hash===reference.sha256,'complete PCM hash mismatch');
   decoder.rewind();const repeated=new Int16Array(first.length);assert(decoder.read(repeated)===first.length&&String(repeated)===String(first),'rewind mismatch');decoder.close();
   return {name:reference.name,samples:reference.samples,sha256:hash,maximumReadMs};
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in result,result
  assert result['value']['sha256']==record['sha256']
  print('Actual Chromium: rejected malformed ABI/output, preserved output ownership, complete original PCM and rewind match native:',json.dumps(result['value']),flush=True)
  supervised=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {RecordedDecoderClient}=await import('./recorded-decoder-client.js'),assert=(value,message)=>{if(!value)throw Error(message);};
   const load=async name=>new Uint8Array(await(await fetch(name+'.wasm')).arrayBuffer());
   const Base=window.Worker;let created=0,terminated=0;window.Worker=class extends Base{constructor(...args){super(...args);created++;}terminate(){terminated++;super.terminate();}};
   try{
    const wasm=await load('valid'),encoded=new Uint8Array([1]),client=await RecordedDecoderClient.open(wasm,encoded);
    assert(wasm.length>0&&encoded[0]===1,'caller input was detached');const first=await client.read(8);assert(String(first)==='4660,-292','worker PCM changed');
    let invalid=false;try{await client.read(1);}catch{invalid=true;}assert(invalid&&!client.closed,'invalid caller size poisoned active worker');await client.rewind();assert(String(await client.read(2))==='4660,-292','worker rewind failed');const inFlight=client.read(2);let busy=false;try{await client.read(2);}catch{busy=true;}assert(busy&&String(await inFlight)==='4660,-292'&&!client.closed,'backpressure lost active read');client.close();assert(client.closed,'close did not terminate');
    const abort=new AbortController(),opening=RecordedDecoderClient.open(wasm,encoded,6291456,abort.signal);abort.abort();let cancelled=false;try{await opening;}catch{cancelled=true;}assert(cancelled,'aborted startup revived');
    const hanging=await RecordedDecoderClient.open(await load('hang-read'),encoded);let timeout=false;try{await hanging.read(2);}catch(error){timeout=String(error).includes('deadline');}assert(timeout&&hanging.closed,'infinite decoder read escaped supervisor');
    timeout=false;try{await RecordedDecoderClient.open(await load('hang-open'),encoded);}catch(error){timeout=String(error).includes('deadline');}assert(timeout,'infinite decoder open escaped supervisor');
    const fault=await RecordedDecoderClient.open(await load('range'),encoded);let denied=false;try{await fault.read(2);}catch{denied=true;}assert(denied&&fault.closed,'invalid PCM worker survived');
    const pending=await RecordedDecoderClient.open(wasm,encoded),reading=pending.read(2);pending.close();let closed=false;try{await reading;}catch{closed=true;}assert(closed&&pending.closed,'closed read completed');
    const recovered=await RecordedDecoderClient.open(wasm,encoded);assert(String(await recovered.read(2))==='4660,-292','fresh worker failed after cancellations');recovered.close();
    const reference=await(await fetch('reference.json')).json(),real=await RecordedDecoderClient.open(await load('decoder'),new Uint8Array(await(await fetch('track.mp3')).arrayBuffer()));
    const pcm=new Uint8Array(reference.samples*2),view=new DataView(pcm.buffer);let at=0;
    for(;;){const values=await real.read();if(!values.length)break;assert(at+values.length*2<=pcm.length,'too much worker PCM');for(let i=0;i<values.length;i++)view.setInt16(at+i*2,values[i],true);at+=values.length*2;}
    assert(at===pcm.length,'incomplete worker track');const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',pcm)),byte=>byte.toString(16).padStart(2,'0')).join('');assert(hash===reference.sha256,'complete worker PCM hash mismatch');real.close();
    assert(created===terminated,'orphan decoder workers');return {created,terminated,samples:reference.samples,sha256:hash};
   }finally{window.Worker=Base;}
  })().then(value=>done({value})).catch(error=>done({error:String(error.stack||error)}));''')
  assert 'error' not in supervised,supervised
  print('Actual Chromium supervised decoder: 100 ms open/read loop termination, cancellation, invalid-output cleanup, fresh recovery and no orphan workers:',supervised['value'],flush=True)

 finally:
  browser.quit();server.shutdown();server.server_close()
