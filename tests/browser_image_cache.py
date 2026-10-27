"""Signed nearest-only exports must draw every image beyond the decoded cache size."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,hashlib,json,os,shutil,struct,subprocess,tempfile,threading,zlib
from PIL import Image
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--assets',type=Path,help='Private lossless PNG corpus exceeding 32 MiB decoded; default uses generated fixtures')
args=parser.parse_args()
inputs=sorted(args.assets.glob('*.png')) if args.assets else [None]*36
assert 1<=len(inputs)<=128
cycle=len(inputs)+1
project=Path(__file__).resolve().parents[1]
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
with tempfile.TemporaryDirectory(prefix='gamepkg-image-cache-') as temporary:
 root=Path(temporary);(root/'key').write_text('73'*32);(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*args):
  result=subprocess.run([str(project/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=180)
  assert result.returncode==0,result.stdout+'\n'+result.stderr
 expected=[]
 for profile in ['core-v1','component-v1']:
  recipe=root/(profile+'-source');shutil.copytree(project/'examples/minimal',recipe)
  decoded_bytes=0
  for number,source in enumerate(inputs):
   destination=recipe/f'assets/image-{number}.png'
   if source:
    assert source.is_file() and not source.is_symlink()
    with Image.open(source) as original:image=original.convert('RGBA')
    assert 1<=image.width<=512 and 1<=image.height<=512
    shutil.copyfile(source,destination)
   else:
    color=(number*7&255,number*17&255,number*29&255,0 if number%3==0 else 255)
    image=Image.new('RGBA',(512,512),color);image.save(destination)
   decoded_bytes+=len(image.tobytes())
   if profile=='core-v1':expected.append(hashlib.sha256(image.tobytes()).hexdigest())
  assert decoded_bytes>33554432,'Corpus must force decoded-cache eviction'
  # Opaque 16-bit RGBA is not supported by the exact JS decoder. It must
  # receive the host's precise fallback even without a straight-alpha import.
  pixels=bytes([203,27,46,255,71,93,117,255]);header=struct.pack('>IIBBBBB',2,1,16,6,0,0,0)
  png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',header)+chunk(b'IDAT',zlib.compress(b'\0'+bytes(v for c in pixels for v in [c,c])))+chunk(b'IEND',b'')
  (recipe/'assets/fallback.png').write_bytes(png)
  if profile=='core-v1':expected.append(hashlib.sha256(pixels).hexdigest())
  cases='\n'.join(f'{i} => gamepkg::draw_asset_image("image-{i}.png",0.0,0.0,512.0,472.0),' for i in range(len(inputs)))
  (recipe/'source/src/lib.rs').write_text('pub fn game_tick(_:u32,state:u64)->u64 {gamepkg::draw_clear(0x193249ff);let ok=match state%'+str(cycle)+' {'+cases+'_ => gamepkg::draw_asset_image("fallback.png",0.0,0.0,512.0,472.0),};if ok {state+1} else {1u64<<63}}')
  manifest=recipe/'manifest.toml';id='com.gamepkg.image-cache-'+profile
  manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.minimal',id))
  archive=root/(profile+'.gamepkg');run('pack',recipe,'-o',archive,'--key',root/'key');run('install',archive,'--trust-creator');run('build',id,'--target','wasm');run('web-export',id,'-o',root/profile)
  metadata=json.loads((root/profile/'game.json').read_text());assert sorted(metadata['exact_png_images'])==sorted(f'image-{i}.png' for i in range(len(inputs)));assert list(metadata['precise_images'])==['fallback.png']
  assert sum(len(v)//2 if isinstance(v,str) else len(v['zlib'])//2 for v in metadata['assets'].values())+len(metadata['precise_images']['fallback.png']['rgba'])//2<=33554432
 server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
 options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('chromium') or shutil.which('google-chrome-stable')
 for flag in ['--headless=new','--mute-audio','--no-sandbox','--disable-dev-shm-usage']:options.add_argument(flag)
 driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
 browser=webdriver.Chrome(service=Service(driver),options=options)
 try:
  browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':'window.cacheFrameLimit='+str(2*cycle+4)+';'+'''window.cacheFrames=[];const Base=Worker;window.Worker=class extends Base{constructor(...args){super(...args);this.addEventListener('message',async({data})=>{if(data.type!=='frame'||window.cacheFrames.length>=window.cacheFrameLimit)return;for(const command of data.commands){if(command[0]!=='image')continue;const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',command[7])),v=>v.toString(16).padStart(2,'0')).join('');window.cacheFrames.push({state:data.state,hash});}});}};'''})
  def start(profile):
   browser.get(f'http://127.0.0.1:{server.server_port}/{profile}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();return wait
  def complete(wait):
   def frames_ready(b):
    status=b.find_element('id','status').text
    assert 'Failed' not in status and 'Error' not in status,status
    frames=b.execute_script('return window.cacheFrames')
    for frame in frames:
     state=int(frame['state']);assert 1<=state<10000,frame
     assert frame['hash']==expected[(state-1)%cycle],frame
    return len({int(frame['state']) for frame in frames if 1<=int(frame['state'])<=2*cycle})==2*cycle
   wait.until(frames_ready);browser.find_element('id','stop').click()
  for profile in ['core-v1','component-v1']:
   complete(start(profile))
   # Startup must validate all representations, including an image that will
   # be evicted before gameplay starts; a restored new worker must recover.
   path=root/profile/'game.json';original=path.read_text();broken=json.loads(original);encoded=broken['assets']['image-0.png'];broken['assets']['image-0.png']=encoded[:-2]+f'{int(encoded[-2:],16)^1:02x}';path.write_text(json.dumps(broken))
   wait=start(profile);wait.until(lambda b:'CRC' in b.find_element('id','status').text)
   path.write_text(original);complete(start(profile))
   print(profile,f'signed nearest-only export: {len(inputs)} PNGs ({decoded_bytes} decoded bytes) plus opaque 16-bit fallback, two complete exact-pixel cycles, corrupt early-image rejection and fresh recovery; cache/frame/export limits unchanged.',flush=True)
 finally:browser.quit();server.shutdown();server.server_close()
