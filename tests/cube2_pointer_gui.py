"""Real X mouse buttons/wheel -> desktop bridge -> original Cube bindings."""
from pathlib import Path
import json,os,re,shutil,struct,subprocess,tempfile,time
from PIL import Image
from Xlib import X,XK,display as xdisplay,protocol
from Xlib.ext import xtest
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
  console('name I')
  # The labels come from the original keymap: the physical middle button is
  # MOUSE3, side buttons MOUSE6/7, extra buttons MOUSE10/11/12,
  # and wheels MOUSE4/5/8/9.
  for name,tag in [('MOUSE1','P'),('MOUSE2','S'),('MOUSE3','M'),('MOUSE6','A'),('MOUSE7','B'),('MOUSE10','E'),('MOUSE11','F'),('MOUSE12','G'),('MOUSE4','U'),('MOUSE5','D'),('MOUSE8','R'),('MOUSE9','L')]:
   console(f'bind {name} [name (concatword (getclientname (getclientnum)) {tag})]')
  # Z is a physical original binding, not the portable B/fire action.
  console('bind Z [name (concatword (getclientname (getclientnum)) Z)]')
  command('xdotool','key','z');time.sleep(.2)
  for button in [1,3,2,8,9]:
   command('xdotool','click',str(button));time.sleep(.15)
  connection=xdisplay.Display(env['DISPLAY'])
  mapping=list(connection.get_pointer_mapping());assert len(mapping)>=10
  try:
   for logical in [10,11,12]:
    remapped=mapping.copy();remapped[9]=logical
    assert connection.set_pointer_mapping(remapped)==X.MappingSuccess
    connection.sync()
    xtest.fake_input(connection,X.ButtonPress,10);xtest.fake_input(connection,X.ButtonRelease,10)
    connection.sync();time.sleep(.15)
  finally:
   assert connection.set_pointer_mapping(mapping)==X.MappingSuccess
   connection.sync()
  # X wheel-left (6) is SDL x=-1, hence MOUSE9; wheel-right is MOUSE8.
  # Synthetic wheel releases also become wheel events; send one press/notch.
  for button in [4,5,6,7]:
   command('xdotool','mousedown',str(button));time.sleep(.15)
  console('defsvarp pointermatrix (getclientname (getclientnum)) ""')
  console('name Order')
  # Send one burst so physical modifier/key/button order is preserved even
  # when the whole sequence lands between two paints. Inspect original script
  # state during the middle click (right=1,left=0), then after release (0,0).
  console('rstate = 0; lstate = 0')
  console('bind RCTRL [rstate = 1; onrelease [rstate = 0]]')
  console('bind LCTRL [lstate = 1; onrelease [lstate = 0]]')
  console('bind MOUSE3 [name (concatword (getclientname (getclientnum)) $rstate $lstate)]')
  console('bind MOUSE4 [name (concatword (getclientname (getclientnum)) $rstate $lstate)]')
  right=connection.keysym_to_keycode(XK.string_to_keysym('Control_R'));assert right
  for kind,detail in [(X.KeyPress,right),(X.ButtonPress,2),(X.ButtonRelease,2),(X.KeyRelease,right),(X.ButtonPress,4)]:
   xtest.fake_input(connection,kind,detail)
  connection.sync()
  time.sleep(.4);command('import','-display',env['DISPLAY'],'-window',window,proof/'after.png')
  target=connection.create_resource_object('window',int(window))
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
  assert actual==b'Order1000',('Native pointer event sequence',actual)
  matrix=re.search(rb'^pointermatrix \"([^\"]*)\"',config,re.M)
  assert matrix and matrix.group(1)==b'IZPSMABEFGUDLR',('Original eight-button matrix',matrix.group(1) if matrix else config)
  print('Cube 2 native pointer passed: all eight original physical buttons, original Z without a synthetic fire action, both wheel axes, rapid right-Ctrl/click/release/wheel order, original bindings and saved results.')
 finally:
  if app and app.poll() is None:
   subprocess.run(['import','-display',env['DISPLAY'],'-window','root',str(proof/'failure.png')],env=env,capture_output=True)
   app.terminate();app.wait(timeout=10)
  if connection:connection.close()
  server.terminate();server.wait(timeout=10)
