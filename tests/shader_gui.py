from gui_test_support import start_display
import os, subprocess, tempfile, time, argparse, shutil
from pathlib import Path
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('--component',action='store_true');parser.add_argument('--backend',choices=['glsl','spirv']);args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
os.chdir(project)
with tempfile.TemporaryDirectory(prefix='gamepkg-shader-ui-') as directory:
 root=Path(directory)
 env=dict(os.environ,DISPLAY=':97',LIBGL_ALWAYS_SOFTWARE='1',GAMEPKG_CONFIG=str(root/'settings.toml'),GAMEPKG_HOME=str(root/'library'),GAMEPKG_SLANG_ROOT=os.environ.get('GAMEPKG_SLANG_ROOT',''));env.pop('WAYLAND_DISPLAY',None)
 # No backend setting by default: first-launch defaults must support desktop playback.
 if args.backend: (root/'settings.toml').write_text(f'shader_backend = "{args.backend}"\n')
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
 display=start_display(env)
 try:
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
   subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(before)],env=env,check=True)
   def dominant(path, channel):
    image=Image.open(path).convert('RGB')
    points=sum(1 for rgb in (image.getpixel((x,y)) for y in range(image.height) for x in range(image.width)) if rgb[channel]>150 and rgb[channel]>2*rgb[1 if channel==2 else 2])
    return points>20000
   deadline=time.monotonic()+15
   while not dominant(before,2):
    assert time.monotonic()<deadline, 'Package blue shader pixels did not appear'
    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
    time.sleep(.1)
    subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(before)],env=env,check=True)
   subprocess.run(['xdotool','keydown','Right'],env=env,check=True);time.sleep(.5)
   after=project/'target/shader-player.png'
   subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(after)],env=env,check=True)
   assert dominant(after,1), 'Package green shader pixels missing'
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

 if args.backend == 'spirv':
  binary=metadata.with_name('0.shader');original_binary=binary.read_bytes()
  binary.write_bytes(b'not SPIR-V')
  result=subprocess.run(['target/release/gamepkg','run','com.gamepkg.shader-switch','--frames','1'],env=env,capture_output=True,text=True)
  assert result.returncode!=0 and 'SPIR-V' in result.stderr, result.stderr
  binary.write_bytes(original_binary)
  print('Malformed cached SPIR-V rejected before playback.')
