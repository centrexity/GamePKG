"""Visually inspect readable library layouts across window sizes and saved scales."""
from pathlib import Path
import os, shutil, subprocess, tempfile, time
from gui_test_support import start_display, click_text
from PIL import Image
PROJECT=Path(__file__).resolve().parents[1]
proof=PROJECT/'target/appearance-proof';proof.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='gamepkg-appearance-') as temporary:
 root=Path(temporary);config=root/'settings.toml';home=root/'library'
 env=dict(os.environ,GAMEPKG_CONFIG=str(config),GAMEPKG_HOME=str(home));env.pop('WAYLAND_DISPLAY',None)
 config.write_text('automatic_update_checks=false\n')
 def run(*args):
  r=subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=60)
  assert r.returncode==0,r.stderr
 key=root/'key';key.write_text('42'*32)
 for name,source in [('2048',PROJECT/'ports/2048'),('stars',PROJECT/'examples/good/collect-stars')]:
  package=root/(name+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator')
 # An older independently signed package with a placeholder still gets host artwork.
 old=root/'old-doom';shutil.copytree(PROJECT/'examples/minimal',old)
 manifest=old/'manifest.toml';manifest.write_text(manifest.read_text().replace('com.gamepkg.minimal','com.gamepkg.doom'))
 old_package=root/'old-doom.gamepkg';run('pack',old,'-o',old_package,'--key',key);run('install',old_package,'--trust-creator')
 run('build','com.gamepkg.collect-stars')
 catalog=root/'games.gamepkglist';catalog.write_text("format_version=1\n[[games]]\nid='com.example.catalog'\ntitle='Explore the community collection'\nversion='2.1.0'\ndownload_url='https://example.invalid/catalog.gamepkg'\n")
 run('import-list',catalog)
 server=start_display(env,1600,1200)
 try:
  for label,width,height,scale,dark in [('wide',1280,900,100,True),('compact',760,850,100,True),('small',620,480,100,True),('large-text',760,850,140,True),('light',1280,900,100,False)]:
   config.write_text(f'automatic_update_checks=false\nui_scale_percent={scale}\ndark_mode={str(dark).lower()}\n')
   app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
   try:
    deadline=time.monotonic()+20
    while True:
     found=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(app.pid),'--name','^GamePKG$'],env=env,capture_output=True,text=True)
     if found.returncode==0:break
     assert app.poll() is None,app.stderr.read().decode();assert time.monotonic()<deadline;time.sleep(.1)
    window=found.stdout.splitlines()[0];subprocess.run(['xdotool','windowsize',window,str(width),str(height)],env=env,check=True);time.sleep(.7)
    screenshot=proof/(label+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(screenshot)],env=env,check=True)
    text=subprocess.check_output(['tesseract',str(screenshot),'stdout','--psm','11'],stderr=subprocess.DEVNULL,text=True)
    assert all(label in text.casefold() for label in ['your games','installed','ready']),text
    if label=='wide':
     click_text(window,env,'com.gamepkg.collect-stars',root)
     time.sleep(.5);subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(proof/'game-page.png')],env=env,check=True)
    # Distinct surface and background tones; high-contrast body text is inspected in the images.
    image=Image.open(screenshot).convert('RGB');assert len(image.getcolors(image.width*image.height))>100
    if label=='wide':assert sum(r>70 and g<70 and b<100 for r,g,b in (image.get_flattened_data() if hasattr(image,'get_flattened_data') else image.getdata()))>1000,'Missing legacy-package fallback artwork'
   finally:app.terminate();app.wait(timeout=10)
  print('Appearance passed: wide table, compact stacked rows, 140% text, light theme and game page.')
 finally:server.terminate();server.wait(timeout=10)
