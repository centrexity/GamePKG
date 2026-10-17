"""Original menu and preferences: use the private cached engine fixture in place."""
from pathlib import Path
import os,subprocess,tempfile,time,struct,re
from gui_test_support import start_display,game_window,wait_game_frame,close_game,cube2_choose_test_map
PROJECT=Path(__file__).resolve().parents[1]
proof=PROJECT/'target/cube2-startup-gui';proof.mkdir(exist_ok=True)
library=PROJECT/'target/cube2-package-test/library'
save=next((library/'saves').glob('*.cube2'))
original_save=save.read_bytes()
config_bytes=(b'exec data/defaults.cfg\nconsize 7\nminiconsize 3\nmaxfps 144\n'
 b'bind PAUSE [name (concatword (getname) _p)]\n'
 b'bind KP1 [name (concatword (getname) _k)]\n'
 b'bind 1 [name wrong_keypad_alias]\n'
 b'bind RCTRL [name (concatword (getname) _r)]\n'
 b'bind LCTRL [name wrong_modifier_alias]\n')
name=b'config.cfg'
payload=struct.pack('<IIII',len(name),len(config_bytes),0,0)+name+config_bytes
save.write_bytes(b'GPS2'+struct.pack('<III',1,len(payload),0)+payload)

with tempfile.TemporaryDirectory(prefix='gamepkg-startup-') as temporary:
    root=Path(temporary);config=root/'settings.toml'
    config.write_text('memory_ceiling_mib=2048\ntick_deadline_ms=500\ncheck_app_updates_on_startup=false\nautomatic_update_checks=false\nmute_audio=true\n')
    env=dict(os.environ,GAMEPKG_HOME=str(PROJECT/'target/cube2-package-test/library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',OMP_THREAD_LIMIT='1',GAMEPKG_RENDERER='vulkan',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    env.pop('WAYLAND_DISPLAY',None)
    server=start_display(env,1500,1100);app=None
    def command(*args):return subprocess.run(list(map(str,args)),env=env,check=True,capture_output=True,timeout=30)
    try:
        app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui'),'--play','com.gamepkg.cube2'],env=env,stdout=(proof/'player.log').open('wb'),stderr=subprocess.STDOUT)
        window,_=game_window(env,'com.gamepkg.cube2',timeout=30)
        command('xdotool','windowfocus',window)
        wait_game_frame(window,env,proof,timeout=30);time.sleep(1)
        command('import','-display',env['DISPLAY'],'-window',window,proof/'original-menu.png')
        text=command('tesseract',proof/'original-menu.png','stdout','--psm','11').stdout.decode().casefold()
        assert 'campaign' in text and ('server browser' in text or 'bot match' in text),text
        cube2_choose_test_map(window,env)
        command('import','-display',env['DISPLAY'],'-window',window,proof/'chosen-map.png')
        command('xdotool','key','slash');time.sleep(.3)
        command('xdotool','type','--clearmodifiers','--delay','20','name (concatword $consize _ $miniconsize _ $maxfps)')
        command('xdotool','key','Return');time.sleep(.5)
        # These controls were absent or collapsed by egui's normalized keys.
        # Send actual physical keys without xdotool adding helper modifiers to
        # produce a keysym (notably Control_R and keypad numbers).
        from Xlib import X, XK, display as xdisplay
        from Xlib.ext import xtest
        connection=xdisplay.Display(env['DISPLAY'])
        try:
            for keysym in ['Pause','KP_End','Control_R']:
                code=connection.keysym_to_keycode(XK.string_to_keysym(keysym))
                assert code,keysym
                xtest.fake_input(connection,X.KeyPress,code);connection.sync();time.sleep(.1)
                xtest.fake_input(connection,X.KeyRelease,code);connection.sync();time.sleep(.3)
        finally:connection.close()

        close_game(window,env);assert app.wait(timeout=15)==0
        saved=save.read_bytes()
        names=re.findall(rb'name [^\n]*',saved)
        assert b'7_3_144_p_k_r' in saved, f'Physical bindings or ordered console text failed: {names!r}'
        print('Original Cube 2 startup/menu/console preferences, physical Pause/keypad/right-modifier bindings and native close pass.')
    finally:
        if app and app.poll() is None:
            command('import','-display',env['DISPLAY'],'-window','root',proof/'failure.png')
        if app and app.poll() is None:app.terminate();app.wait(timeout=10)
        save.write_bytes(original_save)
        server.terminate();server.wait(timeout=10)
