"""Real game-only windows with the signed recorded-effects example and null audio."""
from pathlib import Path
import os, subprocess, tempfile, time
from PIL import Image
from gui_test_support import start_display, game_window
project = Path(__file__).resolve().parents[1]
cli = project/'target/release/gamepkg'; gui = project/'target/release/gamepkg-gui'
with tempfile.TemporaryDirectory(prefix='gamepkg-effects-gui-') as temporary:
    root = Path(temporary)
    (root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
    (root/'null.conf').write_text('pcm.!default { type null }\n')
    (root/'synthetic.key').write_text('69'*32)
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'), ALSA_CONFIG_PATH=str(root/'null.conf'), LIBGL_ALWAYS_SOFTWARE='1')
    env.pop('WAYLAND_DISPLAY', None)
    def run(*arguments):
        result = subprocess.run([str(cli), *map(str, arguments)], env=env, capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, result.stderr
    run('pack', project/'examples/good/recorded-effects', '-o', root/'example.gamepkg', '--key', root/'synthetic.key')
    run('install', root/'example.gamepkg', '--trust-creator')
    id = 'com.gamepkg.recorded-effects'
    run('build', id, '--target', 'wasm'); run('build', id, '--target', 'native')
    display = start_display(env, 1100, 850)
    try:
        for target in ['wasm', 'native']:
            for renderer in ['vulkan', 'opengl']:
                app = subprocess.Popen([str(gui), '--play', id, '--target', target, '--renderer', renderer], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                held = False
                try:
                    window, _ = game_window(env, id)
                    subprocess.run(['xdotool', 'windowfocus', window], env=env, check=True)
                    def await_color(expected):
                        end = time.monotonic()+10
                        while True:
                            assert app.poll() is None, 'Game exited before drawing'
                            path = root/'frame.png'
                            subprocess.run(['import', '-display', env['DISPLAY'], '-window', window, str(path)], env=env, check=True)
                            image = Image.open(path).convert('RGB')
                            scale = min(image.width/640, image.height/480)
                            ox = (image.width-640*scale)/2; oy = (image.height-480*scale)/2
                            actual = image.getpixel((round(ox+390*scale), round(oy+240*scale)))
                            if actual == expected: return
                            assert time.monotonic() < end, (target, renderer, actual, expected)
                            time.sleep(.05)
                    await_color((16,38,56))
                    subprocess.run(['xdotool', 'keydown', 'Return'], env=env, check=True); held = True
                    await_color((98,221,173))
                    subprocess.run(['xdotool', 'keyup', 'Return'], env=env, check=True); held = False
                    await_color((16,38,56))
                    print(target, renderer, 'recorded-effects game window: launch, held input and continued rendering passed', flush=True)
                finally:
                    if held: subprocess.run(['xdotool', 'keyup', 'Return'], env=env, check=True)
                    if app.poll() is None: app.terminate()
                    out, err = app.communicate(timeout=10)
                    assert b'Sound unavailable' not in err and b'Unable to prepare recorded effects' not in err, err.decode()
    finally:
        display.terminate(); display.wait(timeout=10)
