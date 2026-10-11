import os, subprocess, tempfile, time, argparse, shutil
from pathlib import Path
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('--component',action='store_true');args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
os.chdir(project)
with tempfile.TemporaryDirectory(prefix='gamepkg-shader-ui-') as directory:
 root=Path(directory)
 env=dict(os.environ,DISPLAY=':97',LIBGL_ALWAYS_SOFTWARE='1',GAMEPKG_CONFIG=str(root/'settings.toml'),GAMEPKG_HOME=str(root/'library'),GAMEPKG_SLANG_ROOT=os.environ.get('GAMEPKG_SLANG_ROOT',''));env.pop('WAYLAND_DISPLAY',None)
 (root/'settings.toml').write_text('shader_backend = "glsl"\n')
 def run(*args):
  result=subprocess.run(['target/release/gamepkg',*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
  assert result.returncode==0,result.stderr
 key=root/'key';key.write_text(os.urandom(32).hex())
 source=project/'examples/good/shader-switch'
 if args.component:
  source=root/'source';shutil.copytree(project/'examples/good/shader-switch',source)
  manifest=source/'manifest.toml';manifest.write_text('runtime_profile = "component-v1"\n'+manifest.read_text())
 run('pack',source,'-o',root/'game.gamepkg','--key',key)
 run('install',root/'game.gamepkg','--trust-creator');run('build','com.gamepkg.shader-switch')
 display=subprocess.Popen(['Xvfb',':97','-screen','0','1024x900x24','-ac'],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
 try:
  time.sleep(1)
  app=subprocess.Popen(['target/release/gamepkg-gui','--play','com.gamepkg.shader-switch'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  try:
   for _ in range(100):
    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
    state=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
    if state.returncode==0:break
    time.sleep(.1)
   window=state.stdout.strip().splitlines()[0]
   subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
   time.sleep(.5)
   before=root/'before.png'
   subprocess.run(['import','-display',':97','-window',window,str(before)],env=env,check=True)
   def dominant(path, channel):
    image=Image.open(path).convert('RGB')
    points=sum(1 for rgb in (image.getpixel((x,y)) for y in range(image.height) for x in range(image.width)) if rgb[channel]>150 and rgb[channel]>2*rgb[1 if channel==2 else 2])
    assert points>20000, f'Package shader pixels missing: {points}'
   dominant(before,2)
   subprocess.run(['xdotool','keydown','Right'],env=env,check=True);time.sleep(.5)
   after=project/'target/shader-player.png'
   subprocess.run(['import','-display',':97','-window',window,str(after)],env=env,check=True)
   dominant(after,1)
   subprocess.run(['xdotool','keyup','Right'],env=env,check=True)
   print('Compiled package fragment passes rendered; held input switched actual blue/green screen pixels.')
  finally:
   app.terminate();app.wait(timeout=5)
 finally:
  display.terminate();display.wait(timeout=5)

 # Resource declarations and missing reflection must fail before graphics setup.
 import json
 metadata=next((root/'library/shader-builds').glob('*/0.reflection.json'))
 original=metadata.read_bytes()
 for reflection in [{'parameters':[{'name':'untrustedTexture'}]}, {}]:
  metadata.write_text(json.dumps(reflection))
  result=subprocess.run(['target/release/gamepkg','run','com.gamepkg.shader-switch','--frames','1'],env=env,capture_output=True,text=True)
  assert result.returncode!=0 and ('resource' in result.stderr or 'reflection' in result.stderr), result.stderr
 metadata.write_bytes(original)
 print('External resources and missing shader reflection rejected before playback.')
