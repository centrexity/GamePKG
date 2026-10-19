"""Signed exports must carry one PNG source and emit exact straight-alpha pixels."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import hashlib,json,os,shutil,subprocess,tempfile,threading
from PIL import Image
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
project=Path(__file__).resolve().parents[1]
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-exact-png-player-') as temporary:
 root=Path(temporary);(root/'key').write_text('65'*32);(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*args):
  result=subprocess.run([str(project/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stdout+'\n'+result.stderr
 for profile in ['core-v1','component-v1']:
  recipe=root/(profile+'-source');shutil.copytree(project/'examples/minimal',recipe)
  manifest=recipe/'manifest.toml';id='com.gamepkg.exact-png-'+profile
  manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.minimal',id))
  for name in ['heart.png','ball.png']:shutil.copy2(project/'ports/arkanoo/assets'/name,recipe/'assets'/name)
  packed=Image.new('P',(2,1));packed.putpalette([17,34,51,100,110,120]+[0]*762);packed.putdata([0,1]);packed.save(recipe/'assets/packed.png',bits=2,transparency=bytes([0,127]))
  Image.new('RGB',(2,1),(17,34,51)).save(recipe/'assets/fallback.png',transparency=(17,34,51))
  (recipe/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,_:u64)->u64 {gamepkg::draw_clear(0x193249ff);let heart=gamepkg::draw_asset_image_linear_straight("heart.png",0.0,0.0,512.0,472.0);let ball=gamepkg::draw_asset_image_linear_straight("ball.png",520.0,0.0,32.0,32.0);let fallback=gamepkg::draw_asset_image_linear_straight("fallback.png",560.0,0.0,2.0,1.0);let packed=gamepkg::draw_asset_image_linear_straight("packed.png",570.0,0.0,2.0,1.0);if heart&&ball&&fallback&&packed {1}else{2}}''')
  run('pack',recipe,'-o',root/(profile+'.gamepkg'),'--key',root/'key');run('install',root/(profile+'.gamepkg'),'--trust-creator');run('build',id,'--target','wasm');run('web-export',id,'-o',root/profile)
  metadata=json.loads((root/profile/'game.json').read_text());assert sorted(metadata['exact_png_images'])==['ball.png','heart.png','packed.png'];assert list(metadata['precise_images'])==['fallback.png']
 server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
 options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('chromium') or shutil.which('google-chrome-stable')
 for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage']:options.add_argument(flag)
 driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
 browser=webdriver.Chrome(service=Service(driver),options=options)
 try:
  browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':'''window.imageHashes=[];const Base=Worker;window.Worker=class extends Base{constructor(...args){super(...args);this.addEventListener('message',async({data})=>{if(data.type!=='frame')return;for(const command of data.commands){if(command[0]!=='straight-linear-image')continue;const bytes=command[7];const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),v=>v.toString(16).padStart(2,'0')).join('');window.imageHashes.push(hash);}});}};'''})
  expected=[]
  for name in ['heart.png','ball.png']:
   with Image.open(project/'ports/arkanoo/assets'/name) as image:expected.append(hashlib.sha256(image.convert('RGBA').tobytes()).hexdigest())
  expected.append(hashlib.sha256(bytes([17,34,51,0])*2).hexdigest())
  expected.append(hashlib.sha256(bytes([17,34,51,0,100,110,120,127])).hexdigest())
  for profile in ['core-v1','component-v1']:
   browser.get(f'http://127.0.0.1:{server.server_port}/{profile}/');wait=WebDriverWait(browser,20);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click()
   wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')=='1');wait.until(lambda b:all(value in b.execute_script('return window.imageHashes') for value in expected))
   browser.find_element('id','stop').click()
   path=root/profile/'game.json';original=path.read_text();broken=json.loads(original);hexbytes=broken['assets']['heart.png'];broken['assets']['heart.png']=hexbytes[:-2]+f'{int(hexbytes[-2:],16)^1:02x}';path.write_text(json.dumps(broken))
   browser.get(f'http://127.0.0.1:{server.server_port}/{profile}/');wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();wait.until(lambda b:'CRC' in b.find_element('id','status').text)
   path.write_text(original);browser.get(f'http://127.0.0.1:{server.server_port}/{profile}/');wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();wait.until(lambda b:b.find_element('id','game').get_attribute('data-state')=='1');browser.find_element('id','stop').click()
   print(profile,'signed export: exact indexed/RGBA pixels without duplicate raw assets, original transparency fallback, CRC rejection and fresh recovery.')
 finally:browser.quit();server.shutdown();server.server_close()
