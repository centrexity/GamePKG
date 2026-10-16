"""Signed source draws actual RGBA sprite pixels through both runtime profiles."""
from gui_test_support import start_display
from pathlib import Path
from PIL import Image
import os, shutil, subprocess, tempfile, time, argparse
parser=argparse.ArgumentParser();parser.add_argument("--png",action="store_true");args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-sprites-') as directory:
 root=Path(directory);source=root/'source';shutil.copytree(project/'examples/minimal',source)
 (source/'source/src/lib.rs').write_text('''pub fn game_tick(input:u32,state:u64)->u64 {
     if !(gamepkg::draw_clear(0x10171dff)) { loop {} };
     let mut pixels=[255,0,0,255, 0,255,0,255, 0,0,255,255, 255,255,255,0];
     if input & gamepkg::HELD_RIGHT != 0 { pixels[1]=255; }
     if !(gamepkg::draw_image(64.0,64.0,512.0,352.0,2,2,&pixels)) { loop {} };
     state
 }''')
 if args.png:
  pixels=[(255,0,0,255),(0,255,0,255),(0,0,255,255),(255,255,255,0)]
  image=Image.new('RGBA',(2,2));image.putdata(pixels);image.save(source/'assets/sprite.png')
  pixels[0]=(255,255,0,255);image.putdata(pixels);image.save(source/'assets/yellow.png')
  (source/'assets/invalid.png').write_bytes(b'invalid PNG')
  (source/'assets/truncated.png').write_bytes((source/'assets/sprite.png').read_bytes()[:40])
  Image.new('RGBA',(513,1)).save(source/'assets/wide.png')
  (source/'assets/oversized.png').write_bytes(bytes(2*1024*1024+1))
  (source/'source/src/lib.rs').write_text('''pub fn game_tick(input:u32,state:u64)->u64 {
      if !(!gamepkg::draw_asset_image("../thumbnail.png",0.0,0.0,10.0,10.0)) { loop {} };
      if !(!gamepkg::draw_asset_image("missing.png",0.0,0.0,10.0,10.0)) { loop {} };
      if !(!gamepkg::draw_asset_image("invalid.png",0.0,0.0,10.0,10.0)) { loop {} };
      if !(!gamepkg::draw_asset_image("truncated.png",0.0,0.0,10.0,10.0)) { loop {} };
      if !(!gamepkg::draw_asset_image("wide.png",0.0,0.0,10.0,10.0)) { loop {} };
      if !(!gamepkg::draw_asset_image("oversized.png",0.0,0.0,10.0,10.0)) { loop {} };
      gamepkg::draw_clear(0x10171dff);
      let path=if input & gamepkg::HELD_RIGHT != 0 {"yellow.png"} else {"sprite.png"};
      if !(gamepkg::draw_asset_image(path,64.0,64.0,512.0,352.0)) { loop {} };
      state
  }''')
 original=(source/'manifest.toml').read_text()
 for profile in ['core-v1','component-v1']:
  config=root/(profile+'.toml');config.write_text('automatic_update_checks=false\n')
  env=dict(os.environ,GAMEPKG_HOME=str(root/profile),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
  cli=project/'target/release/gamepkg'
  def run(*args):
   if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
   result=subprocess.run([str(cli),*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
   assert result.returncode==0,result.stderr
  key=root/(profile+'.key');run('keygen','-o',key)
  (source/'manifest.toml').write_text(f'runtime_profile="{profile}"\n'+original)
  package=root/(profile+'.gamepkg');run('pack',source,'-o',package,'--key',key,*(['--shared-assets'] if args.png and profile=='component-v1' else []))
  if args.png and profile=='component-v1':
   import zipfile
   with zipfile.ZipFile(package) as archive:assert 'assets/sprite.png' not in archive.namelist()
   assert (root/profile/'asset-pool').exists()
  run('install',package,'--trust-creator');run('build','com.gamepkg.minimal');run('run','com.gamepkg.minimal','--frames','2')
  display=start_display(env)
  app=subprocess.Popen([str(project/'target/release/gamepkg-gui'),'--play','com.gamepkg.minimal'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  try:
   for _ in range(100):
    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
    found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
    if found.returncode==0:break
    time.sleep(.1)
   else:raise AssertionError('GUI did not open')
   window=found.stdout.splitlines()[0];path=project/'target'/f'sprites-{"png" if args.png else "rgba"}-{profile}.png'
   deadline=time.monotonic()+15
   while True:
    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
    subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
    with Image.open(path) as image:
     colors=image.convert('RGB').getcolors(image.width*image.height)
    counts={color:count for count,color in colors}
    if all(counts.get(color,0)>15000 for color in [(255,0,0),(0,255,0),(0,0,255)]):
     assert counts.get((255,255,255),0)<1000,'Transparent sprite pixels rendered opaque white'
     assert counts.get((16,23,29),0)>15000,'Transparent sprite quadrant did not preserve the background'
     break
    assert time.monotonic()<deadline,counts
    time.sleep(.1)
   subprocess.run(['xdotool','windowfocus',window,'keydown','Right'],env=env,check=True)
   deadline=time.monotonic()+10
   changed=project/'target'/f'sprites-changed-{"png" if args.png else "rgba"}-{profile}.png'
   while True:
    subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(changed)],env=env,check=True)
    with Image.open(changed) as image:
     counts={color:count for count,color in image.convert('RGB').getcolors(image.width*image.height)}
    if counts.get((255,255,0),0)>15000 and counts.get((255,0,0),0)<1000:break
    assert time.monotonic()<deadline,'Changed sprite pixels did not reach the texture'
    time.sleep(.1)
   subprocess.run(['xdotool','keyup','Right'],env=env,check=True)
   print(profile+(' PNG assets' if args.png else ' RGBA')+': signed source built and played; RGB quadrants, transparency, and changed texture pixels verified.',flush=True)
  finally:
   app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
