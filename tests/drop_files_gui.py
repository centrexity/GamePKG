"""Deliver real Xdnd file-manager drops to the native app on a private display."""
from pathlib import Path
import os,subprocess,tempfile,time,tomllib,select
from Xlib import X,display,protocol
from gui_test_support import start_display,click_text
PROJECT=Path(__file__).resolve().parents[1]
BINS=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))
def send_drop(env,window,path):
 d=display.Display(env['DISPLAY']);root=d.screen().root;source=root.create_window(0,0,1,1,0,X.CopyFromParent)
 atom=lambda n:d.intern_atom(n)
 selection=atom('XdndSelection');uri=atom('text/uri-list');target=d.create_resource_object('window',int(window));source.set_selection_owner(selection,X.CurrentTime)
 def message(name,data):target.send_event(protocol.event.ClientMessage(window=target,client_type=atom(name),data=(32,data)),event_mask=0);d.flush()
 message('XdndEnter',[source.id,5<<24,uri,0,0]);message('XdndPosition',[source.id,0,(200<<16)|200,X.CurrentTime,atom('XdndActionCopy')])
 dropped=False;deadline=time.monotonic()+10
 while time.monotonic()<deadline:
  if not d.pending_events():select.select([d.fileno()],[],[],.1);continue
  event=d.next_event()
  if event.type==X.ClientMessage and event.client_type==atom('XdndStatus') and not dropped:
   assert event.data[1][1]&1,'App rejected the native drag'
   message('XdndDrop',[source.id,0,X.CurrentTime,0,0]);dropped=True
  elif event.type==X.SelectionRequest:
   requestor=d.create_resource_object('window',event.requestor.id);prop=event.property or event.target
   requestor.change_property(prop,event.target,8,(path.resolve().as_uri()+'\r\n').encode())
   requestor.send_event(protocol.event.SelectionNotify(time=event.time,requestor=event.requestor,selection=event.selection,target=event.target,property=prop),event_mask=0);d.flush()
  elif event.type==X.ClientMessage and event.client_type==atom('XdndFinished'):
   source.destroy();d.close();return
 source.destroy();d.close();raise AssertionError('Native drop did not finish')
with tempfile.TemporaryDirectory(prefix='gamepkg-drop-gui-') as temp:
 root=Path(temp);config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
 key=root/'key';key.write_text('63'*32);package=root/'sample with spaces.gamepkg'
 subprocess.run([str(BINS/'gamepkg'),'pack',str(PROJECT/'examples/minimal'),'-o',str(package),'--key',str(key)],env=env,check=True,capture_output=True)
 server=start_display(env,1500,1200);app=None
 try:
  app=subprocess.Popen([str(BINS/'gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  deadline=time.monotonic()+20
  while time.monotonic()<deadline:
   if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
   found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
   if found.returncode==0:break
   time.sleep(.1)
  assert found.returncode==0;window=found.stdout.splitlines()[0]
  subprocess.run(['xdotool','windowsize',window,'1400','1100','windowfocus',window],env=env,check=True)
  send_drop(env,window,package)
  click_text(window,env,'Cancel package review',root,timeout=20)
  assert not list((root/'library').glob('*.gamepkg')),'Drop installed without approval'
  send_drop(env,window,PROJECT/'examples/lists/starter.gamepkglist')
  deadline=time.monotonic()+10
  while not (root/'library/catalog.gamepkglist').exists() and time.monotonic()<deadline:time.sleep(.1)
  games=tomllib.loads((root/'library/catalog.gamepkglist').read_text())['games'];assert len(games)==2
  assert not list((root/'library').glob('*.gamepkg')),'List downloaded/installed games'
  send_drop(env,window,package)
  click_text(window,env,'Trust this creator and install',root,timeout=20)
  deadline=time.monotonic()+10
  while not (root/'library/com.gamepkg.minimal.gamepkg').exists() and time.monotonic()<deadline:time.sleep(.1)
  assert (root/'library/com.gamepkg.minimal.gamepkg').exists()
  print('Native drag-and-drop passed: escaped-space package path, explicit review/cancel/install, and list import without game downloads.')
 finally:
  if app:
   app.terminate()
   try:app.wait(timeout=10)
   except subprocess.TimeoutExpired:app.kill();app.wait()
  server.terminate();server.wait(timeout=10)
