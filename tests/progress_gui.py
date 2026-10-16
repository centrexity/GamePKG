"""Unlock a real Collect Stars achievement through desktop keyboard input."""
import argparse,json,os,subprocess,tempfile,time,shutil,select
from pathlib import Path
from gui_test_support import start_display,click_text
PROJECT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--indexeddb-package',type=Path);args=parser.parse_args()
with tempfile.TemporaryDirectory(prefix='gamepkg-progress-gui-') as directory:
 root=Path(directory);config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
 def run(*args):
  r=subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert r.returncode==0,r.stderr;return r.stdout
 key=root/'creator.key';run('keygen','-o',key);package=root/'collect-stars.gamepkg'
 run('pack',PROJECT/'examples/good/collect-stars','-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.gamepkg.collect-stars')
 display=start_display(env)
 app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui'),'--play','com.gamepkg.collect-stars'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 try:
  deadline=time.monotonic()+15
  while True:
   assert app.poll() is None,app.stderr.read().decode()
   found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
   if found.returncode==0:break
   assert time.monotonic()<deadline,'Player did not open';time.sleep(.1)
  window=found.stdout.splitlines()[0];subprocess.run(['xdotool','windowfocus',window],env=env,check=True);time.sleep(.5)
  for direction in ['Left','Up']:
   subprocess.run(['xdotool','key','--clearmodifiers','--repeat','6','--delay','180',direction],env=env,check=True)
  deadline=time.monotonic()+10
  while True:
   records=list((root/'library/progress').glob('*.json'))
   history=json.loads(records[0].read_text()) if records else {}
   if 'first_star' in history.get('unlocked',{}):break
   assert time.monotonic()<deadline,'Normal gameplay did not unlock First star';time.sleep(.1)
  assert 'ten_stars' not in history['unlocked'],history
  click_text(window,env,'Back to game',root)
  text=run('list');assert '[Unlocked] First star' in text and '[Locked] Star collector' in text,text
  history=json.loads(records[0].read_text());assert history['played_ms']>0,history
  print('Desktop keyboard play collected a star; achievement and measured play time persisted after returning to Library.')
  if args.indexeddb_package:
   from selenium import webdriver
   from selenium.webdriver.chrome.options import Options
   from selenium.webdriver.chrome.service import Service
   from selenium.webdriver.support.ui import WebDriverWait
   host=subprocess.Popen([str(PROJECT/'target/release/gamepkg'),'web-library','--indexeddb-package',str(args.indexeddb_package),'--port','0'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
   try:
    ready,_,_=select.select([host.stdout],[],[],10);assert ready,'Browser host did not start'
    line=host.stdout.readline();assert line.startswith('Open http://127.0.0.1:'),line
    url=line.strip().split(' ',1)[1];base,fragment=url.split('#');scope=fragment.split(':')[1]
    options=Options();options.binary_location=shutil.which('chromium');options.add_argument('--headless=new');options.add_argument('--no-sandbox');options.add_argument('--disable-dev-shm-usage');options.add_argument('--user-data-dir='+str(root/'browser-profile'))
    for cached in [False,True]:
     with webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options) as browser:
      browser.get(base+'#wrong:'+scope if cached else url)
      status='showing cached metadata' if cached else 'saved using Hindsight'
      WebDriverWait(browser,20).until(lambda b:status in b.find_element('id','status').text)
      browser.find_element('css selector','#games td:nth-child(6) details summary').click()
      rows=browser.find_element('id','games').text
      assert 'Unlocked — First star' in rows and 'Locked — Star collector' in rows,rows
      stored=browser.execute_async_script("const done=arguments[0];import('/storage.js').then(async ({HindsightLibraryStorage})=>{const storage=new HindsightLibraryStorage(location.hash.slice(1).split(':')[1]);await storage.open();const snapshot=await storage.loadLibrary();await storage.close();done(snapshot.games[0].history);}).catch(error=>done({error:String(error)}));")
      assert stored==history,stored
    print('Normal desktop gameplay history persisted through actual Hindsight and an unauthorized browser restart.')
   finally:
    host.terminate();host.wait(timeout=10)
 finally:
  app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
