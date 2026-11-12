"""Maintainer-only complete source-resolution frames; no original assets are published."""
from pathlib import Path
import argparse,hashlib,json,math,os,shutil,subprocess,sys,tempfile,time
from PIL import Image,ImageChops
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('original_assets',type=Path);parser.add_argument('--desktop',action='store_true');parser.add_argument('--browser',action='store_true');parser.add_argument('--output',type=Path);args=parser.parse_args()
project=Path(__file__).resolve().parents[1];sys.path.insert(0,str(project/'tests'))
from gui_test_support import start_display,game_window
# Pinned c0m4r/arkanoo revision 9d613e78c9ed5657ae7fda9206cd4a82a893b7c8.
HASHES={'background1.png': '21c34d3db19f1d2f8560bada107af03762a4003dfbc2c9a3c9b80b0bc9b3f8b2', 'background2.png': 'f5b14219e8b265d6bb2e669b1050768cd2a57688a8691a142569e458a4ba35e9', 'background3.png': '5634e4eb36fc4ef61120064f003f0fe69fdaf23302e0bfaf64ad1fed84aeb56d', 'background4.png': '053b1263b2647d128a73d6f0236781a873790fb473af95348fbba1b1b1793eeb', 'background5.png': 'cf7808cee9e154e3ab8b064867557717806476ecbeefa2fd4c16eab0860cee68', 'background6.png': 'a45a1912f5e7d3b6251a0794612a1cd6cd42507b1d38a27bddd24250daf9dcf3'}
proof=args.output or project/'target/original-image-frame-proof';proof.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='gamepkg-original-image-frames-') as temporary:
 root=Path(temporary);library=root/'library';config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(config),GAMEPKG_ORIGINAL_IMAGE_LIBRARY=str(library),LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',SDL_AUDIODRIVER='dummy');env.pop('WAYLAND_DISPLAY',None)
 env['GAMEPKG_BUNDLED_AUDIO_DECODER']=str(project/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm');env['GAMEPKG_BUNDLED_AUDIO_CONVERTER']=str(project/'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm')
 key=root/'key';key.write_text('84'*32);cli=project/'target/release/gamepkg';gui=project/'target/release/gamepkg-gui'
 def run(*parts):
  result=subprocess.run([str(cli),*map(str,parts)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stdout+'\n'+result.stderr
 assets=root/'tiles';assets.mkdir();expected=[]
 for number in range(1,7):
  name=f'background{number}.png';original=args.original_assets/name;assert hashlib.sha256(original.read_bytes()).hexdigest()==HASHES[name]
  with Image.open(original) as png:image=png.convert('RGBA')
  assert image.size==(2816,1536) and image.getchannel('A').getextrema()==(255,255);hashes=[]
  for index in range(24):
   x=index%6*510;y=index//6*510;w=min(510,2816-x);h=min(510,1536-y);left=max(0,x-1);top=max(0,y-1);right=min(2816,x+w+1);bottom=min(1536,y+h+1)
   tile=image.crop((left,top,right,bottom));hashes.append(hashlib.sha256(tile.tobytes()).hexdigest());tile.convert('RGB').save(assets/f'background{number}-{index}.png',optimize=True)
  expected.append(hashes)
 paths='\n'.join(str(i)+' => ['+','.join('"background'+str(i+1)+'-'+str(j)+'.png"' for j in range(24))+'],' for i in range(6))
 source='''pub fn game_tick(input:u32,state:u64)->u64 {
 let previous=((state>>32) as u32)%6;let selected=if input&gamepkg::RIGHT!=0 {(previous+1)%6} else {previous};
 let paths=match selected {PATHS _ => ["background1-0.png";24],};
 gamepkg::draw_clear(0x102030ff);let sx=640.0/2816.0;let sy=360.0/1536.0;
 for index in 0..24 {let cx=index%6*510usize;let cy=index/6*510usize;let cw=(2816-cx).min(510);let ch=(1536-cy).min(510);let left=cx.saturating_sub(1);let top=cy.saturating_sub(1);let right=(cx+cw+1).min(2816);let bottom=(cy+ch+1).min(1536);
 let x=cx as f32*sx;let y=60.0+cy as f32*sy;let x1=((cx+cw) as f32*sx).min(640.0);let y1=60.0+(cy+ch) as f32*sy;
 if !gamepkg::draw_clip(x,y,x1-x,y1-y){return 0;}
 if !gamepkg::draw_asset_image_linear(paths[index],left as f32*sx,60.0+top as f32*sy,(right-left) as f32*sx,(bottom-top) as f32*sy){return 0;}
 if !gamepkg::draw_clip_reset(){return 0;}}
 let colors=[0xff0000ff,0x00ff00ff,0x0000ffff,0xffff00ff,0xff00ffff,0x00ffffff];gamepkg::draw_rect(0.0,0.0,10.0,10.0,colors[selected as usize]);
 (u64::from(selected)<<32)|24
}'''.replace('PATHS',paths)
 ids={}
 for profile in ['core-v1','component-v1']:
  recipe=root/profile;shutil.copytree(project/'examples/minimal',recipe);shutil.copytree(assets,recipe/'assets',dirs_exist_ok=True);(recipe/'source/src/lib.rs').write_text(source)
  id='com.gamepkg.full-images-'+profile;ids[profile]=id;manifest=recipe/'manifest.toml';text=manifest.read_text().replace('com.gamepkg.minimal',id).replace('[limits]','[limits]\nrender_profile="large-images-v1"').replace('recommended_memory_bytes = 8388608','recommended_memory_bytes = 268435456').replace('hard_memory_bytes = 16777216','hard_memory_bytes = 268435456');manifest.write_text('runtime_profile="'+profile+'"\nengine_version=">=0.1.18, <0.2.0"\n'+text)
  archive=root/(profile+'.gamepkg');run('pack',recipe,'--key',key,'-o',archive);run('install',archive,'--trust-creator');run('build',id,'--target','wasm')
  if profile=='core-v1':run('build',id,'--target','native')
 (library/'expected-images.json').write_text(json.dumps(expected))
 compiled=subprocess.run(['cargo','test','--release','--all-features','--test','original_image_frames','--no-run','--message-format=json'],env=env,cwd=project,capture_output=True,text=True,timeout=180);assert compiled.returncode==0,compiled.stderr
 executable=next(value['executable'] for line in compiled.stdout.splitlines() if line.startswith('{') and (value:=json.loads(line)).get('target',{}).get('name')=='original_image_frames' and value.get('executable'))
 subprocess.run([executable,'--ignored','--nocapture'],env=env,check=True,timeout=90)
 report={'scope':'Private complete original-background framework probe; not activated port artwork','profiles':[],'pixel_hashes':expected,'render_profile':'large-images-v1'}
 display=None
 try:
  if args.desktop or args.browser:
   display=start_display(env,3000,1800,local_only=True);reference=root/'sdl-reference';subprocess.run(['cc','-Wall','-Wextra','-Werror',str(project/'tests/reference/sdl_full_image.c'),'-o',str(reference),*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_image'],text=True).split()],check=True)
  references={}
  def compare(image,scene,label):
   width,height=image.size;key=(scene,width,height)
   if key not in references:
    raw=subprocess.check_output([str(reference),str(args.original_assets/f'background{scene+1}.png'),str(width),str(height)],env=env,timeout=30);assert len(raw)==width*height*4;references[key]=Image.frombytes('RGBA',(width,height),raw).convert('RGB')
   difference=ImageChops.difference(image.convert('RGB'),references[key]);maximum=max(v[1] for v in difference.getextrema());image.save(proof/(label+'.png'));references[key].save(proof/(label+'-original.png'));report['profiles'].append({'label':label,'scene':scene,'size':[width,height],'max_channel_delta':maximum});(proof/'results.json').write_text(json.dumps(report,indent=2));assert maximum<=2,(label,maximum)
  if args.desktop:
   for profile in ['core-v1','component-v1']:
    for target in (['native','wasm'] if profile=='core-v1' else ['wasm']):
     for renderer in ['vulkan','opengl']:
      app=subprocess.Popen([str(gui),'--play',ids[profile],'--target',target,'--renderer',renderer],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
      try:
       window,_=game_window(env,ids[profile])
       for width,height in [(1280,960),(854,640)]:
        subprocess.run(['xdotool','windowsize',window,str(width),str(height),'windowfocus',window],env=env,check=True);time.sleep(.3)
        for scene in range(6):
         if scene:subprocess.run(['xdotool','key','--window',window,'Right'],env=env,check=True)
         marker=[(255,0,0),(0,255,0),(0,0,255),(255,255,0),(255,0,255),(0,255,255)][scene];deadline=time.monotonic()+20
         while True:
          assert app.poll() is None,app.stderr.read().decode() if app.poll() is not None else ''
          path=proof/'current.png';subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
          with Image.open(path) as frame:frame=frame.convert('RGB')
          scale=min(frame.width/640,frame.height/480);ox=(frame.width-640*scale)/2;oy=(frame.height-480*scale)/2
          if frame.getpixel((int(ox+5*scale),int(oy+5*scale)))==marker:break
          assert time.monotonic()<deadline,(profile,target,renderer,scene);time.sleep(.05)
         crop=frame.crop((round(ox),round(oy+60*scale),round(ox+640*scale),round(oy+420*scale)))
         compare(crop,scene,f'{profile}-{target}-{renderer}-{width}x{height}-{scene}')
        # Return to first scene before the next resolution.
        subprocess.run(['xdotool','key','--window',window,'Right'],env=env,check=True);time.sleep(.1)
      except Exception:
       windows=subprocess.run(['xdotool','search','--onlyvisible','--name','.*'],env=env,capture_output=True,text=True)
       names=[]
       for wid in windows.stdout.splitlines()[:12]:names.append(subprocess.run(['xdotool','getwindowname',wid],env=env,capture_output=True,text=True).stdout.strip())
       print('Private scene window failure:',profile,target,renderer,'exit',app.poll(),'titles',names,flush=True)
       subprocess.run(['import','-display',env['DISPLAY'],'-window','root',str(proof/'desktop-failure.png')],env=env,check=False)
       raise
      finally:
       if app.poll() is None:app.terminate()
       _,stderr=app.communicate(timeout=15)
       if stderr:(proof/f'{profile}-{target}-{renderer}-stderr.txt').write_bytes(stderr[-8192:])
  if args.browser:
   from functools import partial
   from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
   import threading
   from selenium import webdriver
   from selenium.webdriver.chrome.options import Options
   from selenium.webdriver.chrome.service import Service
   from selenium.webdriver.support.ui import WebDriverWait
   from selenium.webdriver.common.keys import Keys
   from io import BytesIO
   import base64
   class Quiet(SimpleHTTPRequestHandler):
    def log_message(self,*parts):pass
   for profile in ['core-v1','component-v1']:run('web-export',ids[profile],'-o',root/(profile+'-web'))
   server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
   options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('chromium')
   for flag in ['--headless=new','--mute-audio','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader']:options.add_argument(flag)
   driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver');browser=webdriver.Chrome(service=Service(driver),options=options)
   try:
    for profile in ['core-v1','component-v1']:
     browser.get(f'http://127.0.0.1:{server.server_port}/{profile}-web/');wait=WebDriverWait(browser,30);wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click();canvas=browser.find_element('id','game')
     for scene in range(6):
      if scene:canvas.send_keys(Keys.ARROW_RIGHT)
      expected_state=str((scene<<32)|24);wait.until(lambda b:canvas.get_attribute('data-state')==expected_state)
      png=browser.execute_script('return document.querySelector("#game").toDataURL("image/png").split(",")[1]')
      with Image.open(BytesIO(base64.b64decode(png))) as frame:crop=frame.convert('RGB').crop((0,60,640,420))
      compare(crop,scene,f'{profile}-browser-{scene}')
     browser.find_element('id','stop').click()
   finally:browser.quit();server.shutdown();server.server_close()
 finally:
  if display:display.terminate();display.wait(timeout=10)
 (proof/'results.json').write_text(json.dumps(report,indent=2))
 print('Complete source-resolution background hashes and frames verified; resource defaults and comparison tolerance unchanged.',flush=True)
