"""Disabled signed networking ignores damaged preferences in the real desktop UI."""
from pathlib import Path
from PIL import Image
import os, shutil, subprocess, tempfile, time
from gui_test_support import start_display, game_window, click_text, close_game, wait_game_frame

PROJECT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-disabled-network-gui-') as temporary:
    root=Path(temporary)
    recipe=root/'recipe';shutil.copytree(PROJECT/'ports/cube2',recipe)
    manifest=recipe/'manifest.toml';text=manifest.read_text()
    assert text.count('network = true')==1
    manifest.write_text(text.replace('network = true','network = false'))
    config=root/'settings.toml'
    config.write_text('automatic_update_checks=false\nmemory_ceiling_mib=2048\ntick_deadline_ms=500\nmute_audio=true\n')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),OMP_THREAD_LIMIT='1',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',GAMEPKG_RENDERER='vulkan',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    env.pop('WAYLAND_DISPLAY',None)
    def cli(*args):
        result=subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=120)
        assert result.returncode==0,(args,result.stdout,result.stderr)
    key=root/'test.key';key.write_text('6e'*32)
    cli('pack',recipe,'-o',root/'offline.gamepkg','--key',key)
    cli('install',root/'offline.gamepkg','--trust-creator')
    cli('build','com.gamepkg.cube2')
    cli('run','com.gamepkg.cube2','--game-data',PROJECT/'target/cube2-test-data','--frames','1')
    preferences=root/'library/network-selections.json'
    damaged=b'{invalid multiplayer preferences\n';preferences.write_bytes(damaged)
    server=start_display(env,1600,1050);app=None;player_pid=None
    def command(*args):
        return subprocess.run(list(map(str,args)),env=env,check=True,capture_output=True,text=True,timeout=15)
    try:
        app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
        window,_=game_window(env,'GamePKG')
        command('xdotool','windowsize',window,'1450','980','windowfocus',window)
        time.sleep(.5);click_text(window,env,'Cube',root)
        click_text(window,env,'Networking is disabled by this package.',root,click=False)
        command('import','-display',env['DISPLAY'],'-window',window,root/'disabled.png')
        visible=subprocess.check_output(['tesseract',str(root/'disabled.png'),'stdout'],stderr=subprocess.DEVNULL,text=True)
        assert 'Save multiplayer settings' not in visible,visible
        assert 'Unable to read multiplayer settings' not in visible,visible
        command('xdotool','mousemove','--window',window,'400','400','click','--repeat','30','--delay','20','4')
        time.sleep(.3)
        command('import','-display',env['DISPLAY'],'-window',window,root/'ready.png')
        image=Image.open(root/'ready.png').convert('RGB')
        # Locate the enabled primary action by its actual rendered bounds,
        # as in the full gameplay workflow; OCR misses dark text on cyan.
        rows=[]
        for y in range(image.height):
            xs=[x for x in range(image.width) if (lambda c:c[0]<100 and c[1]>170 and c[2]>140)(image.getpixel((x,y)))]
            if len(xs)>=80:rows.append((y,min(xs),max(xs)))
        assert rows,'No enabled primary Play button'
        group=[rows[0]]
        for row in rows[1:]:
            if row[0]>group[-1][0]+1:break
            group.append(row)
        command('xdotool','mousemove','--window',window,str((group[0][1]+group[0][2])//2),str((group[0][0]+group[-1][0])//2),'click','1')
        try:player,player_pid=game_window(env,'com.gamepkg.cube2',timeout=30)
        except AssertionError:
            command('import','-display',env['DISPLAY'],'-window',window,root/'launch-failed.png')
            detail=subprocess.check_output(['tesseract',str(root/'launch-failed.png'),'stdout'],stderr=subprocess.DEVNULL,text=True)
            raise AssertionError('Offline launch failed: '+detail)
        assert player_pid!=app.pid
        wait_game_frame(player,env,root,timeout=30)
        assert preferences.read_bytes()==damaged,'Offline launch rewrote unrelated preferences'
        close_game(player,env)
        assert app.poll() is None,'Closing offline game closed library'
        assert preferences.read_bytes()==damaged,'Offline close rewrote unrelated preferences'
        player_pid=None
        print('Signed disabled networking: real game page, no editable multiplayer form, corrupt preferences ignored, separate rendered game launch and normal close passed.')
    finally:
        if player_pid:
            try:os.kill(player_pid,15)
            except ProcessLookupError:pass
        if app and app.poll() is None:app.terminate();app.wait(timeout=10)
        server.terminate();server.wait(timeout=10)
