"""Build Cube 2 at the default memory ceiling; explain the blocker and enable Play on Save.

Requires the existing target/cube2-test-data fixture. Never changes the user's
library, global settings or original game files.
"""
from pathlib import Path
import os, shutil, subprocess, tempfile, time, tomllib
from PIL import Image
from gui_test_support import start_display, game_window, click_text, choose_native_file, click_primary_play, close_game
PROJECT=Path(__file__).resolve().parents[1]
assert (PROJECT/'target/cube2-test-data/data').is_dir(), 'Supply the existing read-only game-data fixture'
os.environ.setdefault('OMP_THREAD_LIMIT','1')
with tempfile.TemporaryDirectory(prefix='gamepkg-cube2-readiness-') as temporary:
    root=Path(temporary);library=root/'library';config=root/'settings.toml'
    config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmemory_ceiling_mib=512\ntick_deadline_ms=500\nmute_audio=true\n')
    env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',OMP_THREAD_LIMIT='1',GAMEPKG_RENDERER='vulkan',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
    cli=PROJECT/'target/release/gamepkg'
    def run(*args):subprocess.run([str(cli),*map(str,args)],env=env,check=True,capture_output=True,text=True,timeout=60)
    key=root/'key';key.write_text('42'*32);package=root/'cube2.gamepkg'
    run('pack',PROJECT/'ports/cube2','-o',package,'--key',key);run('install',package,'--trust-creator')
    server=start_display(env,1500,1100);app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    player=None
    try:
        window,_=game_window(env,'GamePKG');subprocess.run(['xdotool','windowsize',window,'1400','1000'],env=env,check=True);time.sleep(.4)
        def click(label,top=None):click_text(window,env,label,root,max_y=top);time.sleep(.3)
        def screen(name):
            path=root/(name+'.png');subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
            return subprocess.check_output(['tesseract',str(path),'stdout','--psm','11'],stderr=subprocess.DEVNULL,text=True).casefold()
        click('Cube',600)
        initial=screen('before');assert '2048' in initial and '512' in initial,initial
        click('Choose game-data')
        choose_native_file(env,'Choose original Sauerbraten installation',PROJECT/'target/cube2-test-data')
        subprocess.run(['xdotool','windowfocus',window,'mousemove','--window',window,'850','100','click','1'],env=env,check=True);time.sleep(.5)
        click('Build',700)
        deadline=time.monotonic()+240
        while True:
            text=screen('build')
            if 'native build verified' in text:break
            assert app.poll() is None and time.monotonic()<deadline,text
            assert 'failed:' not in text,text
            time.sleep(1)
        assert '2048' in text and '512' in text,text
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(PROJECT/'target/cube2-readiness-blocked.png')],env=env,check=True)
        assert tomllib.loads(config.read_text())['memory_ceiling_mib']==512,'Build silently raised the memory ceiling'
        click('Open memory settings',800)
        _,row_y=click_text(window,env,'Memory ceiling',root,click=False)
        x,y=click_text(window,env,'512',root,click=False,max_y=row_y+25)
        subprocess.run(['xdotool','mousemove','--window',window,str(x),str(y),'click','1'],env=env,check=True);time.sleep(.3)
        subprocess.run(['xdotool','key','--clearmodifiers','ctrl+a','type','--clearmodifiers','2048'],env=env,check=True);time.sleep(.3)
        subprocess.run(['xdotool','key','Return'],env=env,check=True);time.sleep(.3)
        click('Save settings')
        if tomllib.loads(config.read_text())['memory_ceiling_mib']!=2048:
            subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(PROJECT/'target/cube2-readiness-memory-failed.png')],env=env,check=True)
            raise AssertionError('Memory control edit was not saved as 2048 MiB')
        click('Library',70);click('Cube',650)
        ready=screen('ready');assert 'ready to play' in ready and 'currently 512' not in ready,ready
        # No Refresh/Recheck action: saving settings must enable the existing build.
        click_primary_play(window,env,root)
        player,_=game_window(env,'com.gamepkg.cube2',timeout=30)
        deadline=time.monotonic()+40
        while True:
            frame=root/'game.png';subprocess.run(['import','-display',env['DISPLAY'],'-window',player,str(frame)],env=env,check=True)
            image=Image.open(frame).convert('RGB')
            if len(image.getcolors(image.width*image.height))>500:break
            assert time.monotonic()<deadline,'No real Cube 2 game frame'
            time.sleep(.2)
        close_game(player,env);player=None
        subprocess.run(['xdotool','windowfocus',window,'mousemove','--window',window,'1100','100','click','1'],env=env,check=True);time.sleep(.3)
        # Losing required files must disable readiness on the next ordinary frame.
        selections=library/'game-data-selections.json';old=selections.read_text()
        import json
        selections.write_text(json.dumps({k:str(root/'missing-data') for k in json.loads(old)}))
        click('Recheck readiness',800)
        missing=screen('missing');assert 'unavailable' in missing or 'missing its' in missing,missing
        print('Cube readiness passed: actual in-app build auto-refresh, explicit 512/2048 MiB blocker, no silent memory change, settings-save readiness update, real Play, and unavailable-data explanation.')
    finally:
        if player is not None:
            try:close_game(player,env)
            except Exception:pass
        app.terminate();app.wait(timeout=10);server.terminate();server.wait(timeout=10)
