"""Maintainer test for the vendored host hook using the app's exact dependencies.

No Python dependency is introduced in the application or game workers.
"""
import json
from pathlib import Path
import subprocess

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
        if name in ('egui', 'winit'):
            libraries[name] = next(Path(p) for p in message['filenames'] if p.endswith('.rlib'))
assert set(libraries) == {'egui', 'winit'}, 'Missing desktop dependency artifacts'
output = libraries['egui'].parent / 'gamepkg-desktop-input-hook-test'
subprocess.run([
    'rustc', '--edition=2021', '--test',
    str(project / 'vendor/egui-winit/src/gamepkg_input.rs'),
    '--extern', f'egui={libraries["egui"]}',
    '--extern', f'winit={libraries["winit"]}',
    '-L', f'dependency={libraries["egui"].parent}', '-o', str(output),
], cwd=project, check=True)
subprocess.run([str(output)], cwd=project, check=True)
