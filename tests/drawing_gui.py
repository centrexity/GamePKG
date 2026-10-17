from gui_test_support import start_display
import os, subprocess, tempfile, time
from pathlib import Path
from PIL import Image
project=Path(__file__).resolve().parents[1]
os.chdir(project)
with tempfile.TemporaryDirectory(prefix='gamepkg-draw-ui-') as directory:
 root=Path(directory)
 env=dict(os.environ,DISPLAY=':97',LIBGL_ALWAYS_SOFTWARE='1',GAMEPKG_CONFIG=str(root/'settings.toml'),GAMEPKG_HOME=str(root/'library'),GAMEPKG_SLANG_ROOT=os.environ.get('GAMEPKG_SLANG_ROOT',''));env.pop('WAYLAND_DISPLAY',None)
 def run(*args):
  if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','native')
  result=subprocess.run(['target/release/gamepkg',*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
  assert result.returncode==0,result.stderr
 key=root/'key';key.write_text(os.urandom(32).hex())
 run('pack','examples/good/shape-playground','-o',root/'game.gamepkg','--key',key)
 run('install',root/'game.gamepkg','--trust-creator');run('build','com.gamepkg.shape-playground')
 display=start_display(env)
 try:
  app=subprocess.Popen(['target/release/gamepkg-gui','--play','com.gamepkg.shape-playground'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  try:
   for _ in range(100):
    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
    state=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
    if state.returncode==0:break
    time.sleep(.1)
   window=state.stdout.strip().splitlines()[0]
   subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
   time.sleep(.5)
   before=root/'before.png'
   subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(before)],env=env,check=True)
   def green_x(path):
    image=Image.open(path).convert('RGB')
    points=[x for y in range(image.height) for x in range(image.width) if image.getpixel((x,y))==(100,220,180)]
    assert points,'Guest green square was not rendered'
    return min(points)
   x=green_x(before)
   subprocess.run(['xdotool','keydown','Left'],env=env,check=True);time.sleep(.5)
   subprocess.run(['xdotool','keyup','Left'],env=env,check=True);time.sleep(.1)
   after=project/'target/drawing-player.png'
   subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(after)],env=env,check=True)
   assert green_x(after)<x-20,'Held input did not move the guest-drawn square'
   print('GUI guest scene rendered; held movement verified from actual screen pixels.')
  finally:
   app.terminate();app.wait(timeout=5)
 finally:
  display.terminate();display.wait(timeout=5)
