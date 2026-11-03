"""Maintainer check: real game window preferences, scoped storage and safe fallback."""
from pathlib import Path
import json, os, subprocess, tempfile, time, zlib, math
from PIL import Image
from gui_test_support import start_display

PROJECT = Path(__file__).resolve().parents[1]
BIN = Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', PROJECT / 'target/release'))
GAME = 'com.gamepkg.arkanoo'
TITLE=json.loads((PROJECT/'tests/fixtures/arkanoo-menu-controls.json').read_text())['labels']['ARKANOO']
TITLE_ALPHA=Image.frombytes('RGBA',(TITLE['width'],TITLE['height']),zlib.decompress(bytes.fromhex(TITLE['rgba_zlib_hex']))).getchannel('A')

with tempfile.TemporaryDirectory(prefix='gamepkg-window-preferences-', dir=PROJECT / 'target') as temporary:
    root = Path(temporary)
    config = root / 'settings.toml'
    config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nrenderer="opengl"\nui_scale_percent=150\n')
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'), GAMEPKG_CONFIG=str(config),
               LIBGL_ALWAYS_SOFTWARE='1', DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus', OMP_THREAD_LIMIT='1')
    env.pop('WAYLAND_DISPLAY', None)
    def run(*args):
        result=subprocess.run(list(map(str,args)), env=env, text=True, capture_output=True, timeout=120)
        assert result.returncode==0,(args,result.stderr)
        return result.stdout
    key=root/'key'; key.write_text('71'*32)
    package=root/'game.gamepkg'
    run(BIN/'gamepkg','pack',PROJECT/'ports/arkanoo','-o',package,'--key',key)
    run(BIN/'gamepkg','install',package,'--trust-creator')
    run(BIN/'gamepkg','build',GAME,'--target','wasm')
    display=start_display(env,width=1400,height=1000)
    wm=None;app=None;logs=[]
    def wait_for(check, message, seconds=15):
        end=time.monotonic()+seconds
        while True:
            result=check()
            if result:return result
            if time.monotonic()>=end:raise AssertionError(message)
            if app is not None and app.poll() is not None:raise AssertionError('Game exited before '+message)
            time.sleep(.05)
    def geometry(window):
        return {key:int(value) for key,value in (line.split('=',1) for line in run('xdotool','getwindowgeometry','--shell',window).splitlines()) if key in ('WIDTH','HEIGHT')}
    def fullscreen(window):return '_NET_WM_STATE_FULLSCREEN' in run('xprop','-id',window,'_NET_WM_STATE')
    def launch(library=False):
        global app
        log=open(root/f'launch-{len(logs)}.log','w+');logs.append(log)
        args=[str(BIN/'gamepkg-gui')]
        if not library:args+=['--play',GAME,'--target','wasm']
        app=subprocess.Popen(args,env=env,stdout=log,stderr=log)
        name='^GamePKG$' if library else '^'+GAME.replace('.','\\.')+'$'
        def found():
            result=subprocess.run(['xdotool','search','--onlyvisible','--name',name],env=env,capture_output=True,text=True)
            return result.stdout.splitlines()[0] if result.returncode==0 else None
        window=wait_for(found,'visible game window')
        run('xdotool','windowactivate','--sync',window)
        if not library:
            def rendered():
                screenshot=root/'frame.png'
                run('import','-display',env['DISPLAY'],'-window',window,screenshot)
                with Image.open(screenshot) as picture:
                    picture=picture.convert('RGB');scale=min(picture.width/640,picture.height/480)
                    left=(picture.width-640*scale)/2+290*scale;top=(picture.height-480*scale)/2+150*scale
                    positive=negative=0
                    for py in range(max(0,math.floor(top)),min(picture.height,math.ceil(top+15.5*scale))):
                        for px in range(max(0,math.floor(left)),min(picture.width,math.ceil(left+60*scale))):
                            sx=max(0,min(119,(px+.5-left)*2/scale-.5));sy=max(0,min(30,(py+.5-top)*2/scale-.5));ix=int(sx);iy=int(sy);fx=sx-ix;fy=sy-iy
                            alpha=sum(TITLE_ALPHA.getpixel((xx,yy))*wx*wy for xx,wx in [(ix,1-fx),(min(ix+1,119),fx)] for yy,wy in [(iy,1-fy),(min(iy+1,30),fy)])
                            color=picture.getpixel((px,py))
                            if alpha>=254:
                                positive+=1
                                if min(color)<252:return False
                            elif alpha==0:
                                negative+=1
                                if max(color)>107:return False
                    return positive>10 and negative>10
            # Skip the independently tested splash through its real original input.
            time.sleep(.15);run('xdotool','key','--window',window,'Return')
            wait_for(rendered,'original ARKANOO title glyphs rendered')
        return window
    def close(window, quit_key=True):
        global app
        if quit_key:
            run('xdotool','key','--window',window,'Up');time.sleep(.15)
            run('xdotool','key','--window',window,'Escape');time.sleep(.15)
            run('xdotool','key','--window',window,'q')
        else:
            run('xdotool','windowactivate','--sync',window)
            run('xdotool','key','alt+F4')
        assert app.wait(timeout=15)==0,'Window did not close cleanly'
        app=None
    try:
        wm=subprocess.Popen(['metacity','--sm-disable','--replace','--no-composite'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        wait_for(lambda:'window id #' in run('xprop','-root','_NET_SUPPORTING_WM_CHECK'),'private window manager')
        window=launch()
        run('xdotool','windowsize','--sync',window,'1100','650')
        wait_for(lambda:geometry(window)=={'WIDTH':1100,'HEIGHT':650},'resized normal window')
        time.sleep(.2)
        run('xdotool','key','--window',window,'F11')
        wait_for(lambda:fullscreen(window),'F11 fullscreen')
        close(window)
        records=list((root/'library/window-preferences').glob('*.json'))
        assert len(records)==1,records
        record=records[0]
        saved=json.loads(record.read_text())
        assert saved==dict(version=1,width=1100,height=650,fullscreen=True,maximized=False,vsync=True),saved
        window=launch()
        assert fullscreen(window),'Fullscreen was not restored on reopen'
        run('xdotool','key','--window',window,'F11')
        wait_for(lambda:not fullscreen(window),'F11 leaves restored fullscreen')
        wait_for(lambda:geometry(window)=={'WIDTH':1100,'HEIGHT':650},'original normal bounds after fullscreen')
        time.sleep(.2)
        close(window,quit_key=False)
        saved=json.loads(record.read_text())
        assert not saved['fullscreen'] and (saved['width'],saved['height'])==(1100,650),saved
        window=launch()
        assert not fullscreen(window)
        assert geometry(window)=={'WIDTH':1100,'HEIGHT':650}
        close(window)
        window=launch()
        run('wmctrl','-ir',hex(int(window)),'-b','add,maximized_vert,maximized_horz')
        wait_for(lambda:'_NET_WM_STATE_MAXIMIZED_VERT' in run('xprop','-id',window,'_NET_WM_STATE'),'maximized window')
        time.sleep(.2)
        close(window,quit_key=False)
        saved=json.loads(record.read_text())
        assert saved['maximized'] and (saved['width'],saved['height'])==(1100,650),saved
        window=launch()
        wait_for(lambda:'_NET_WM_STATE_MAXIMIZED_VERT' in run('xprop','-id',window,'_NET_WM_STATE'),'restored maximized state')
        run('xdotool','key','--window',window,'F11')
        wait_for(lambda:fullscreen(window),'fullscreen from maximized state')
        close(window)
        saved=json.loads(record.read_text())
        assert saved['fullscreen'] and saved['maximized'],saved
        window=launch()
        assert fullscreen(window),'Fullscreen from maximized state was not restored'
        run('xdotool','key','--window',window,'F11')
        wait_for(lambda:not fullscreen(window),'exit fullscreen into maximized state')
        wait_for(lambda:'_NET_WM_STATE_MAXIMIZED_VERT' in run('xprop','-id',window,'_NET_WM_STATE'),'maximized state after fullscreen reopen')
        run('wmctrl','-ir',hex(int(window)),'-b','remove,maximized_vert,maximized_horz')
        wait_for(lambda:geometry(window)=={'WIDTH':1100,'HEIGHT':650},'normal bounds after maximization')
        time.sleep(.2)
        close(window,quit_key=False)
        previous=record.read_bytes()
        window=launch(library=True)
        assert not fullscreen(window)
        assert geometry(window)=={'WIDTH':1280,'HEIGHT':820}
        close(window,quit_key=False)
        assert record.read_bytes()==previous,'Library window overwrote game preferences'
        damaged=b'{"version":1,"width":999999,"height":650,"fullscreen":true,"maximized":false}'
        record.write_bytes(damaged)
        window=launch()
        assert not fullscreen(window)
        assert geometry(window)=={'WIDTH':960,'HEIGHT':720}
        close(window)
        assert record.read_bytes()==damaged,'Invalid settings were silently overwritten'
        print('Native window settings: resize/fullscreen/maximized combinations, F11, normal-size preservation, game/OS close, library isolation and damaged-record fallback passed.')
    finally:
        if app is not None and app.poll() is None:
            app.terminate()
            try:app.wait(timeout=10)
            except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
        for log in logs:log.close()
        if wm is not None:wm.terminate();wm.wait(timeout=10)
        display.terminate();display.wait(timeout=10)
