"""Recorded music must retain its output clock in actual muted game windows."""
from pathlib import Path
import argparse,os,shutil,subprocess,tempfile,time,wave
from PIL import Image
from gui_test_support import start_display,game_window
parser=argparse.ArgumentParser();parser.add_argument('--smoke',action='store_true');args=parser.parse_args()
project=Path(__file__).resolve().parents[1];cli=project/'target/release/gamepkg';gui=project/'target/release/gamepkg-gui'
with tempfile.TemporaryDirectory(prefix='gamepkg-muted-clock-') as temporary:
 root=Path(temporary);recipe=root/'recipe';shutil.copytree(project/'examples/minimal',recipe)
 text=(recipe/'manifest.toml').read_text().replace('com.gamepkg.minimal','com.gamepkg.muted-clock').replace('audio = false','audio = true').replace('[limits]','[limits]\nmusic_memory_bytes=7340032')
 (recipe/'manifest.toml').write_text(text)
 with wave.open(str(root/'tone.wav'),'wb') as output:
  output.setnchannels(2);output.setsampwidth(2);output.setframerate(44100);output.writeframes((b'\x39\x30\x60\xa4')*4096)
 subprocess.run(['ffmpeg','-v','error','-i',str(root/'tone.wav'),'-codec:a','libmp3lame','-write_xing','0',str(recipe/'assets/tone.mp3')],check=True)
 (recipe/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,state:u64)->u64 {
  let mut next=state;if state==0 {next=if gamepkg::music_play("tone.mp3") {1}else{2};}
  let color=if next==2 {0x0000ffff}else if gamepkg::music_failed() {0xff00ffff}else if gamepkg::music_finished() {0x00ff00ff}else{0xff0000ff};
  gamepkg::draw_clear(color);next
 }''')
 (root/'synthetic.key').write_text('75'*32);(root/'null.conf').write_text('pcm.!default { type null }\n');(root/'unavailable.conf').write_text('pcm.!default { type deliberately_missing_fixture_pcm }\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'),ALSA_CONFIG_PATH=str(root/'null.conf'),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
 base='automatic_update_checks=false\ncheck_app_updates_on_startup=false\n';(root/'settings.toml').write_text(base)
 def run(*arguments):
  result=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stderr
 id='com.gamepkg.muted-clock';run('pack',recipe,'-o',root/'fixture.gamepkg','--key',root/'synthetic.key');run('install',root/'fixture.gamepkg','--trust-creator');run('build',id,'--target','wasm');run('build',id,'--target','native')
 display=start_display(env,1100,850)
 try:
  for target in (['wasm'] if args.smoke else ['wasm','native']):
   for renderer in (['vulkan'] if args.smoke else ['vulkan','opengl']):
    for muted,volume,available in [(False,100,True),(True,100,True),(False,0,True),(True,100,False)]:
     (root/'settings.toml').write_text(base+f'mute_audio={str(muted).lower()}\naudio_volume={volume}\n')
     selected=dict(env,ALSA_CONFIG_PATH=str(root/('null.conf' if available else 'unavailable.conf')))
     app=subprocess.Popen([str(gui),'--play',id,'--target',target,'--renderer',renderer],env=selected,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
     try:
      window,_=game_window(env,id);end=time.monotonic()+10
      while True:
       assert app.poll() is None,'Game exited before music completed'
       frame=root/'frame.png';subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(frame)],env=env,check=True)
       image=Image.open(frame).convert('RGB');actual=image.getpixel((image.width//2,image.height//2))
       if actual==((0,255,0) if available else (0,0,255)):break
       assert time.monotonic()<end,(target,renderer,muted,volume,actual,'music clock never completed')
       time.sleep(.05)
      print(target,renderer,'mute',muted,'volume',volume,'device',available,'actual completion or explicit playback denial',flush=True)
     finally:
      if app.poll() is None:app.terminate()
      out,err=app.communicate(timeout=10)
      assert b'Unable to prepare recorded audio' not in err,err.decode()
 finally:
  display.terminate();display.wait(timeout=10)
