"""Real resolution and original crosshair compatibility; no data is copied.
Prepare tests/cube2_package.py first. GAMEPKG_CUBE2_TEST_DATA selects an optional
older original installation; pass --legacy to assert the old crosshair path.
"""
from pathlib import Path
import os, subprocess, tempfile, time, sys
from PIL import Image
from gui_test_support import start_display, game_window, close_game, wait_game_frame
PROJECT=Path(__file__).resolve().parents[1]
LIBRARY=PROJECT/'target/cube2-package-test/library'
DATA=Path(os.environ.get('GAMEPKG_CUBE2_TEST_DATA',str(PROJECT/'target/cube2-test-data')))
proof=PROJECT/'target/cube2-resolution-gui';proof.mkdir(exist_ok=True)
remembered=LIBRARY/'game-data-selections.json'
old=remembered.read_bytes() if remembered.exists() else None
saves={p:p.read_bytes() for p in (LIBRARY/'saves').glob('*.cube2')}
with tempfile.TemporaryDirectory(prefix='gamepkg-resolution-') as temporary:
    config=Path(temporary)/'settings.toml'
    config.write_text('memory_ceiling_mib=2048\ntick_deadline_ms=500\ncheck_app_updates_on_startup=false\nautomatic_update_checks=false\nmute_audio=true\n')
    env=dict(os.environ,GAMEPKG_HOME=str(LIBRARY),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',GAMEPKG_RENDERER='vulkan',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    env.pop('WAYLAND_DISPLAY',None)
    server=start_display(env,1800,1100);app=None
    def command(*args):
        return subprocess.run(list(map(str,args)),env=env,check=True,capture_output=True,timeout=90)
    try:
        command(PROJECT/'target/release/gamepkg','run','com.gamepkg.cube2','--game-data',DATA,'--frames','3')
        log=(proof/'player.log').open('wb')
        app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui'),'--play','com.gamepkg.cube2'],env=env,stdout=log,stderr=log)
        window,_=game_window(env,'com.gamepkg.cube2',timeout=30)
        command('xdotool','windowfocus',window)
        wait_game_frame(window,env,proof,timeout=30)
        for width,height in [(1000,800),(1600,900)]:
            command('xdotool','windowsize',window,width,height)
            time.sleep(2)
            path=proof/f'{width}x{height}.png'
            command('import','-display',env['DISPLAY'],'-window',window,path)
            image=Image.open(path).convert('RGB')
            assert image.size==(width,height),image.size
            # Before the fix, a 16:9 window retained wide black side bars.
            for x in [10,width-11]:
                strip=image.crop((x,100,x+1,height-100))
                assert sum(max(strip.getpixel((0,y)))>20 for y in range(strip.height))>100,'Game does not fill the window aspect ratio'
        image=Image.open(proof/'1600x900.png').convert('RGB')
        center=image.crop((770,420,830,480))
        assert sum(min(center.getpixel((x,y)))>180 for x in range(center.width) for y in range(center.height))>=8,'Original gameplay crosshair is missing'
        # Ask the original engine to persist its actual rendering dimensions.
        command('xdotool','key','t');time.sleep(.3)
        command('xdotool','type','--clearmodifiers','--delay','20','/name (concatword $scr_w x $scr_h)')
        command('xdotool','key','Return');time.sleep(.5)
        close_game(window,env);assert app.wait(timeout=15)==0,(proof/'player.log').read_text()
        saved=b''.join(p.read_bytes() for p in (LIBRARY/'saves').glob('*.cube2'))
        assert b'1280x720' in saved,'Engine retained the fixed 640x480 resolution'
        expected=b'data/crosshair.png' if '--legacy' in sys.argv else b'packages/crosshairs/default.png'
        assert b'loadcrosshair "'+expected+b'" 0' in saved,'Original crosshair asset was not loaded'
        print('Cube 2 passed: live resize, 5:4 and 16:9 presentation, 1280x720 engine render, visible original crosshair:',expected.decode())
    finally:
        if app and app.poll() is None:
            command('import','-display',env['DISPLAY'],'-window','root',proof/'failure.png')
            app.terminate();app.wait(timeout=10)
        if old is None:remembered.unlink(missing_ok=True)
        else:remembered.write_bytes(old)
        for path,data in saves.items():path.write_bytes(data)
        server.terminate();server.wait(timeout=10)
