from gui_test_support import start_display,wait_game_frame
import os, subprocess, tempfile, time, sys, threading, select
from pathlib import Path
from PIL import Image
project=Path(__file__).resolve().parents[1]
os.chdir(project)
with tempfile.TemporaryDirectory(prefix='gamepkg-draw-ui-') as directory:
 root=Path(directory)
 muted='--muted' in sys.argv
 if muted: (root/'settings.toml').write_text('mute_audio = true\n')
 env=dict(os.environ,DISPLAY=':97',LIBGL_ALWAYS_SOFTWARE='1',GAMEPKG_CONFIG=str(root/'settings.toml'),GAMEPKG_HOME=str(root/'library'),GAMEPKG_SLANG_ROOT=os.environ.get('GAMEPKG_SLANG_ROOT',''));env.pop('WAYLAND_DISPLAY',None)
 def run(*args):
  if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
  result=subprocess.run(['target/release/gamepkg',*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
  assert result.returncode==0,result.stderr
 # Capture real device bytes through a bounded FIFO instead of a removed HUD.
 fifo=root/'audio.pcm';os.mkfifo(fifo)
 stop=threading.Event();audible=threading.Event()
 def capture_audio():
  fd=os.open(fifo,os.O_RDONLY|os.O_NONBLOCK)
  try:
   while not stop.is_set():
    ready,_,_=select.select([fd],[],[],.1)
    if not ready:continue
    data=os.read(fd,8192)
    if not data:time.sleep(.01);continue
    if len(set(data))>4:audible.set()
  finally:os.close(fd)
 capture=threading.Thread(target=capture_audio,daemon=True);capture.start()
 (root/'null-alsa.conf').write_text('pcm.null { type null }\npcm.!default { type file slave.pcm "null" file "'+str(fifo)+'" format "raw" }\n')
 env['ALSA_CONFIG_PATH']=str(root/'null-alsa.conf')
 key=root/'key';key.write_text(os.urandom(32).hex())
 run('pack','examples/good/tone-pad','-o',root/'game.gamepkg','--key',key)
 run('install',root/'game.gamepkg','--trust-creator');run('build','com.gamepkg.tone-pad')
 display=start_display(env)
 try:
  app=subprocess.Popen(['target/release/gamepkg-gui','--play','com.gamepkg.tone-pad'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  try:
   for _ in range(100):
    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
    state=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
    if state.returncode==0:break
    time.sleep(.1)
   window=state.stdout.strip().splitlines()[0]
   subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
   wait_game_frame(window,env,root)
   before=project/('target/audio-before-muted.png' if muted else 'target/audio-before.png')
   subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(before)],env=env,check=True)
   def green_x(path):
    image=Image.open(path).convert('RGB')
    points=[x for y in range(image.height) for x in range(image.width) if image.getpixel((x,y))==(100,220,180)]
    assert points,'Guest green square was not rendered'
    return min(points)
   x=green_x(before)
   if not muted:assert audible.wait(timeout=10),'No non-silent samples reached the configured audio device'
   else:assert not audible.is_set(),'Muted playback emitted non-silent device samples'
   subprocess.run(['xdotool','keydown','Down'],env=env,check=True);time.sleep(.5)
   after=project/('target/audio-player-muted.png' if muted else 'target/audio-player.png')
   subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(after)],env=env,check=True)
   subprocess.run(['xdotool','keyup','Down'],env=env,check=True)
   assert green_x(after)<x-20,'Held input did not select the lower pitch'
   print('GUI audio '+('muted by settings' if muted else 'active through a null output device')+'; held pitch selection verified from pixels.')
  finally:
   app.terminate();app.wait(timeout=5)
 finally:
  display.terminate();display.wait(timeout=5)
  stop.set();capture.join(timeout=2);assert not capture.is_alive(),'Audio capture did not stop'
