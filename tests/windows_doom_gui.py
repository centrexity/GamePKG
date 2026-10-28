"""Real Windows DOOM launcher integration with public data and muted output.

No player media, WSL, system graphics install or product test hooks. Screenshot
checks prove presentation/input integration, not pixel-perfect game fidelity.
"""
import hashlib, io, json, os, platform, struct, subprocess, sys, tempfile, time, tomllib, urllib.request, zipfile
from pathlib import Path
from PIL import ImageChops
from windows_owned_window import OwnedGameWindow, read_shared_file
from doom_gui_proof import bundle, gamma
PROJECT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(PROJECT/'tools'))
from package_release import executable_architecture
ARCHIVE_SHA='3f9b264f3e3ce503b4fb7f6bdcb1f419d93c7b546f4df3e874dd878db9688f59'
WAD_SHA='7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d'

def pixels(image):return image.get_flattened_data() if hasattr(image,'get_flattened_data') else image.getdata()

def public_wad(path):
 url='https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip'
 with urllib.request.urlopen(url,timeout=180) as response:data=response.read(24143782)
 assert len(data)==24143781 and hashlib.sha256(data).hexdigest()==ARCHIVE_SHA,'public fixture archive pin mismatch'
 with zipfile.ZipFile(io.BytesIO(data)) as archive:
  entry=archive.getinfo('freedoom-0.13.0/freedoom1.wad');assert entry.file_size==28795076
  wad=archive.read(entry)
 assert hashlib.sha256(wad).hexdigest()==WAD_SHA,'public WAD pin mismatch'
 path.write_bytes(wad)

def remembered_identity(manifest):
 # Match the documented host-only identity without introducing a product hook.
 roles=[{'id':item['id'],'title':item['title']} for item in manifest['roms']]
 value=[manifest['id'],manifest['creator_key'],manifest.get('requires_rom',bool(roles)),roles]
 return hashlib.sha256(json.dumps(value,ensure_ascii=False,separators=(',',':')).encode()).hexdigest()

def wait_image(window,name):
 end=time.monotonic()+30
 while True:
  image=window.capture(name)
  if len(set(pixels(image)))>64 and sum(max(rgb)-min(rgb)>16 for rgb in pixels(image))>1000:return image
  assert time.monotonic()<end,'no original game image appeared';time.sleep(.1)

def require_game_viewport(image):
 width,height=image.size;scale=min(width/640,height/480)
 left=(width-640*scale)/2;top=(height-480*scale)/2
 # Check letterbox fields across the whole client, outside the raster boundary.
 if left>=2:
  for x in [0,width-1]:
   assert all(image.getpixel((x,y))==(0,0,0) for y in range(height)),'non-game pixels in side letterbox'
 if top>=2:
  for y in [0,height-1]:
   assert all(image.getpixel((x,y))==(0,0,0) for x in range(width)),'non-game pixels in top/bottom letterbox'
 interior=image.crop((int(left)+1,int(top)+1,int(width-left)-1,int(height-top)-1))
 assert len(set(pixels(interior)))>64,'blank game viewport'
 assert sum(max(rgb)>16 for rgb in pixels(interior))>interior.width*interior.height//5,'game image only covers a small part of the viewport'
 assert sum(max(rgb)-min(rgb)>16 for rgb in pixels(interior))>1000,'gray application UI is not a game frame'

def checkpoint(home):
 files=list((home/'saves').glob('*.blob'))
 assert len(files)<=1,'unexpected extra game save scope'
 if not files:return None
 try:return read_shared_file(files[0],4*1024*1024)
 except FileNotFoundError:return None

def wait_save(window,home,name):
 end=time.monotonic()+15;last='no checkpoint'
 while True:
  window.alive();data=checkpoint(home)
  if data is not None:
   try:
    slot=bundle(data)[0]
    if slot[:24].split(b'\0',1)[0]==name.encode():return data
    last='different original save description'
   except AssertionError as error:last=str(error)
  assert time.monotonic()<end,f'physical save-name input did not commit: {last}';time.sleep(.1)

def save_named(window,home,name):
 window.tap(0x3c);window.tap(0x1c) # Original F2 menu and slot zero.
 for _ in range(24):window.tap(0x0e)
 scans={'G':0x22,'U':0x16,'I':0x17,' ':0x39,'S':0x1f,'A':0x1e,'V':0x2f,'E':0x12,'R':0x13,'L':0x26,'O':0x18,'D':0x20}
 for character in name:window.tap(scans[character])
 window.tap(0x1c)
 return wait_save(window,home,name)

def main():
 assert platform.system()=='Windows','requires actual Windows'
 cli=PROJECT/'target/release/gamepkg.exe';gui=PROJECT/'target/release/gamepkg-gui.exe'
 architecture=executable_architecture(cli,True)
 assert executable_architecture(gui,True)==architecture
 assert architecture==os.environ.get('GAMEPKG_TEST_EXPECTED_ARCH',architecture)
 proof=PROJECT/'target/windows-gui-proof'
 with tempfile.TemporaryDirectory(prefix='gamepkg public Doom GUI ') as directory:
  root=Path(directory);home=root/'library';config=root/'settings.toml';wad=root/'public-freedoom1.wad'
  public_wad(wad)
  env={k:v for k,v in os.environ.items() if not k.startswith('GAMEPKG_')}
  env.update(GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(config),SDL_AUDIODRIVER='dummy',ALSOFT_DRIVERS='null',GALLIUM_DRIVER='llvmpipe',LIBGL_ALWAYS_SOFTWARE='1')
  def settings(renderer):
   config.write_text(f'automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\ntick_deadline_ms=500\nrenderer="{renderer}"\nnative_watchdog_seconds=0\n')
  def run(*arguments):
   result=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=900)
   assert result.returncode==0,(result.stdout+'\n'+result.stderr)[-5000:]
   return result.stdout
  settings('opengl');key=root/'creator.key';package=root/'doom.gamepkg'
  run('keygen','-o',key);run('pack',PROJECT/'ports/doom','-o',package,'--key',key);run('install',package,'--trust-creator')
  run('build','com.gamepkg.doom') # Public default Build, with no target override.
  assert not list((home/'builds').rglob('*.wasm')),'default Build produced Wasm'
  binaries=list((home/'builds').glob('*/native-v1-*/game'));assert len(binaries)==1
  assert executable_architecture(binaries[0],True)==architecture
  run('integrity-check','com.gamepkg.doom')
  with zipfile.ZipFile(package) as archive:manifest=tomllib.loads(archive.read('manifest.toml').decode())
  selections=home/'rom-selections.json';selections.write_text(json.dumps({remembered_identity(manifest):[str(wad)]}))
  remembered=selections.read_bytes();previous=0
  for renderer in ['opengl','vulkan']:
   settings(renderer);environment=dict(env,GAMEPKG_RENDERER=renderer)
   if renderer=='vulkan':
    icd=PROJECT/'target/release'/('lvp_icd.'+('x86' if architecture=='i686' else 'x86_64')+'.json')
    assert icd.is_file();assert executable_architecture(PROJECT/'target/release/vulkan-1.dll',True)==architecture
    environment.update(VK_DRIVER_FILES=str(icd),VK_ICD_FILENAMES=str(icd))
   for launch in ['first','restart']:
    label=f'doom-{renderer}-{launch}';loaded=checkpoint(home) is not None
    with OwnedGameWindow(gui,['--play','com.gamepkg.doom'],environment,proof,label) as window:
     wait_image(window,'initial')
     if not loaded:
      for _ in range(4):window.tap(0x1c);time.sleep(.4)
     time.sleep(.5);before=wait_image(window,'before-movement');require_game_viewport(before)
     window.key(0x48,True,True);time.sleep(.35);window.key(0x48,False,True);time.sleep(.2)
     after=window.capture('after-movement');require_game_viewport(after)
     changed=sum(max(rgb)>30 for rgb in pixels(ImageChops.difference(before,after)))
     assert changed>1000,'no visible movement/input response'
     name='GUI SAVE' if launch=='first' else 'GUI RELOAD'
     saved=save_named(window,home,name);base_gamma=gamma(saved);size=window.size()
     window.tap(0x57) # Original F11 gamma, not a window/fullscreen command.
     end=time.monotonic()+10
     while gamma(checkpoint(home))!=(base_gamma+1)%5:
      window.alive();assert time.monotonic()<end,'F11 did not persist original gamma';time.sleep(.1)
     assert window.size()==size,'F11 resized the game window'
     window.capture('gamma');window.resize(853,641);time.sleep(.3)
     end=time.monotonic()+15
     while True:
      try:require_game_viewport(window.capture('resized'));break
      except AssertionError:
       window.alive();assert time.monotonic()<end,'resized game viewport did not settle';time.sleep(.1)
     window.close()
    records=list((home/'progress').glob('*.json'));assert len(records)==1
    played=json.loads(records[0].read_text())['played_ms'];assert played>previous
    previous=played;assert selections.read_bytes()==remembered,'launch changed remembered ROM selection'
    assert hashlib.sha256(wad.read_bytes()).hexdigest()==WAD_SHA,'public WAD changed'
    run('integrity-check','com.gamepkg.doom')
    (proof/(label+'-history.json')).write_text(json.dumps({'architecture':architecture,'renderer':renderer,'launch':launch,'played_ms':played,'gamma':gamma(checkpoint(home)),'wad_sha256':WAD_SHA,'native_build_only':True,'remembered_rom_unchanged':True},indent=2)+'\n')
    print(f'Verified Windows DOOM GUI: {renderer} {launch}; default native Build/Play, remembered WAD, visible image/movement, named checkpoint, F11 gamma, resize, close and increasing history.',flush=True)
  print('Windows public-Freedoom normal launcher integration passed; muted audio; no user media or WSL/Python player dependency.',flush=True)

if __name__=='__main__':main()
