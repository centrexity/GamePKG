"""Catalog preview descriptions and signed package metadata have distinct authority."""
from pathlib import Path
import os, shutil, subprocess, tempfile, time
from gui_test_support import start_display, click_text
PROJECT=Path(__file__).resolve().parents[1];proof=PROJECT/'target/metadata-proof';proof.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='gamepkg-metadata-') as temporary:
 root=Path(temporary);home=root/'library';config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(config));env.pop('WAYLAND_DISPLAY',None)
 def run(*args):
  result=subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,text=True,capture_output=True,timeout=60)
  assert result.returncode==0,result.stderr
  return result.stdout
 source=root/'game';shutil.copytree(PROJECT/'examples/minimal',source)
 manifest=source/'manifest.toml';manifest.write_text('title="Moon Garden"\ndescription="Explore a quiet puzzle garden."\nauthor="Example Studio"\nhomepage="https://example.com/game"\ngenres=["Puzzle", "Adventure"]\n'+manifest.read_text().replace('com.gamepkg.minimal','com.example.metadata'))
 key=root/'key';key.write_text('29'*32);package=root/'game.gamepkg';run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator')
 catalog=root/'games.gamepkglist';catalog.write_text('''format_version=1
[[games]]
id="com.example.preview"
title="Catalog Preview"
description="Try this puzzle before downloading."
download_url="https://example.invalid/preview.gamepkg"
[[games]]
id="com.example.metadata"
title="Misleading Catalog Title"
description="Untrusted catalog description."
download_url="https://example.invalid/game.gamepkg"
''');run('import-list',catalog)
 listed=run('list');assert 'Moon Garden' in listed and 'Explore a quiet puzzle garden.' in listed
 server=start_display(env,1500,1100);app=None
 try:
  app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
  deadline=time.monotonic()+20
  while True:
   found=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(app.pid),'--name','^GamePKG$'],env=env,capture_output=True,text=True)
   if found.returncode==0:break
   assert app.poll() is None;assert time.monotonic()<deadline;time.sleep(.1)
  window=found.stdout.splitlines()[0];subprocess.run(['xdotool','windowsize',window,'1400','1000'],env=env,check=True);time.sleep(.5)
  def screen(name):
   path=proof/name;subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
   return subprocess.check_output(['tesseract',str(path),'stdout','--psm','11'],stderr=subprocess.DEVNULL,text=True)
  click_text(window,env,'Catalog Preview',root);time.sleep(.4)
  preview=screen('catalog-description.png');assert 'Try this puzzle before downloading.' in preview,preview
  assert not (home/'com.example.preview.gamepkg').exists()
  click_text(window,env,'Back to library',root);click_text(window,env,'Moon Garden',root);time.sleep(.4)
  signed=screen('signed-description.png')
  for text in ['Moon Garden','Explore a quiet puzzle garden.','Example Studio','Puzzle','Adventure','Project website']:assert text in signed,signed
  assert 'Untrusted catalog description' not in signed and 'Misleading Catalog Title' not in signed,signed
  print('Metadata GUI passed: offline catalog previews, signed titles/descriptions/author/genres/homepage, CLI metadata and signed-over-catalog precedence.')
 finally:
  if app:app.terminate();app.wait(timeout=10)
  server.terminate();server.wait(timeout=10)
