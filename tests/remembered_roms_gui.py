"""Remember distinct game ROMs through switching, app restart and --play."""
from pathlib import Path
import csv,io,os,shutil,subprocess,tempfile,time,json
from PIL import Image
from gui_test_support import start_display,game_window,click_text,choose_native_file,close_game,wait_game_frame,click_primary_play
os.environ.setdefault('OMP_THREAD_LIMIT','1')
PROJECT=Path(__file__).resolve().parents[1];BINS=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))
with tempfile.TemporaryDirectory(prefix='gamepkg-remembered-roms-') as temp:
 root=Path(temp);home=root/'library';config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);env['DBUS_SESSION_BUS_ADDRESS']='unix:path=/tmp/gamepkg-no-session-bus'
 key=root/'key';key.write_text('65'*32)
 def run(*args):
  r=subprocess.run([str(BINS/'gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr
 for name in ['alpha','beta']:
  source=root/name;shutil.copytree(PROJECT/'examples/minimal',source)
  mp=source/'manifest.toml';mp.write_text(mp.read_text().replace('com.gamepkg.minimal','com.example.'+name)+f'\n[[roms]]\nid="main"\ntitle="{name.title()} ROM"\n')
  (source/'source/src/lib.rs').write_text('pub fn game_tick(_:u32,_:u64)->u64 { let mut bytes=[0u8;1];gamepkg::rom_read_at(0,0,&mut bytes);let color=if bytes[0]==65 {0xff0000ff} else if bytes[0]==66 {0x00ff00ff} else {0x0000ffff};gamepkg::draw_clear(color);u64::from(bytes[0])<<32 }')
  package=root/(name+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.example.'+name)
 a=root/'alpha.rom';b=root/'beta.rom';a.write_bytes(b'A');b.write_bytes(b'B')
 server=start_display(env,1800,1200);app=None;window=None
 def launch(*args):
  global app,window
  app=subprocess.Popen([str(BINS/'gamepkg-gui'),*args],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  end=time.monotonic()+20
  while time.monotonic()<end:
   if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
   found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
   if found.returncode==0:break
   time.sleep(.1)
  assert found.returncode==0;window=found.stdout.splitlines()[0]
  subprocess.run(['xdotool','windowsize',window,'1700','1100','windowfocus',window],env=env,check=True);time.sleep(.3)
 def close():
  app.terminate();app.wait(timeout=10)
 def words():
  path=root/'screen.png';subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
  Image.open(path).resize((3400,2200)).save(root/'ocr.png')
  tsv=subprocess.check_output(['tesseract',str(root/'ocr.png'),'stdout','--psm','11','tsv'],stderr=subprocess.DEVNULL,text=True)
  return [dict(w,left=int(w['left'])//2,top=int(w['top'])//2,width=int(w['width'])//2,height=int(w['height'])//2) for w in csv.DictReader(io.StringIO(tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if w['text'].strip()]
 def click_word(w):subprocess.run(['xdotool','mousemove','--window',window,str(w['left']+w['width']//2),str(w['top']+w['height']//2),'click','1'],env=env,check=True);time.sleep(.2)
 def click_rom_field(name):
  ws=words();labels=[w for w in ws if w['text'].lower()==name and w['left']<200]
  assert labels,ws
  label=min(labels,key=lambda w:w['top'])
  fields=[w for w in ws if label['top']+label['height']<w['top']<label['top']+110 and w['left']<650]
  assert fields,ws
  click_word(min(fields,key=lambda w:w['top']))
 def expect_rom(byte):
  wait_game_frame(window,env,root,(255,0,0) if byte==65 else (0,255,0))

 try:
  launch()
  for name,path in [('alpha',a),('beta',b)]:
   click_text(window,env,'com.example.'+name,root);time.sleep(.2)
   click_rom_field(name)
   choose_native_file(env,'Choose '+name.title()+' ROM',path)
   deadline=time.monotonic()+10
   while True:
    stored=json.loads((home/'rom-selections.json').read_text()) if (home/'rom-selections.json').exists() else {}
    if any(str(path) in paths for paths in stored.values()):break
    assert time.monotonic()<deadline,'Selected ROM was not remembered'
    time.sleep(.1)
   if name=='alpha':
    before=(home/'rom-selections.json').read_bytes()
    click_rom_field(name)
    choose_native_file(env,'Choose Alpha ROM')
    assert (home/'rom-selections.json').read_bytes()==before,'Cancel changed remembered ROMs'
   time.sleep(.3);click_text(window,env,'Back to library',root)
  stored=json.loads((home/'rom-selections.json').read_text());assert sorted(stored.values())==sorted([[str(a)],[str(b)]])
  close();launch()
  click_text(window,env,'com.example.alpha',root);click_primary_play(window,env,root);library_window=window;window,pid=game_window(env,'com.example.alpha');assert pid != app.pid;expect_rom(65);close_game(window,env);window=library_window;click_text(window,env,'Back to library',root)
  click_text(window,env,'com.example.beta',root);click_primary_play(window,env,root);library_window=window;window,pid=game_window(env,'com.example.beta');assert pid != app.pid;expect_rom(66);close_game(window,env);window=library_window;click_text(window,env,'Back to library',root)
  proof=PROJECT/'target/remembered-roms-library.png';subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(proof)],env=env,check=True)
  close();launch('--play','com.example.alpha');expect_rom(65)
  print('Remembered ROMs passed: separate file paths, game switching, restart and direct --play without reselecting files.')
 finally:
  if app and app.poll() is None:close()
  server.terminate();server.wait(timeout=10)
