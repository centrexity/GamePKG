"""Verify exact production winit pointer coordinates after a covering window closes."""
from pathlib import Path
import json, os, queue, subprocess, tempfile, threading, time
from gui_test_support import start_display
project=Path(__file__).resolve().parents[1]
build=subprocess.run(['cargo','build','--locked','--release','--all-features','--lib','--message-format=json'],cwd=project,stdout=subprocess.PIPE,text=True)
if build.returncode:
 for line in build.stdout.splitlines():
  message=json.loads(line)
  if message.get('reason')=='compiler-message':print(message['message'].get('rendered',''),flush=True)
 raise RuntimeError('Production window dependency build failed')
artifact=None
for line in build.stdout.splitlines():
 message=json.loads(line)
 if message.get('reason')=='compiler-artifact' and message['target']['name']=='winit':
  artifact=next(Path(name) for name in message['filenames'] if name.endswith('.rlib'))
assert artifact is not None
with tempfile.TemporaryDirectory(prefix='gamepkg-cursor-reentry-') as temporary:
 root=Path(temporary);binary=root/'probe'
 subprocess.run(['rustc','--edition=2021',str(project/'tests/fixtures/x11-cursor-reentry.rs'),'--extern',f'winit={artifact}','-L',f'dependency={artifact.parent}','-o',str(binary)],check=True)
 env=dict(os.environ);env.pop('WAYLAND_DISPLAY',None)
 display=start_display(env,1000,700);app=None
 try:
  app=subprocess.Popen([str(binary)],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,bufsize=1)
  events=queue.Queue()
  def reader():
   for line in app.stdout:events.put(line.strip())
  thread=threading.Thread(target=reader,daemon=True);thread.start()
  def wait(expected):
   deadline=time.monotonic()+5;seen=[]
   while time.monotonic()<deadline:
    try:line=events.get(timeout=.1)
    except queue.Empty:
     assert app.poll() is None,'Probe exited early'
     continue
    seen.append(line)
    if line==expected:return
   raise AssertionError(f'Missing {expected}; observed {seen}')
  def command(*args):return subprocess.check_output(['xdotool',*map(str,args)],env=env,text=True).strip()
  wait('READY')
  parent=command('search','--onlyvisible','--name','^gamepkg-cursor-parent$').splitlines()[0]
  child=command('search','--onlyvisible','--name','^gamepkg-cursor-child$').splitlines()[0]
  command('windowraise',parent,'windowfocus',parent,'mousemove','--window',parent,100,100);wait('P 100 100')
  command('windowraise',child,'windowfocus',child,'mousemove','--window',child,200,200);wait('C 200 200')
  command('windowunmap',child,'windowfocus',parent);wait('P 200 200')
  command('mousemove','--window',parent,100,100,'click',1)
  wait('CLICK 100 100')
  print('Real X11 reentry preserves motion back to the previous position before click.')
 finally:
  if app is not None:
   app.terminate();app.wait(timeout=10)
  display.terminate();display.wait(timeout=10)
