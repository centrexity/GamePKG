"""Verify AV1 frame progression and explicit Theora fallback in a virtual GUI."""
from pathlib import Path
import os, shutil, subprocess, tempfile, time
root=Path(tempfile.mkdtemp(prefix='gamepkg-preview-gui-')); project=Path(__file__).resolve().parents[1]
source=root/'source';shutil.copytree(project/'examples/minimal',source)
cli=project/'target/release/gamepkg';key=root/'creator.key';env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'),DISPLAY=':98',LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
(root/'settings.toml').write_text('automatic_update_checks = false\n')
subprocess.run([str(cli),'keygen','-o',str(key)],check=True,capture_output=True)
for name,codec,color in [('preview.av1.webm','libaom-av1','red'),('preview.ogv','libtheora','blue')]:
 subprocess.run(['ffmpeg','-v','error','-f','lavfi','-i',f'color=c={color}:s=64x64:r=5','-t','4','-an','-vf',"drawbox=color=lime:t=fill:enable='gte(t,2)'" if codec=='libaom-av1' else 'null','-c:v',codec,'-threads','1',str(source/name)],check=True)
package=root/'sample.gamepkg';subprocess.run([str(cli),'pack',str(source),'-o',str(package),'--key',str(key)],env=env,check=True,capture_output=True);subprocess.run([str(cli),'install',str(package),'--trust-creator'],env=env,check=True,capture_output=True)
display=subprocess.Popen(['Xvfb',':98','-screen','0','1600x1000x24','-ac'],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE);time.sleep(1)
app=subprocess.Popen([str(project/'target/release/gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
try:
 for i in range(100):
  state=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
  if state.returncode==0: break
  if app.poll() is not None: raise RuntimeError(app.stderr.read().decode())
  time.sleep(.1)
 window=state.stdout.strip().splitlines()[0];subprocess.run(['xdotool','windowsize',window,'1500','950'],env=env,check=True);time.sleep(.5)
 subprocess.run(['import','-display',':98','-window',window,str(project/'target/preview-library.png')],env=env,check=True)
 print('window='+window,flush=True)
 subprocess.run(['xdotool','mousemove','--window',window,'478','150','click','1'],env=env,check=True);time.sleep(1)
 subprocess.run(['import','-display',':98','-window',window,str(project/'target/preview-details.png')],env=env,check=True)
 from PIL import Image
 def count_color(color, screenshot):
  subprocess.run(['import','-display',':98','-window',window,str(screenshot)],env=env,check=True)
  data=Image.open(screenshot).convert('RGB').tobytes()
  pixels=zip(data[0::3],data[1::3],data[2::3])
  return sum(1 for r,g,b in pixels if (r>200 and g<60 and b<60 if color=='red' else g>200 and r<60 and b<60 if color=='green' else b>200 and r<60 and g<60))
 def await_color(color, name):
  screenshot=project/'target'/name
  for _ in range(100):
   if count_color(color,screenshot)>50000:return
   if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
   time.sleep(.1)
  raise AssertionError(f'{color} preview did not render')
 subprocess.run(['xdotool','mousemove','--window',window,'50','496','click','1'],env=env,check=True)
 await_color('red','preview-av1-first.png')
 await_color('green','preview-av1-later.png')
 subprocess.run(['xdotool','mousemove','--window',window,'160','496','click','1'],env=env,check=True)
 await_color('blue','preview-theora.png')
 subprocess.run(['xdotool','mousemove','--window',window,'85','10','click','1'],env=env,check=True);time.sleep(.3)
 assert count_color('blue',project/'target/settings-after-preview.png')==0, 'preview remained visible in settings'
 assert not (root/'library/builds').exists(), 'preview started a game build'
 print('GUI preview passed: AV1 frames advance and explicit Theora fallback renders.',flush=True)
finally:
 app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10);shutil.rmtree(root)
