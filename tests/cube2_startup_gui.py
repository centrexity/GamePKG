"""Original menu and preferences: use the private cached engine fixture in place."""
from pathlib import Path
import os,subprocess,tempfile,time,struct,re,argparse
from gui_test_support import start_display,game_window,wait_game_frame,close_game,cube2_choose_test_map
parser=argparse.ArgumentParser();parser.add_argument('--renderer',choices=['vulkan','opengl'],default='vulkan');parser.add_argument('--locks-enabled',action='store_true');args=parser.parse_args()
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
 b'bind LCTRL [name wrong_modifier_alias]\n'
 b'keymap 0x40000102 TRACKNEXT\nbind TRACKNEXT [onrelease [maxfps 145]; name (concatword (getname) _n)]\n')
name=b'config.cfg'
payload=struct.pack('<IIII',len(name),len(config_bytes),0,0)+name+config_bytes
save.write_bytes(b'GPS2'+struct.pack('<III',1,len(payload),0)+payload)

with tempfile.TemporaryDirectory(prefix='gamepkg-startup-') as temporary:
    root=Path(temporary);config=root/'settings.toml'
    config.write_text('memory_ceiling_mib=2048\ntick_deadline_ms=500\ncheck_app_updates_on_startup=false\nautomatic_update_checks=false\nmute_audio=true\n')
    env=dict(os.environ,GAMEPKG_HOME=str(PROJECT/'target/cube2-package-test/library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',OMP_THREAD_LIMIT='1',GAMEPKG_RENDERER=args.renderer,DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    env.pop('WAYLAND_DISPLAY',None)
    server=start_display(env,1500,1100);app=None
    def command(*args):return subprocess.run(list(map(str,args)),env=env,check=True,capture_output=True,timeout=30)
    try:
        if args.locks_enabled:
            subprocess.run(['xdotool','key','Caps_Lock','Num_Lock'],env=env,check=True)
            # This private Xvfb fixture uses its default XKB map: Caps/Num LEDs 0/1.
            state=subprocess.check_output(['xset','q'],env=env,text=True)
            leds=re.search(r'LED mask:\s*([0-9a-fA-F]+)',state)
            assert leds and int(leds.group(1),16)&3==3,state
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
        XK.load_keysym_group('xf86')
        connection=xdisplay.Display(env['DISPLAY'])
        try:
            for keysym in ['Pause','KP_End','Control_R','XF86_AudioNext']:
                code=connection.keysym_to_keycode(XK.string_to_keysym(keysym))
                assert code,keysym
                xtest.fake_input(connection,X.KeyPress,code);connection.sync();time.sleep(.1)
                if keysym=='XF86_AudioNext':
                    # The extended key must be released by the same focus-reset
                    # path as ordinary keys, before its physical keyup arrives.
                    time.sleep(.3)
                    screen=connection.screen()
                    other=screen.root.create_window(0,0,50,50,0,screen.root_depth,X.InputOutput,X.CopyFromParent)
                    other.map();other.set_input_focus(X.RevertToParent,X.CurrentTime)
                    connection.sync();time.sleep(.4)
                    connection.create_resource_object('window',int(window)).set_input_focus(X.RevertToParent,X.CurrentTime)
                    connection.sync();time.sleep(.3);other.destroy()
                xtest.fake_input(connection,X.KeyRelease,code);connection.sync();time.sleep(.3)
        finally:connection.close()

        close_game(window,env);assert app.wait(timeout=15)==0
        saved=save.read_bytes()
        names=re.findall(rb'name [^\n]*',saved)
        assert b'7_3_144_p_k_r_n' in saved, f'Physical bindings or ordered console text failed: {names!r}'
        assert b'maxfps 145' in saved,'Extended media key was not released and persisted'
        print('Original Cube 2 startup/menu/console preferences, physical Pause/keypad/right-modifier/media bindings and native close pass.' + (' Caps/Num enabled before launch.' if args.locks_enabled else ''))
    finally:
        if app and app.poll() is None:
            command('import','-display',env['DISPLAY'],'-window','root',proof/'failure.png')
        if app and app.poll() is None:app.terminate();app.wait(timeout=10)
        save.write_bytes(original_save)
        server.terminate();server.wait(timeout=10)
