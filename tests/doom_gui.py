"""Select a private IWAD and verify real native DOOM gameplay and saving."""
from pathlib import Path
import argparse,csv,io,os,shutil,subprocess,tempfile,time
from PIL import Image,ImageChops
from gui_test_support import start_display,click_text
def pixels(im):return im.get_flattened_data() if hasattr(im,'get_flattened_data') else im.getdata()
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('wad',type=Path);p.add_argument('--library',type=Path,required=True);args=p.parse_args()
proof=PROJECT/'target/doom-native-proof';proof.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-gui-',dir=PROJECT/'target') as temp:
 root=Path(temp);shutil.copytree(args.library,root/'library',ignore=shutil.ignore_patterns('saves','progress'))
 config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
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
  assert found.returncode==0;window=found.stdout.splitlines()[0]
  subprocess.run(['xdotool','windowsize',window,'1700','1000','windowfocus',window],env=env,check=True)
  click_text(window,env,'com.gamepkg.doom',root,timeout=20);time.sleep(.3)
  def capture(name):
   path=proof/name;subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);return path
  screen=capture('rom-selection.png');large=root/'ocr.png';Image.open(screen).resize((5100,3000)).save(large)
  tsv=subprocess.check_output(['tesseract',str(large),'stdout','--psm','11','tsv'],stderr=subprocess.DEVNULL,text=True)
  words=[w for w in csv.DictReader(io.StringIO(tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if w['text'].strip()]
  your=min([w for w in words if w['text']=='Your'],key=lambda w:int(w['top']))
  y=(int(your['top'])+int(your['height'])//2)//3
  subprocess.run(['xdotool','mousemove','--window',window,'350',str(y),'click','1','type','--clearmodifiers',str(args.wad.resolve())],env=env,check=True)
  time.sleep(.3);click_text(window,env,'Play',root,timeout=20)
  subprocess.run(['xdotool','windowsize',window,'1000','800','windowfocus',window],env=env,check=True)
  time.sleep(5)
  def key(value):subprocess.run(['xdotool','key','--window',window,value],env=env,check=True)
  # Menus: open, new game, episode, difficulty.
  for _ in range(4):key('Return');time.sleep(.6)
  time.sleep(3);before=capture('gameplay-before.png')
  screen=Image.open(before).convert('RGB');hud=screen.crop((0,screen.height-120,screen.width-50,screen.height))
  hud.resize((hud.width*2,hud.height*2)).save(root/'hud.png')
  text=subprocess.check_output(['tesseract',str(root/'hud.png'),'stdout','--psm','6'],stderr=subprocess.DEVNULL,text=True).upper()
  assert 'HEALTH' in text and sum(r>150 and r>g*2 and r>b*2 for r,g,b in pixels(hud))>1000,text
  subprocess.run(['xdotool','keydown','--window',window,'Up'],env=env,check=True);time.sleep(.6)
  subprocess.run(['xdotool','keyup','--window',window,'Up'],env=env,check=True);time.sleep(.2)
  after=capture('gameplay-moved.png')
  diff=ImageChops.difference(Image.open(before).convert('RGB'),Image.open(after).convert('RGB'))
  assert sum(max(pixel)>30 for pixel in pixels(diff))>1000,'No visible movement'
  key('z');key('v');time.sleep(.4);capture('automap.png');key('v')
  subprocess.run(['xdotool','keydown','--window',window,'q','keydown','--window',window,'e'],env=env,check=True)
  key('Return');time.sleep(.2)
  subprocess.run(['xdotool','keyup','--window',window,'q','keyup','--window',window,'e'],env=env,check=True)
  deadline=time.monotonic()+5
  while time.monotonic()<deadline:
   blobs=list((root/'library/saves').glob('*.blob'))
   if blobs:break
   time.sleep(.1)
  assert len(blobs)==1 and 1000<blobs[0].stat().st_size<=512*1024,'No scoped DOOM quicksave'
  capture('quicksaved.png')
  print('Native DOOM passed: WAD selection, visible level/HUD, keyboard movement, fire/map controls and scoped quicksave.')
 finally:
  if app is not None:
   app.terminate()
   try:app.wait(timeout=10)
   except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
  display.terminate();display.wait(timeout=10)
