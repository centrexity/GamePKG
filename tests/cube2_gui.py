"""Actual library data picker, separate Cube 2 window, input and native close."""
from pathlib import Path
import json,os,signal,subprocess,tempfile,time
from PIL import Image,ImageChops
from collections import Counter
from gui_test_support import start_display,click_text,game_window,choose_native_file
from Xlib import display as xdisplay,protocol
# OCR helpers also spawn subprocesses directly, so apply this to the test process.
os.environ['OMP_THREAD_LIMIT']='1'
PROJECT=Path(__file__).resolve().parents[1]
FIXTURE=PROJECT/'target/cube2-package-test'
LIBRARY=FIXTURE/'library'
assert (LIBRARY/'com.gamepkg.cube2.gamepkg').is_file(),'Run tests/cube2_package.py first'
with tempfile.TemporaryDirectory(prefix='gamepkg-cube2-gui-') as temporary:
 proof=PROJECT/'target/cube2-gui';proof.mkdir(exist_ok=True)
 root=Path(temporary);config=root/'settings.toml'
 config.write_text('automatic_update_checks=false\nmemory_ceiling_mib=2048\ntick_deadline_ms=500\nmute_audio=true\n')
 # Bound OCR's OpenMP workers so screenshot checks do not starve llvmpipe.
 env=dict(os.environ,OMP_THREAD_LIMIT='1',GAMEPKG_HOME=str(LIBRARY),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',GAMEPKG_RENDERER='vulkan',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
 env.pop('WAYLAND_DISPLAY',None)
 remembered=LIBRARY/'game-data-selections.json';old=remembered.read_bytes() if remembered.exists() else None
 if remembered.exists():remembered.unlink()
 server=start_display(env,1600,1050);app=None;quit_app=None;wm=None;connection=None;player_pid=None
 def command(*args):return subprocess.run(list(map(str,args)),env=env,check=True,capture_output=True,text=True,timeout=15)
 def close(window):
  global connection
  connection=xdisplay.Display(env['DISPLAY']);target=connection.create_resource_object('window',int(window))
  target.send_event(protocol.event.ClientMessage(window=target,client_type=connection.intern_atom('WM_PROTOCOLS'),data=(32,[connection.intern_atom('WM_DELETE_WINDOW'),0,0,0,0])),event_mask=0);connection.flush()
 try:
  app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
  library_window,_=game_window(env,'GamePKG')
  command('xdotool','windowsize',library_window,'1450','980','windowfocus',library_window)
  time.sleep(.5);click_text(library_window,env,'Cube',root)
  time.sleep(.3);click_text(library_window,env,'Choose game-data',root)
  dialog=choose_native_file(env,'Choose original Sauerbraten installation',PROJECT/'target/cube2-test-data')
  # GTK completes a directory path on the first Enter; then navigate/confirm.
  for attempt in range(3):
   time.sleep(.5)
   if remembered.exists():break
   candidates=subprocess.run(['xdotool','search','--onlyvisible','--name','^(Choose original Sauerbraten installation|Select a [Ff]ile|Select a [Ff]older|zenity)$'],env=env,capture_output=True,text=True).stdout.splitlines()
   def area(candidate):
    data=subprocess.run(['xdotool','getwindowgeometry','--shell',candidate],env=env,capture_output=True,text=True).stdout
    fields=dict(line.split('=',1) for line in data.splitlines() if '=' in line)
    return int(fields.get('WIDTH',0))*int(fields.get('HEIGHT',0))
   if not candidates:continue
   dialog=max(candidates,key=area)
   if area(dialog)<100000:continue
   command('xdotool','windowfocus',dialog,'key','--clearmodifiers','Return')
   time.sleep(.4)
   if remembered.exists():break
   candidates=subprocess.run(['xdotool','search','--onlyvisible','--name','^(Choose original Sauerbraten installation|Select a [Ff]ile|Select a [Ff]older|zenity)$'],env=env,capture_output=True,text=True).stdout.splitlines()
   if candidates:
    dialog=max(candidates,key=area)
    if area(dialog)>100000:
     command('import','-display',env['DISPLAY'],'-window',dialog,proof/'data-picker.png')
     click_text(dialog,env,'OK',root)
  deadline=time.monotonic()+5
  while not remembered.exists() and time.monotonic()<deadline:time.sleep(.1)
  if not remembered.exists():
   names=subprocess.run(['xdotool','search','--onlyvisible','--name','.','getwindowname','%@'],env=env,capture_output=True,text=True).stdout
   command('import','-display',env['DISPLAY'],'-window','root',proof/'picker-failure-root.png')
   raise AssertionError('Data folder was not remembered. Windows: '+names)
  # Exercise the actual server form without starting a connection. A private
  # address must be rejected until its separate local-network opt-in is checked.
  network=LIBRARY/'network-selections.json'
  click_text(library_window,env,'Save multiplayer settings',root);time.sleep(.3)
  assert network.exists() and not json.loads(network.read_text())
  command('xdotool','windowfocus',library_window)
  click_text(library_window,env,'server.example:28785',root)
  command('xdotool','key','--clearmodifiers','ctrl+a')
  command('xdotool','type','--clearmodifiers','127.0.0.1:28785')
  click_text(library_window,env,'Save multiplayer settings',root);time.sleep(.3)
  assert not json.loads(network.read_text()),'Local server was authorized without opting in'
  command('import','-display',env['DISPLAY'],'-window',library_window,proof/'multiplayer-denied.png')
  text=subprocess.check_output(['tesseract',str(proof/'multiplayer-denied.png'),'stdout'],stderr=subprocess.DEVNULL,text=True)
  assert 'enable local/private' in text.lower(),text
  click_text(library_window,env,'Allow local/private',root)
  click_text(library_window,env,'Save multiplayer settings',root);time.sleep(.3)
  choices=list(json.loads(network.read_text()).values())
  assert len(choices)==1 and choices[0]['server']=='127.0.0.1:28785' and choices[0]['allow_local']
  command('import','-display',env['DISPLAY'],'-window',library_window,proof/'multiplayer-settings.png')
  click_text(library_window,env,'Use offline play',root);time.sleep(.3)
  assert not json.loads(network.read_text()),'Offline play retained a network grant'
  click_text(library_window,env,'master.example:28787',root)
  command('xdotool','type','--clearmodifiers','master.example:28787')
  click_text(library_window,env,'Save multiplayer settings',root);time.sleep(.3)
  choices=list(json.loads(network.read_text()).values())
  assert len(choices)==1 and choices[0]['master']=='master.example:28787' and not choices[0]['server']
  click_text(library_window,env,'Use offline play',root);time.sleep(.3)
  assert not json.loads(network.read_text()),'Offline play retained a master discovery grant'

  command('xdotool','mousemove','--window',library_window,'1400','400','click','--repeat','15','--delay','20','4');time.sleep(.3)
  command('import','-display',env['DISPLAY'],'-window',library_window,proof/'ready.png')
  image=Image.open(proof/'ready.png').convert('RGB')
  rows=[]
  for y in range(image.height):
   xs=[x for x in range(image.width) if (lambda c:c[0]<100 and c[1]>170 and c[2]>140)(image.getpixel((x,y)))]
   if len(xs)>=80:rows.append((y,min(xs),max(xs)))
  assert rows,'No enabled primary Play button'
  group=[rows[0]]
  for row in rows[1:]:
   if row[0]>group[-1][0]+1:break
   group.append(row)
  command('xdotool','mousemove','--window',library_window,str((group[0][1]+group[0][2])//2),str((group[0][0]+group[-1][0])//2),'click','1')
  window,player_pid=game_window(env,'com.gamepkg.cube2',timeout=30)
  assert player_pid!=app.pid
  command('xdotool','windowfocus',window)
  proof=PROJECT/'target/cube2-gui';proof.mkdir(exist_ok=True)
  def capture(name):
   path=proof/name;command('import','-display',env['DISPLAY'],'-window',window,path);return Image.open(path).convert('RGB')
  records=list((LIBRARY/'progress').glob('*.json'))
  played_before=json.loads(records[0].read_text())['played_ms'] if records else 0
  saves_before=list((LIBRARY/'saves').glob('*.cube2'))
  saved_before=saves_before[0].stat().st_mtime_ns if saves_before else 0
  time.sleep(4);before=capture('playing.png')
  text=subprocess.run(['tesseract',str(proof/'playing.png'),'stdout'],capture_output=True,text=True,check=True).stdout
  for chrome in ['Close game','Restart','Touch controls','Sound on','Score:']:
   assert chrome not in text,(chrome,text)
  assert len(before.getcolors(before.width*before.height))>500,'No rendered 3D scene'
  command('xdotool','keydown','w');time.sleep(.5);command('xdotool','keyup','w');time.sleep(.2)
  moved=capture('moved.png')
  difference=ImageChops.difference(before,moved)
  assert sum(max(pixel)>35 for pixel in difference.get_flattened_data())>2000,'Keyboard movement did not change scene'
  def ammo(image,name):
   w,h=image.size
   crop=image.convert('L').crop((int(w*.48),int(h*.915),int(w*.54),int(h*.98))).resize((400,300))
   readings=[]
   for threshold in [140,150,160,170,180,200]:
    path=root/(name+'-ammo-'+str(threshold)+'.png');crop.point(lambda v:255 if v>threshold else 0).save(path)
    text=subprocess.check_output(['tesseract',str(path),'stdout','--psm','7','-c','tessedit_char_whitelist=0123456789'],stderr=subprocess.DEVNULL,text=True).strip()
    if text.isdigit():readings.append(int(text))
   votes=Counter(readings).most_common()
   ammo.last_readings=readings
   if votes and votes[0][1]>=3 and (len(votes)==1 or votes[0][1]>=2*votes[1][1]):return votes[0][0]
   return None
  before_ammo=ammo(moved,'before')
  deadline=time.monotonic()+3
  while before_ammo is None:
   assert time.monotonic()<deadline,('Could not read baseline ammunition',ammo.last_readings)
   time.sleep(.1);before_ammo=ammo(capture('before-fire.png'),'before')
  command('xdotool','click','1')
  deadline=time.monotonic()+3
  while True:
   time.sleep(.1);fired=capture('fired.png')
   if ammo(fired,'fired')==before_ammo-1:break
   assert time.monotonic()<deadline,('A short mouse click did not fire the weapon',before_ammo,ammo.last_readings)
  command('xdotool','key','t');time.sleep(.3)
  command('xdotool','type','--clearmodifiers','--delay','25','/name GamePKGDesktop')
  command('xdotool','key','Return');time.sleep(.3)
  command('xdotool','key','t');time.sleep(.2)
  command('xdotool','type','--clearmodifiers','--delay','25','/bind F12 [quit]')
  command('xdotool','key','Return');time.sleep(.3)
  command('xdotool','key','Escape');time.sleep(.3);capture('menu.png')
  command('import','-display',env['DISPLAY'],'-window','root',proof/'root-after-input.png')
  close(window)
  deadline=time.monotonic()+10
  while time.monotonic()<deadline:
   try:os.kill(player_pid,0)
   except ProcessLookupError:break
   time.sleep(.1)
  else:
   command('import','-display',env['DISPLAY'],'-window','root',proof/'close-failure.png')
   raise AssertionError('Game window did not close normally')
  saves=list((LIBRARY/'saves').glob('*.cube2'));assert saves and saves[0].read_bytes().startswith(b'GPS2') and saves[0].stat().st_mtime_ns>saved_before
  assert b'GamePKGDesktop' in saves[0].read_bytes(),'Desktop console text was not saved'
  records=list((LIBRARY/'progress').glob('*.json'));assert json.loads(records[0].read_text())['played_ms']>played_before+1000
  assert app.poll() is None,'Closing the game closed the library'
  saved_before=saves[0].stat().st_mtime_ns
  played_before=json.loads(records[0].read_text())['played_ms']
  # A private real window manager makes minimize/restore an actual IconicState
  # transition, rather than merely covering the window with another surface.
  wm_env=dict(env,GSETTINGS_BACKEND='memory',XDG_CONFIG_HOME=str(root/'wm-config'),XDG_CACHE_HOME=str(root/'wm-cache'),XDG_DATA_HOME=str(root/'wm-data'))
  with (root/'window-manager.log').open('wb') as log:
   wm=subprocess.Popen(['dbus-run-session','--','metacity','--sm-disable','--no-composite'],env=wm_env,stdout=log,stderr=log,start_new_session=True)
  deadline=time.monotonic()+10
  while '_NET_SUPPORTING_WM_CHECK' not in command('xprop','-root','_NET_SUPPORTING_WM_CHECK').stdout or 'window id' not in command('xprop','-root','_NET_SUPPORTING_WM_CHECK').stdout:
   assert wm.poll() is None,(root/'window-manager.log').read_text()
   assert time.monotonic()<deadline,'Private window manager did not start'
   time.sleep(.1)
  quit_app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui'),'--play','com.gamepkg.cube2'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
  window,player_pid=game_window(env,'com.gamepkg.cube2',timeout=30)
  command('xdotool','windowfocus',window)
  deadline=time.monotonic()+30
  while True:
   scene=capture('quit-ready.png')
   if len(scene.getcolors(scene.width*scene.height))>500:break
   assert time.monotonic()<deadline,'The second player did not render its scene'
   time.sleep(.1)
  command('xdotool','windowminimize',window)
  deadline=time.monotonic()+3
  while 'Iconic' not in command('xprop','-id',window,'WM_STATE').stdout:
   assert time.monotonic()<deadline,'Game did not enter minimized state';time.sleep(.1)
  deadline=time.monotonic()+15
  # The checkpoint timer starts when history opens, just before its worker.
  # Require substantial recorded background time without assuming those two
  # instants coincide to the millisecond.
  while json.loads(records[0].read_text())['played_ms']<played_before+5000:
   assert quit_app.poll() is None,'Minimized player exited'
   assert time.monotonic()<deadline,'Minimized player did not checkpoint play time'
   time.sleep(.1)
  command('xdotool','windowmap',window,'windowactivate','--sync',window)
  deadline=time.monotonic()+3
  while 'Normal' not in command('xprop','-id',window,'WM_STATE').stdout:
   assert time.monotonic()<deadline,'Game did not restore';time.sleep(.1)
  time.sleep(.3);capture('restored.png')
  command('xdotool','key','F12')
  assert quit_app.wait(timeout=15)==0,quit_app.stderr.read().decode()
  assert saves[0].stat().st_mtime_ns>saved_before,'In-game Quit did not save preferences'
  assert json.loads(records[0].read_text())['played_ms']>played_before+1000,'In-game Quit did not save time'
  assert app.poll() is None,'In-game Quit closed the library'
  player_pid=None
  print('Cube 2 desktop passed: real folder picker, server-choice form/local opt-in/offline clearing, separate Vulkan game-only player, movement/fire, console text, native close, real minimize/restore with background play-time checkpoint, in-game Quit and saved preferences.')
 finally:
  if app and app.poll() is None:
   subprocess.run(['import','-display',env['DISPLAY'],'-window','root',str(proof/'final-root.png')],env=env,capture_output=True)
   subprocess.run(['import','-display',env['DISPLAY'],'-window',library_window,str(proof/'library-last.png')],env=env,capture_output=True)
  if connection:connection.close()
  if player_pid:
   try:os.kill(player_pid,15)
   except ProcessLookupError:pass
  if quit_app and quit_app.poll() is None:quit_app.terminate();quit_app.wait(timeout=10)
  if app and app.poll() is None:app.terminate();app.wait(timeout=10)
  if wm and wm.poll() is None:
   os.killpg(wm.pid,signal.SIGTERM);wm.wait(timeout=10)
  if old is not None and not remembered.exists():remembered.write_bytes(old)
  server.terminate();server.wait(timeout=10)
