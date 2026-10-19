from gui_test_support import start_display
import os, subprocess, tempfile, time
from pathlib import Path
from PIL import Image
from Xlib import display as xdisplay, protocol
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
   text=subprocess.check_output(['tesseract',str(after),'stdout'],stderr=subprocess.DEVNULL,text=True)
   for label in ['Close game','Restart','Score:','Sound on','Touch controls']:
    assert label not in text,(label,text)
   # The scene occupies the entire client area, retaining its 4:3 proportions
   # when the native window becomes wide. Letterboxing must stay black.
   subprocess.run(['xdotool','windowsize',window,'1000','600'],env=env,check=True)
   time.sleep(.3)
   wide=root/'wide.png'
   subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(wide)],env=env,check=True)
   image=Image.open(wide).convert('RGB')
   assert image.size==(1000,600),image.size
   assert image.getpixel((0,0))==(0,0,0)
   assert image.getpixel((999,599))==(0,0,0)
   green_x(wide)
   # Ask the app to close through the native window protocol, allowing save
   # and play-history cleanup instead of terminating its process.
   connection=xdisplay.Display(env['DISPLAY'])
   target=connection.create_resource_object('window',int(window))
   target.send_event(protocol.event.ClientMessage(window=target,
    client_type=connection.intern_atom('WM_PROTOCOLS'),
    data=(32,[connection.intern_atom('WM_DELETE_WINDOW'),0,0,0,0])),event_mask=0)
   connection.flush()
   assert app.wait(timeout=10)==0,app.stderr.read().decode()
   connection.close()
   import json
   records=list((root/'library/progress').glob('*.json'))
   assert records and json.loads(records[0].read_text())['played_ms']>0
   print('Game-only native window passed: scene, held input, resizing, native close and saved play time.')
  finally:
   if app.poll() is None:app.terminate();app.wait(timeout=5)
 finally:
  display.terminate();display.wait(timeout=5)
