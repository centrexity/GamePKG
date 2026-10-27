"""Actual worker PNG byte parity; all generated reports and copies stay private."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,os,shutil,tempfile,threading
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
parser=argparse.ArgumentParser();parser.add_argument('report',type=Path);parser.add_argument('--assets',type=Path);args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-exact-png-') as temporary:
 root=Path(temporary)
 for name in ['png-exact.js','png-inflate.js']:shutil.copy2(project/'web-player'/name,root/name)
 report=json.loads(args.report.read_text());(root/'report.json').write_text(json.dumps(report))
 for row in report['images']:
  name=row['asset'];assert not Path(name).is_absolute() and '..' not in Path(name).parts
  destination=root/'assets'/name;destination.parent.mkdir(parents=True,exist_ok=True);shutil.copy2((args.assets or project/'ports/arkanoo/assets')/name,destination)
 (root/'index.html').write_text('<!doctype html><title>Private exact PNG verification</title>')
 (root/'worker.js').write_text('''onmessage=async()=>{try{
 const {decodeExactPNG}=await import('./png-exact.js'),report=await (await fetch('report.json')).json();let checked=0;
 const inputs=await Promise.all(report.images.map(async row=>({row,bytes:new Uint8Array(await (await fetch('assets/'+row.asset)).arrayBuffer())}))),consecutive=[];
 // Startup decodes already-loaded images consecutively. Fetch/hash awaits
 // between images used to hide an optimized-browser decoder-construction regression.
 for(const {row,bytes} of inputs){postMessage({stage:row.asset});const decoded=await decodeExactPNG(bytes);if(!decoded)throw Error('Unsupported original image '+row.asset);consecutive.push({row,decoded});}
 for(const {row,decoded} of consecutive){const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',decoded.pixels)),v=>v.toString(16).padStart(2,'0')).join('');if(hash!==row.rgba_sha256)throw Error('Consecutive pixel mismatch '+row.asset);}
 for(const {row,bytes} of inputs){const decoded=await decodeExactPNG(bytes);if(!decoded)throw Error('Unsupported original image '+row.asset);
 const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',decoded.pixels)),v=>v.toString(16).padStart(2,'0')).join('');if(hash!==row.rgba_sha256)throw Error('Pixel mismatch '+row.asset);checked++;
 const broken=bytes.slice();broken[broken.length-1]^=1;let rejected=false;try{await decodeExactPNG(broken);}catch{rejected=true;}if(!rejected)throw Error('Bad CRC accepted');}
 postMessage({checked});}catch(error){postMessage({error:String(error)});}};
''')
 server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
 options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('chromium') or shutil.which('google-chrome-stable')
 for flag in ['--headless=new','--mute-audio','--no-sandbox','--disable-dev-shm-usage']:options.add_argument(flag)
 driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
 driver_log=root/'chromedriver.log'
 try:browser=webdriver.Chrome(service=Service(driver,log_output=str(driver_log)),options=options)
 except Exception:
  print('Chrome startup log:',driver_log.read_text(errors='replace')[-8192:] if driver_log.exists() else 'unavailable',flush=True)
  server.shutdown();server.server_close();raise
 try:
  browser.set_script_timeout(30);browser.get(f'http://127.0.0.1:{server.server_port}/')
  result=browser.execute_async_script('const done=arguments[arguments.length-1],worker=new Worker("worker.js");let last;const timer=setTimeout(()=>{worker.terminate();done({error:"PNG startup deadline exceeded",last});},10000);worker.onmessage=({data})=>{if(data.stage){last=data.stage;return;}clearTimeout(timer);worker.terminate();done(data);};worker.onerror=e=>{clearTimeout(timer);worker.terminate();done({error:e.message});};worker.postMessage({});')
  assert result=={'checked':76},result
  print('Actual Chromium worker: consecutive startup and individual decoding preserve all 76 original PNG RGBA hashes; corrupted CRC rejects.')
 finally:browser.quit();server.shutdown();server.server_close()
