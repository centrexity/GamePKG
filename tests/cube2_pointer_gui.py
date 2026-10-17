"""Real X mouse buttons/wheel -> desktop bridge -> original Cube bindings."""
from pathlib import Path
import json,os,re,shutil,struct,subprocess,tempfile,time
from PIL import Image
from Xlib import display as xdisplay,protocol
from gui_test_support import start_display,game_window,cube2_choose_test_map
PROJECT=Path(__file__).resolve().parents[1]
FIXTURE=PROJECT/'target/cube2-package-test/library'
with tempfile.TemporaryDirectory(prefix='cube2-pointer-ui-') as temporary:
 root=Path(temporary);library=root/'library';shutil.copytree(FIXTURE,library)
 config=root/'settings.toml';config.write_text('memory_ceiling_mib=2048\ntick_deadline_ms=500\nautomatic_update_checks=false\nmute_audio=true\n')
 env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',GAMEPKG_RENDERER='vulkan',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
 env.pop('WAYLAND_DISPLAY',None)
 proof=PROJECT/'target/cube2-pointer-gui';proof.mkdir(exist_ok=True)
 server=start_display(env);app=None;connection=None
 def command(*args):return subprocess.run(list(map(str,args)),env=env,check=True,capture_output=True,text=True,timeout=10)
 def console(text):
  command('xdotool','key','t');time.sleep(.15)
  command('xdotool','type','--clearmodifiers','--delay','15','/'+text)
  command('xdotool','key','Return');time.sleep(.15)
 try:
  app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui'),'--play','com.gamepkg.cube2'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
  window,_=game_window(env,'com.gamepkg.cube2',timeout=30)
  command('xdotool','windowfocus',window)
  deadline=time.monotonic()+30
  while True:
   command('import','-display',env['DISPLAY'],'-window',window,proof/'ready.png')
   image=Image.open(proof/'ready.png').convert('RGB')
   if len(image.getcolors(image.width*image.height))>500:break
   assert app.poll() is None and time.monotonic()<deadline,'No playable pointer-test scene'
   time.sleep(.1)
  cube2_choose_test_map(window,env)
  console('name Input')
  # The labels come from the original keymap: the physical middle button is
  # MOUSE3, the two side buttons MOUSE6/7, and wheels MOUSE4/5/8/9.
  for name,tag in [('MOUSE3','M'),('MOUSE6','A'),('MOUSE7','B'),('MOUSE4','U'),('MOUSE5','D'),('MOUSE8','R'),('MOUSE9','L')]:
   console(f'bind {name} [name (concatword (getclientname (getclientnum)) {tag})]')
  # X wheel-left (6) is SDL x=-1, hence MOUSE9; wheel-right is MOUSE8.
  for button in [2,8,9,4,5,6,7]:
   # XInput's synthetic wheel button releases also become wheel events in
   # winit. Send only the press for one notch; real wheel axes have no release.
   command('xdotool','mousedown' if button in [4,5,6,7] else 'click',str(button));time.sleep(.15)
  time.sleep(.2);command('import','-display',env['DISPLAY'],'-window',window,proof/'after.png')
  connection=xdisplay.Display(env['DISPLAY']);target=connection.create_resource_object('window',int(window))
  target.send_event(protocol.event.ClientMessage(window=target,client_type=connection.intern_atom('WM_PROTOCOLS'),data=(32,[connection.intern_atom('WM_DELETE_WINDOW'),0,0,0,0])),event_mask=0);connection.flush()
  assert app.wait(timeout=10)==0,app.stderr.read().decode()
  saves=list((library/'saves').glob('*.cube2'));assert len(saves)==1
  data=saves[0].read_bytes();assert data[:4]==b'GPS2';count,size,reserved=struct.unpack_from('<3I',data,4);assert size==len(data)-16 and reserved==0
  offset=16;config=None
  for _ in range(count):
   n,size,a,b=struct.unpack_from('<4I',data,offset);assert a==b==0;offset+=16
   name=data[offset:offset+n].decode();offset+=n;body=data[offset:offset+size];offset+=size
   if name=='config.cfg':config=body
  assert config is not None
  actual=re.search(rb'^name "([^"]*)"',config,re.M).group(1)
  assert actual==b'InputMABUDLR',('Native pointer event sequence',actual)
  print('Cube 2 native pointer passed: physical middle/side buttons, both vertical and horizontal wheel directions, original bindings and saved results.')
 finally:
  if app and app.poll() is None:
   subprocess.run(['import','-display',env['DISPLAY'],'-window','root',str(proof/'failure.png')],env=env,capture_output=True)
   app.terminate();app.wait(timeout=10)
  if connection:connection.close()
  server.terminate();server.wait(timeout=10)
