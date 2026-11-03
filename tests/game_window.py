"""Direct player startup and normal operating-system window close save progress."""
from pathlib import Path
import json, os, subprocess, tempfile, time
from gui_test_support import start_display, game_window
from Xlib import display as xdisplay, protocol
PROJECT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-player-window-') as temporary:
 root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config));env.pop('WAYLAND_DISPLAY',None)
 def run(*args):
  result=subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,text=True,capture_output=True,timeout=60)
  assert result.returncode==0,result.stderr
 key=root/'key';key.write_text('64'*32);package=root/'test.gamepkg'
 run('pack',PROJECT/'examples/minimal','-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.gamepkg.minimal')
 server=start_display(env);app=None
 try:
  # Steam must retain both the selected library and settings, even if Steam's
  # own inherited environment points somewhere else. Keep all paths private.
  wrong=root/'invalid-inherited-settings.toml';wrong.write_text('not valid TOML\n')
  launch_env=dict(env,GAMEPKG_HOME=str(root/'wrong-library'),GAMEPKG_CONFIG=str(wrong))
  app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui'),'--library',str(root/'library'),'--play','com.gamepkg.minimal','--target','native','--config',str(config)],env=launch_env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
  window,pid=game_window(env,'com.gamepkg.minimal');assert pid==app.pid
  library=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True)
  assert library.returncode!=0,'Player opened library UI'
  time.sleep(1.5)
  from PIL import Image
  for width,height in [(960,720),(1200,500),(400,700)]:
   subprocess.run(['xdotool','windowsize',window,str(width),str(height)],env=env,check=True)
   time.sleep(.5)
   capture=root/'player.png'
   subprocess.run(['import','-window',window,str(capture)],env=env,check=True)
   image=Image.open(capture).convert('RGB')
   text=subprocess.run(['tesseract',str(capture),'stdout'],capture_output=True,text=True,check=True).stdout
   for label in ['Close game','Restart','Score:','Sound on','Touch controls','Arrows']:
    assert label not in text,(label,text)
   # The square game board stays centered with black letterboxing.
   w,h=image.size
   assert all(channel<10 for channel in image.getpixel((2,2))),image.getpixel((2,2))
   assert image.getpixel((w//2,h//2)) != (0,0,0)

  connection=xdisplay.Display(env['DISPLAY']);target=connection.create_resource_object('window',int(window))
  target.send_event(protocol.event.ClientMessage(window=target,client_type=connection.intern_atom('WM_PROTOCOLS'),data=(32,[connection.intern_atom('WM_DELETE_WINDOW'),0,0,0,0])),event_mask=0)
  connection.flush();assert app.wait(timeout=15)==0,app.stderr.read().decode();connection.close()
  records=list((root/'library/progress').glob('*.json'));assert len(records)==1
  assert json.loads(records[0].read_text())['played_ms']>=1000
  assert wrong.read_text()=='not valid TOML\n'
  assert not (root/'wrong-library').exists()
  print('Separate player passed: Steam-style explicit library/settings override inherited paths, game title/process, no library UI, normal close and saved play time.')
 finally:
  if app and app.poll() is None:app.terminate();app.wait(timeout=10)
  server.terminate();server.wait(timeout=10)
