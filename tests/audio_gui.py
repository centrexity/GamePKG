import os, subprocess, tempfile, time, sys
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
  result=subprocess.run(['target/release/gamepkg',*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
  assert result.returncode==0,result.stderr
 (root/'null-alsa.conf').write_text('pcm.!default { type null }\n')
 env['ALSA_CONFIG_PATH']=str(root/'null-alsa.conf')
 key=root/'key';key.write_text(os.urandom(32).hex())
 run('pack','examples/good/tone-pad','-o',root/'game.gamepkg','--key',key)
 run('install',root/'game.gamepkg','--trust-creator');run('build','com.gamepkg.tone-pad')
 display=subprocess.Popen(['Xvfb',':97','-screen','0','1024x900x24','-ac'],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
 try:
  time.sleep(1)
  app=subprocess.Popen(['target/release/gamepkg-gui','--play','com.gamepkg.tone-pad'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  try:
   for _ in range(100):
    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
    state=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
    if state.returncode==0:break
    time.sleep(.1)
   window=state.stdout.strip().splitlines()[0]
   subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
   time.sleep(.5)
   before=project/('target/audio-before-muted.png' if muted else 'target/audio-before.png')
   subprocess.run(['import','-display',':97','-window',window,str(before)],env=env,check=True)
   def green_x(path):
    image=Image.open(path).convert('RGB')
    points=[x for y in range(image.height) for x in range(image.width) if image.getpixel((x,y))==(100,220,180)]
    assert points,'Guest green square was not rendered'
    return min(points)
   x=green_x(before)
   pixels=Image.open(before).convert('RGB')
   active=any(g>140 and r<100 and b<100 for y in range(min(60,pixels.height)) for x in range(min(120,pixels.width)) for r,g,b in [pixels.getpixel((x,y))])
   assert active != muted, 'Sound activity did not match the mute setting'
   subprocess.run(['xdotool','keydown','Down'],env=env,check=True);time.sleep(.5)
   after=project/('target/audio-player-muted.png' if muted else 'target/audio-player.png')
   subprocess.run(['import','-display',':97','-window',window,str(after)],env=env,check=True)
   subprocess.run(['xdotool','keyup','Down'],env=env,check=True)
   assert green_x(after)<x-20,'Held input did not select the lower pitch'
   print('GUI audio '+('muted by settings' if muted else 'active through a null output device')+'; held pitch selection verified from pixels.')
  finally:
   app.terminate();app.wait(timeout=5)
 finally:
  display.terminate();display.wait(timeout=5)
