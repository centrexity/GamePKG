"""Run the actual SDL/X11 extra-button reference on a private virtual display."""
from pathlib import Path
import os, shlex, subprocess, tempfile
from gui_test_support import start_display

project = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-mouse-') as temporary:
    binary = Path(temporary)/'mouse-reference'
    flags = shlex.split(subprocess.check_output(['pkg-config', '--cflags', '--libs', 'sdl2', 'x11', 'xtst'], text=True))
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', str(project/'tests/doom_extra_mouse.c'), '-o', str(binary), *flags], check=True)
    env = dict(os.environ, SDL_VIDEODRIVER='x11')
    env.pop('WAYLAND_DISPLAY', None)
    display = start_display(env)
    try:
        subprocess.run([str(binary)], env=env, check=True, timeout=15)
    finally:
        display.terminate()
        display.wait(timeout=10)
