"""Saved URL lists: restart, targeted/global refresh, partial failure and removal confirmation."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,tempfile,time
from gui_test_support import start_display,game_window,click_text
PROJECT=Path(__file__).resolve().parents[1]
os.environ.setdefault('OMP_THREAD_LIMIT','1')
with tempfile.TemporaryDirectory(prefix='gamepkg-saved-lists-') as temporary:
 root=Path(temporary);home=root/'library';tools=root/'tools';tools.mkdir()
 first='https://lists.example.invalid/first.gamepkglist';second='https://lists.example.invalid/second.gamepkglist'
 (tools/'curl').write_text('''#!/usr/bin/env python3
import json,os,pathlib,sys
args=sys.argv[1:];url=args[args.index('--url')+1];root=pathlib.Path(os.environ['LIST_FIXTURE'])
with (root/'requests.jsonl').open('a') as log:log.write(json.dumps(url)+'\\n')
responses=json.loads((root/'responses.json').read_text())
if url not in responses:sys.exit(23)
pathlib.Path(args[args.index('--output')+1]).write_text(responses[url]);print('200',end='')
''');(tools/'curl').chmod(0o755)
 env=dict(os.environ,LIST_FIXTURE=str(root),PATH=str(tools)+os.pathsep+os.environ['PATH'],GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(root/'settings.toml'));env.pop('WAYLAND_DISPLAY',None)
 (root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n');(root/'key').write_text('49'*32)
 def cli(*args):
  result=subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=90);assert result.returncode==0,result.stderr;return result.stdout
 def catalog(names,version='1.0.0'):
  return 'format_version=1\n'+''.join(f'[[games]]\nid="com.example.{name}"\ntitle="Remote {name}"\nversion="{version}"\ndownload_url="https://games.invalid/{name}.gamepkg"\n' for name in names)
 responses={first:catalog(['one']),second:catalog(['two'])}
 def publish(): (root/'responses.json').write_text(json.dumps(responses))
 publish();cli('pack',PROJECT/'examples/minimal','-o',root/'game.gamepkg','--key',root/'key');cli('install',root/'game.gamepkg','--trust-creator')
 installed=home/'com.gamepkg.minimal.gamepkg';original=hashlib.sha256(installed.read_bytes()).hexdigest()
 cli('import-list',first)
 state=home/'catalog-sources.json'
 def saved():return json.loads(state.read_text())['lists']
 def await_state(predicate):
  end=time.monotonic()+30
  while not predicate(saved()):
   assert time.monotonic()<end,saved();time.sleep(.1)
  time.sleep(.5)
 def requests():return [json.loads(line) for line in (root/'requests.jsonl').read_text().splitlines()]
 server=start_display(env,1600,1400);app=None
 def launch():
  global app,window
  app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
  window,_=game_window(env,'GamePKG');subprocess.run(['xdotool','windowsize',window,'1450','1150','windowfocus',window],env=env,check=True);time.sleep(.4)
 def click(text):click_text(window,env,text,root,timeout=25)
 try:
  launch();click('Add')
  _,hint_y=click_text(window,env,'Or drop a',root,click=False)
  subprocess.run(['xdotool','mousemove','--window',window,'100',str(hint_y+40),'click','1','type','--clearmodifiers',second],env=env,check=True)
  click('Import .gamepkglist');await_state(lambda lists:len(lists)==2);click('Saved game lists')
  responses[first]=catalog(['one','new'],'2.0.0');publish();before=requests().count(second)
  click('Update list');await_state(lambda lists:len(lists[0]['games'])==2)
  assert requests().count(second)==before,'Individual update fetched another list'
  assert saved()[0]['games'][0]['version']=='2.0.0'
  app.terminate();app.wait(timeout=10);launch();click('Add');click('Saved game lists');assert len(saved())==2
  responses[first]=catalog(['one','new','fresh'],'3.0.0');responses[second]=catalog(['two','other'],'2.0.0');publish()
  click('Back to library');click('Refresh library');await_state(lambda lists:len(lists[0]['games'])==3 and len(lists[1]['games'])==2)
  retained=saved()[0];responses[first]='invalid list';responses[second]=catalog(['two','other','third'],'3.0.0');publish()
  click('Refresh library');await_state(lambda lists:len(lists[1]['games'])==3);assert saved()[0]==retained,'Failed list lost last good snapshot'
  click('Add');click('Remove list');click('Cancel');assert len(saved())==2,'Cancel removed a list'
  click('Remove list');click('Confirm removal');await_state(lambda lists:len(lists)==1)
  assert saved()[0]['url']==second
  listing=cli('list');assert 'com.example.one' not in listing and 'com.example.third' in listing
  assert hashlib.sha256(installed.read_bytes()).hexdigest()==original,'Removing a list changed an installed game'
  assert list(home.glob('*.gamepkg'))==[installed],'List refresh downloaded or installed games'
  assert set(requests())=={first,second},requests()
  proof=PROJECT/'target/saved-lists.png';subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(proof)],env=env,check=True)
  print('Saved lists passed: URL persistence, individual/all refresh, changed/new metadata, failed-list retention, confirmed/cancelled removal, installed-game preservation and zero package downloads.')
 finally:
  if app and app.poll() is None:app.terminate();app.wait(timeout=10)
  server.terminate();server.wait(timeout=10)
