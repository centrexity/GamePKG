"""Compare batched filtered draws with the pinned previously published renderer."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import hashlib,os,shutil,tempfile,threading
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
project=Path(__file__).resolve().parents[1]
reference=(project/'tests/fixtures/browser-linear-reference.js').read_text();assert hashlib.sha256(reference.encode()).hexdigest()=='28c7002b2e8495b149ffeef6851402449a0d41ba0a17e13c2466b36dd9a5dbf5'
current=(project/'web-player/player.js').read_text();current=current[current.index('// Use one-level GPU'):current.index('function publishFrameIdentity')]
prefix='const context=null,data={custom_renderer:true};function color(rgba){return `rgba(${rgba>>>24},${rgba>>>16&255},${rgba>>>8&255},${(rgba&255)/255})`;}\n'
suffix='''\nexport function draw(frame){const canvas=new OffscreenCanvas(640,480),target=canvas.getContext('2d');let copies=0;const original=target.drawImage;target.drawImage=function(...args){copies++;return original.apply(this,args);};render(frame,target,false);return {pixels:target.getImageData(0,0,640,480).data,copies};}\nexport async function drawCooperatively(frame){const canvas=new OffscreenCanvas(640,480),target=canvas.getContext('2d');let copies=0;const original=target.drawImage;target.drawImage=function(...args){copies++;return original.apply(this,args);};await renderCooperatively(frame,target,false,()=>true);return {pixels:target.getImageData(0,0,640,480).data,copies};}\n'''
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-linear-batch-') as temporary:
 root=Path(temporary);(root/'old.js').write_text(prefix+reference+suffix);(root/'new.js').write_text(prefix+current+suffix);(root/'index.html').write_text('<!doctype html><title>Private filtered-image parity</title>')
 server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
 options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('chromium') or shutil.which('google-chrome-stable')
 for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage']:options.add_argument(flag)
 driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');browser=webdriver.Chrome(service=Service(driver),options=options)
 try:
  browser.set_script_timeout(30);browser.get(f'http://127.0.0.1:{server.server_port}/')
  result=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const old=await import('./old.js'),fresh=await import('./new.js'),pixels=Uint8Array.from({length:64},(_,i)=>i%4===3?[0,63,127,128,254,255][Math.floor(i/4)%6]:(i*79+17)%256);
   const sprite=(i)=>[i%2?'straight-linear-image':'linear-image',(i*17)%620-.25,(i*29)%460+.375,16.5,12.75,4,4,pixels];
   const all=Array.from({length:235},(_,i)=>sprite(i)),cases=[all,all.slice(0,3),[['clip',17.5,30.25,230.5,170.5],...all.slice(0,27),['clip-reset'],...all.slice(27,48)],all.slice(0,8).flatMap((entry,i)=>[entry,['rect',i*7+.25,i*9+.75,40.5,25.5,0xaabbcc77],['image',i*13,i*5,6,9,1,1,new Uint8Array([100,17,225,83])]]),[['linear-image',0,0,0,10,4,4,pixels],...all.slice(0,2)]];
   const reports=[];for(let index=0;index<cases.length;index++)for(const background of [0x193249ff,0x00000000]){
    const frame={background,commands:cases[index],state:'0'},a=old.draw(frame),b=fresh.draw(frame),c=await fresh.drawCooperatively(frame);let differences=0;for(let i=0;i<a.pixels.length;i++)if(a.pixels[i]!==b.pixels[i]||a.pixels[i]!==c.pixels[i])differences++;
    if(differences)throw Error('Pixel difference '+index+' background '+background+': '+differences);
    reports.push({case:index,background,oldCopies:a.copies,newCopies:b.copies,cooperativeCopies:c.copies});
   }
   if(reports[0].oldCopies!==235||reports[0].newCopies!==1||reports[0].cooperativeCopies!==1)throw Error('Expected one bounded run copy');return reports;
  })().then(done,error=>done({error:String(error)}));''')
  assert isinstance(result,list),result;print('Exact filtered-render parity for',len(result),'mixed/clip/transparent-background cases; 235 consecutive copies become one:',result)
 finally:browser.quit();server.shutdown();server.server_close()
