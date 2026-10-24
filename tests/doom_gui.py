"""Private native DOOM window, physical gamma, library return and relaunch."""
from pathlib import Path
import argparse,os,shutil,signal,struct,subprocess,tempfile,time
from PIL import Image,ImageChops
from gui_test_support import start_display,game_window,click_text,choose_native_file,click_primary_play
def pixels(im):return im.get_flattened_data() if hasattr(im,'get_flattened_data') else im.getdata()
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('wad',type=Path);p.add_argument('--library',type=Path,required=True);args=p.parse_args()
proof=PROJECT/'target/doom-native-proof';proof.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-gui-',dir=PROJECT/'target') as temp:
 root=Path(temp);shutil.copytree(args.library,root/'library',ignore=shutil.ignore_patterns('saves','progress'))
 config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);env['DBUS_SESSION_BUS_ADDRESS']='unix:path=/tmp/gamepkg-no-session-bus'
 display=start_display(env,1800,1100);app=None
 try:
  binary=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg-gui'
  app=subprocess.Popen([str(binary)],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  deadline=time.monotonic()+20
  while time.monotonic()<deadline:
   if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
   found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,text=True,capture_output=True,timeout=2)
   if found.returncode==0:break
   time.sleep(.1)
  assert found.returncode==0;window=found.stdout.splitlines()[0];library_window=window
  subprocess.run(['xdotool','windowsize',window,'1700','1000','windowfocus',window],env=env,check=True)
  click_text(window,env,'DOOM',root,timeout=20);time.sleep(.3)
  def capture(name):
   path=proof/name;subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);return path
  click_text(window,env,'Choose ROM',root,timeout=20)
  choose_native_file(env,'Choose Your DOOM.WAD or DOOM2.WAD (original PC IWAD)',args.wad.resolve())
  time.sleep(.3);click_primary_play(window,env,root)
  window,pid=game_window(env,'com.gamepkg.doom');assert pid != app.pid
  subprocess.run(['xdotool','windowsize',window,'1700','1000','windowfocus',window],env=env,check=True)
  subprocess.run(['xdotool','windowsize',window,'1000','800','windowfocus',window],env=env,check=True)
  time.sleep(5)
  def key(value):subprocess.run(['xdotool','key','--window',window,value],env=env,check=True)
  # Menus: open, new game, episode, difficulty.
  for _ in range(4):key('Return');time.sleep(.6)
  time.sleep(3);before=capture('gameplay-before.png')
  screen=Image.open(before).convert('RGB');hud=screen.crop((0,screen.height-120,screen.width-50,screen.height))
  # Read the actual HEALTH label rather than asking OCR to segment every HUD
  # number and its decorative border in one text block.
  w,h=screen.size
  health=screen.convert('L').crop((int(w*.15),int(h*.91),int(w*.34),int(h*.97))).resize((760,192))
  health.point(lambda v:255 if v>140 else 0).save(root/'health.png')
  text=subprocess.check_output(['tesseract',str(root/'health.png'),'stdout','--psm','7'],stderr=subprocess.DEVNULL,text=True).upper()
  assert 'HEALTH' in text and sum(r>150 and r>g*2 and r>b*2 for r,g,b in pixels(hud))>1000,text
  subprocess.run(['xdotool','keydown','--window',window,'Up'],env=env,check=True);time.sleep(.6)
  subprocess.run(['xdotool','keyup','--window',window,'Up'],env=env,check=True);time.sleep(.2)
  after=capture('gameplay-moved.png')
  diff=ImageChops.difference(Image.open(before).convert('RGB'),Image.open(after).convert('RGB'))
  assert sum(max(pixel)>30 for pixel in pixels(diff))>1000,'No visible movement'
  key('Control_L');key('Tab');time.sleep(.4);capture('automap.png');key('Tab')
  key('F2');time.sleep(.3);key('Return');time.sleep(.3)
  subprocess.run(['xdotool','type','--delay','100','--window',window,'GUI SAVE'],env=env,check=True)
  time.sleep(.6);key('Return')
  deadline=time.monotonic()+15
  while time.monotonic()<deadline:
   blobs=list((root/'library/saves').glob('*.blob'))
   if len(blobs)==1 and b'GUI SAVE\0' in blobs[0].read_bytes():break
   time.sleep(.1)
  assert len(blobs)==1 and 1000<blobs[0].stat().st_size<=4*1024*1024,'No scoped DOOM save'
  capture('quicksaved.png')
  saved=blobs[0].read_bytes()
  assert b'GUI SAVE\0' in saved,f'Physical keyboard save-name entry failed: {saved[44:68]!r}'
  def gamma():
   data=blobs[0].read_bytes()
   assert data[:8]==b'\x89GPD2\r\n\x1a' and len(data)>=44
   lengths=struct.unpack('<8I',data[12:44]);assert 44+sum(lengths)==len(data)
   offset=44+sum(lengths[:6]);main=data[offset:offset+lengths[6]]
   for line in main.splitlines():
    fields=line.split(None,1)
    if len(fields)==2 and fields[0]==b'usegamma':return int(fields[1])
   raise AssertionError('Gamma is missing from the actual saved configuration')
  def wait_gamma(expected):
   deadline=time.monotonic()+10
   while time.monotonic()<deadline:
    if gamma()==expected:return
    time.sleep(.05)
   raise AssertionError(f'Expected saved gamma {expected}, got {gamma()}')
  def geometry():
   text=subprocess.check_output(['xdotool','getwindowgeometry','--shell',window],env=env,text=True)
   values=dict(line.split('=',1) for line in text.splitlines() if '=' in line)
   return int(values['WIDTH']),int(values['HEIGHT'])
  def quit_game(process):
   key('F10');time.sleep(.2);key('y')
   deadline=time.monotonic()+12
   while time.monotonic()<deadline:
    visible=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(process)],env=env,capture_output=True,text=True,timeout=2)
    if visible.returncode!=0:return
    # The original ENDOOM screen waits for a final key after its quit sound.
    subprocess.run(['xdotool','key','--window',window,'Return'],env=env,capture_output=True,timeout=2)
    time.sleep(.2)
   raise AssertionError('Original quit/ENDOOM flow did not close the game window')
  wait_gamma(0);size=geometry();key('F11');wait_gamma(1)
  assert geometry()==size,'F11 gamma changed the desktop window size'
  capture('gamma-one.png');print('Physical F11 changed saved gamma without resizing the game window.',flush=True)
  previous_pid=pid;quit_game(pid)
  subprocess.run(['xdotool','windowfocus',library_window],env=env,check=True)
  click_primary_play(library_window,env,root)
  window,pid=game_window(env,'com.gamepkg.doom');assert pid not in (app.pid,previous_pid)
  subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
  time.sleep(1);size=geometry();key('F11');wait_gamma(2)
  assert geometry()==size,'F11 after relaunch changed the desktop window size'
  capture('gamma-relaunched.png');quit_game(pid)
  # Only terminate the game process created on this private display.
  subprocess.run(['xdotool','windowfocus',library_window],env=env,check=True)
  click_primary_play(library_window,env,root)
  window,pid=game_window(env,'com.gamepkg.doom');assert pid != app.pid
  os.kill(pid,signal.SIGKILL)
  assert app.poll() is None,'The library exited with the failed game'
  click_text(library_window,env,'stopped unexpectedly',root,timeout=15,max_y=900,click=False)
  click_primary_play(library_window,env,root)
  window,recovered_pid=game_window(env,'com.gamepkg.doom');assert recovered_pid not in (app.pid,pid)
  subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
  time.sleep(1);wait_gamma(2);quit_game(recovered_pid)
  print('Native DOOM passed: WAD selection, visible gameplay/HUD, movement/map key exercises, named scoped save, actual F11 gamma without resizing and remembered gamma across a separate game-process relaunch, visible unexpected-exit reporting and recovery.')
 except BaseException:
  if 'library_window' in locals():
   subprocess.run(['import','-display',env['DISPLAY'],'-window',library_window,str(proof/'shortcut-failed-library.png')],env=env,capture_output=True,timeout=10)
  raise
 finally:
  if app is not None:
   app.terminate()
   try:app.wait(timeout=10)
   except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
  display.terminate();display.wait(timeout=10)
