"""Exercise the canonical Settings resolution controls in real private windows.

No game source or input-handler substitutions. Players need no Python.
"""
from pathlib import Path
import os, re, shutil, subprocess, tempfile, time
from PIL import Image
from gui_test_support import start_display

PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
GUI = PROJECT / 'target/release/gamepkg-gui'

with tempfile.TemporaryDirectory(prefix='gamepkg-arkanoo-resolution-') as temporary:
    root = Path(temporary)
    config = root / 'settings.toml'
    config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(config),
               LIBGL_ALWAYS_SOFTWARE='1', DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    env.pop('WAYLAND_DISPLAY', None)
    key = root / 'key'
    key.write_text('27'*32)

    def run(*args):
        result = subprocess.run(list(map(str, args)), env=env, capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, (args, result.stderr)
        return result.stdout

    for profile in ['core-v1', 'component-v1']:
        source = root / profile
        shutil.copytree(PROJECT/'ports/arkanoo', source)
        manifest = source/'manifest.toml'
        manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.arkanoo', 'com.gamepkg.resolution-'+profile))
        package = root/(profile+'.gamepkg')
        run(CLI, 'pack', source, '-o', package, '--key', key)
        run(CLI, 'install', package, '--trust-creator')
        run(CLI, 'build', 'com.gamepkg.resolution-'+profile, '--target', 'wasm')
    run(CLI, 'build', 'com.gamepkg.resolution-core-v1', '--target', 'native')

    display = start_display(env, width=1500, height=1100)
    wm = app = None

    def wait(check, message, seconds=20):
        end = time.monotonic()+seconds
        while time.monotonic() < end:
            if check():
                return
            if app is not None and app.poll() is not None:
                raise AssertionError((message, app.communicate()))
            time.sleep(.05)
        if (root/'settings-screen.png').exists():
            shutil.copy2(root/'settings-screen.png', '/tmp/gamepkg-resolution-readiness-failure.png')
            print(run('tesseract', root/'settings-screen.png', 'stdout', '--psm', '11'), flush=True)
        raise AssertionError(message)

    try:
        wm = subprocess.Popen(['metacity', '--sm-disable', '--replace', '--no-composite'], env=env,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        wait(lambda: 'window id #' in run('xprop', '-root', '_NET_SUPPORTING_WM_CHECK'), 'private window manager')
        for profile, target in [('core-v1', 'native'), ('core-v1', 'wasm'), ('component-v1', 'wasm')]:
            game_id = 'com.gamepkg.resolution-'+profile
            window = None

            def find():
                global window
                result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^'+re.escape(game_id)+'$'],
                                        env=env, capture_output=True, text=True, timeout=5)
                if result.returncode == 0:
                    window = result.stdout.splitlines()[0]
                    return True
                return False

            def dimensions():
                value = run('xwininfo', '-id', window)
                return tuple(int(re.search(r'^\s*'+name+r':\s*(\d+)', value, re.M).group(1)) for name in ['Width', 'Height'])

            def click(x, y):
                width, height = dimensions()
                scale = min(width/640, height/480)
                px = round((width-640*scale)/2+x*scale)
                py = round((height-480*scale)/2+y*scale)
                run('xdotool', 'mousemove', '--window', window, str(px), str(py))
                run('xdotool', 'click', '1')

            def launch():
                global app
                app = subprocess.Popen([GUI, '--play', game_id, '--target', target, '--renderer', 'opengl'],
                                       env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                wait(find, 'separate original game window')
                run('xdotool', 'windowactivate', '--sync', window)
                run('xdotool', 'key', '--window', window, 'Return')
                def screen_text(rect):
                    image = root/'settings-screen.png'
                    run('import', '-display', env['DISPLAY'], '-window', 'root', image)
                    info = run('xwininfo', '-id', window)
                    origin = [int(re.search('Absolute upper-left '+axis+r':\s*(-?\d+)', info).group(1)) for axis in ['X', 'Y']]
                    width, height = dimensions()
                    scale = min(width/640, height/480)
                    x, y, w, h = rect
                    left = origin[0]+(width-640*scale)/2+(x+1)*scale
                    top = origin[1]+(height-480*scale)/2+(y+1)*scale
                    # Recognize the white button label separately from the busy
                    # original background; exclude the button's light border.
                    label = root/'control-label.png'
                    with Image.open(image) as picture:
                        cropped = picture.crop((round(left), round(top), round(left+(w-2)*scale), round(top+(h-2)*scale)))
                        cropped = cropped.convert('L').point(lambda value: 0 if value > 180 else 255)
                        cropped.resize((cropped.width*4, cropped.height*4)).save(label)
                    return re.sub(r'\s+', '', run('tesseract', label, 'stdout', '--psm', '7').lower())
                # Wait for rendered controls, rather than assuming startup speed.
                wait(lambda: 'settings' in screen_text((270, 277.5, 100, 20)), 'original main menu rendered')
                click(320, 287.5)
                wait(lambda: 'windowed' in screen_text((270, 260, 100, 20)), 'original Settings page rendered')

            launch()
            run('xdotool', 'windowsize', window, '1125', '800')
            wait(lambda: dimensions() == (1125, 800), 'starting window size')
            time.sleep(.25)
            click(320, 322.5)  # Original 1280x720 rectangle.
            wait(lambda: dimensions() == (1280, 720), 'original Settings preview')
            click(325, 423.75)  # Original Revert rectangle.
            wait(lambda: dimensions() == (1125, 800), 'original Settings Revert')
            click(320, 340)  # Original 1366x768 rectangle.
            wait(lambda: dimensions() == (1366, 768), 'second original resolution')
            click(265, 423.75)  # Original Keep rectangle.
            time.sleep(5.5)
            assert dimensions() == (1366, 768), 'Keep expired despite confirmation'
            run('xdotool', 'key', 'alt+F4')
            assert app.wait(timeout=15) == 0, app.communicate()
            app.communicate()
            app = None
            launch()
            wait(lambda: dimensions() == (1366, 768), 'Keep persisted after close/reopen')
            click(320, 322.5)
            wait(lambda: dimensions() == (1280, 720), 'unconfirmed Settings preview')
            wait(lambda: dimensions() == (1366, 768), 'host timeout restores confirmed size', seconds=15)
            click(320, 322.5)
            wait(lambda: dimensions() == (1280, 720), 'Settings usable after timeout')
            click(325, 423.75)
            wait(lambda: dimensions() == (1366, 768), 'second Settings Revert')
            run('xdotool', 'key', 'alt+F4')
            assert app.wait(timeout=15) == 0, app.communicate()
            app.communicate()
            app = None
            print(profile, target, ': original resolution selection, Revert, Keep, saved reopen, timeout and reuse passed', flush=True)
    finally:
        if app is not None and app.poll() is None:
            app.terminate()
            app.communicate(timeout=15)
        if wm is not None:
            wm.terminate()
            wm.wait(timeout=10)
        display.terminate()
        display.wait(timeout=10)
