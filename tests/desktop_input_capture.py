"""Maintainer test for the vendored host hook using the app's exact dependencies.

No Python dependency is introduced in the application or game workers.
"""
import json
import os
import sys
from pathlib import Path
import subprocess
from cargo_artifacts import dependency_search_args

project = Path(__file__).resolve().parents[1]
build = subprocess.run(
    ['cargo', 'build', '--locked', '--all-features', '--lib', '--message-format=json'],
    cwd=project, check=True, stdout=subprocess.PIPE, text=True,
)
libraries = {}
for line in build.stdout.splitlines():
    message = json.loads(line)
    if message.get('reason') == 'compiler-artifact':
        name = message['target']['name']
        if name in ('egui', 'winit', 'raw_window_handle', 'x11_dl'):
            libraries[name] = next(Path(p) for p in message['filenames'] if p.endswith('.rlib'))
assert set(libraries) == {'egui', 'winit', 'raw_window_handle', 'x11_dl'}, 'Missing desktop dependency artifacts'
output = libraries['egui'].parent / 'gamepkg-desktop-input-hook-test'
subprocess.run([
    'rustc', '--edition=2021', '--test',
    str(project / 'vendor/egui-winit/src/gamepkg_input.rs'),
    '--extern', f'egui={libraries["egui"]}',
    '--extern', f'winit={libraries["winit"]}',
    '--extern', f'raw_window_handle={libraries["raw_window_handle"]}',
    '--extern', f'x11_dl={libraries["x11_dl"]}',
    '--cfg', 'feature="x11"',
    *dependency_search_args(build.stdout), '-o', str(output),
], cwd=project, check=True)
subprocess.run([str(output)], cwd=project, check=True)

if '--x11-locks' in sys.argv:
    from gui_test_support import start_display
    env = dict(os.environ)
    env.pop('WAYLAND_DISPLAY', None)
    display = start_display(env)
    try:
        subprocess.run([str(output), '--ignored', 'x11_lock_state', '--nocapture'], env=env, cwd=project, check=True)
    finally:
        display.terminate()
        display.wait(timeout=10)
