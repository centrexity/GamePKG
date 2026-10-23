"""Original VSync controls and real Vulkan/OpenGL presentation across restarts.

Private Linux/glibc maintainer test; no recordings, user library or source edits.
The forwarding observer records actual backend API calls without replacing them.
"""
from pathlib import Path
import json, os, re, shutil, subprocess, tempfile, time
from PIL import Image
from gui_test_support import start_display

PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT/'target/release/gamepkg'
GUI = PROJECT/'target/release/gamepkg-gui'

with tempfile.TemporaryDirectory(prefix='gamepkg-arkanoo-vsync-') as temporary:
    root = Path(temporary)
    config = root/'settings.toml'
    config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(config),
               LIBGL_ALWAYS_SOFTWARE='1', DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    env.pop('WAYLAND_DISPLAY', None)
    key = root/'key'
    key.write_text('29'*32)

    def run(*args):
        value = subprocess.run(list(map(str, args)), env=env, capture_output=True, text=True, timeout=180)
        assert value.returncode == 0, (args, value.stderr)
        return value.stdout

    observer = root/'presentation-trace.so'
    run('cc', '-std=c11', '-shared', '-fPIC', '-Wall', '-Wextra', '-Werror',
        PROJECT/'tests/presentation_trace.c', '-ldl', '-o', observer)
    for profile in ['core-v1', 'component-v1']:
        source = root/profile
        shutil.copytree(PROJECT/'ports/arkanoo', source)
        manifest = source/'manifest.toml'
        manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.arkanoo', 'com.gamepkg.vsync-'+profile))
        package = root/(profile+'.gamepkg')
        run(CLI, 'pack', source, '-o', package, '--key', key)
        run(CLI, 'install', package, '--trust-creator')
        run(CLI, 'build', 'com.gamepkg.vsync-'+profile, '--target', 'wasm')
    run(CLI, 'build', 'com.gamepkg.vsync-core-v1', '--target', 'native')

    display = start_display(env, width=1500, height=1100)
    wm = app = None
    trace = root/'presentation.log'

    def wait(check, message, seconds=20):
        end = time.monotonic()+seconds
        while time.monotonic() < end:
            if check():
                return
            if app is not None and app.poll() is not None:
                raise AssertionError((message, app.communicate()))
            time.sleep(.05)
        if (root/'screen.png').exists():
            shutil.copy2(root/'screen.png', '/tmp/gamepkg-vsync-failure.png')
        if (root/'label.png').exists():
            shutil.copy2(root/'label.png', '/tmp/gamepkg-vsync-label-failure.png')
            print('Last button OCR:', run('tesseract', root/'label.png', 'stdout', '--psm', '7'), flush=True)
        raise AssertionError((message, trace.read_text() if trace.exists() else 'no backend calls'))

    def geometry():
        info = run('xwininfo', '-id', window)
        origin = [int(re.search('Absolute upper-left '+axis+r':\s*(-?\d+)', info).group(1)) for axis in ['X', 'Y']]
        size = [int(re.search(r'^\s*'+name+r':\s*(\d+)', info, re.M).group(1)) for name in ['Width', 'Height']]
        return *origin, *size

    def click(x, y):
        _, _, width, height = geometry()
        scale = min(width/640, height/480)
        run('xdotool', 'mousemove', '--window', window, str(round((width-640*scale)/2+x*scale)), str(round((height-480*scale)/2+y*scale)))
        run('xdotool', 'click', '1')

    def label_at(y):
        image = root/'screen.png'
        run('import', '-display', env['DISPLAY'], '-window', 'root', image)
        x, top, width, height = geometry()
        scale = min(width/640, height/480)
        left = x+(width-640*scale)/2+271*scale
        top += (height-480*scale)/2+(y+1)*scale
        label = root/'label.png'
        with Image.open(image) as picture:
            crop = picture.crop((round(left), round(top), round(left+98*scale), round(top+18*scale)))
            crop = crop.convert('L').point(lambda value: 0 if value > 180 else 255)
            crop.resize((crop.width*4, crop.height*4)).save(label)
        return re.sub(r'\s+', '', run('tesseract', label, 'stdout', '--psm', '7').lower())

    def presentation(expected):
        rows = [line.split() for line in trace.read_text().splitlines()] if trace.exists() else []
        if renderer == 'vulkan':
            rows = [row for row in rows if row[0] == 'vulkan']
            # Fifo=2; the private driver supports non-vsync Immediate=0/Mailbox=1.
            return bool(rows) and all(int(row[2]) == 0 and (int(row[1]) == 2 if expected else int(row[1]) in [0, 1]) for row in rows)
        rows = [row for row in rows if row[0].startswith(('egl', 'glx-'))]
        return bool(rows) and all(int(row[1]) == int(expected) and int(row[2]) == (1 if row[0] == 'egl' else 0) for row in rows)

    def vsync_label(expected):
        prefix, separator, value = label_at(285).partition(':')
        # OCR reads the original Roboto 'c' as 'e' at this capture size. The
        # ON/OFF state remains exact; complete SDL frame tests check every glyph.
        return separator == ':' and prefix in ['vsync', 'vsyne'] and value == ('on' if expected else 'off')

    def launch(expected, library=False):
        global app, window
        trace.write_text('')
        traced_env = dict(env, LD_PRELOAD=str(observer), GAMEPKG_PRESENTATION_TRACE=str(trace))
        args = [GUI, '--renderer', renderer]
        if not library:
            args += ['--play', game_id, '--target', target]
        app = subprocess.Popen(list(map(str, args)), env=traced_env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        title = 'GamePKG' if library else game_id
        def find():
            global window
            value = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^'+re.escape(title)+'$'], env=env, capture_output=True, text=True, timeout=5)
            if value.returncode == 0:
                window = value.stdout.splitlines()[0]
                return True
            return False
        wait(find, 'own visible window')
        run('xdotool', 'windowactivate', '--sync', window)
        wait(lambda: presentation(expected), 'real presentation configuration')
        if not library:
            run('xdotool', 'key', '--window', window, 'Return')
            wait(lambda: 'settings' in label_at(277.5), 'original main menu')
            click(320, 287.5)
            wait(lambda: 'windowed' in label_at(260), 'original Settings page')
            wait(lambda: vsync_label(expected), 'saved VSync button state')

    def close():
        global app
        run('xdotool', 'key', 'alt+F4')
        assert app.wait(timeout=15) == 0, app.communicate()
        app.communicate()
        app = None

    try:
        wm = subprocess.Popen(['metacity', '--sm-disable', '--replace', '--no-composite'], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        wait(lambda: 'window id #' in run('xprop', '-root', '_NET_SUPPORTING_WM_CHECK'), 'private window manager')
        for renderer in ['opengl', 'vulkan']:
            for profile, target in [('core-v1', 'native'), ('core-v1', 'wasm'), ('component-v1', 'wasm')]:
                game_id = 'com.gamepkg.vsync-'+profile
                launch(True)
                click(320, 295)
                wait(lambda: vsync_label(False), 'original VSync toggle off')
                assert presentation(True), 'VSync changed before the required restart'
                close()
                records = [path for path in (root/'library/window-preferences').glob('*.json') if json.loads(path.read_text())['vsync'] is False]
                assert len(records) == 1, records
                record = records[0]
                if target == 'native':
                    launch(True, library=True)
                    close()
                    assert json.loads(record.read_text())['vsync'] is False, 'library changed game preference'
                launch(False)
                click(320, 295)
                wait(lambda: vsync_label(True), 'original VSync toggle on')
                assert presentation(False), 'VSync changed before restart'
                close()
                assert json.loads(record.read_text())['vsync'] is True
                launch(True)
                close()
                print(renderer, profile, target, ': real backend ON/OFF/ON, original control, next-launch persistence and library isolation passed', flush=True)
    finally:
        if app is not None and app.poll() is None:
            app.terminate()
            app.communicate(timeout=15)
        if wm is not None:
            wm.terminate()
            wm.wait(timeout=10)
        display.terminate()
        display.wait(timeout=10)
