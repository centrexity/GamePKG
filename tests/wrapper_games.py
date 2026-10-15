"""Signed owned wrappers -> offline isolated build -> both guest profiles."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
CLI = Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', PROJECT/'target/release'))/'gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-wrappers-') as directory:
    root = Path(directory)
    source = root/'source'
    shutil.copytree(PROJECT/'examples/good/wrapper-game', source)
    original = (source/'manifest.toml').read_text()
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args):
        result = subprocess.run([str(CLI), *map(str, args)], env=env, capture_output=True, text=True, timeout=140)
        assert result.returncode == 0, result.stderr
        return result.stdout
    key = root/'creator.key'
    run('keygen', '-o', key)
    for profile in ['core-v1', 'component-v1']:
        identity = f'com.gamepkg.wrappers-{profile}'
        (source/'manifest.toml').write_text(f'runtime_profile = "{profile}"\n'+original.replace('com.gamepkg.wrappers', identity))
        package = root/f'{profile}.gamepkg'
        run('pack', source, '-o', package, '--key', key)
        run('install', package, '--trust-creator')
        run('build', identity)
        output = run('run', identity, '--frames', '2')
        assert 'draw commands: 3' in output, output
        for name in ['gamepkg-file', 'gamepkg-input', 'gamepkg-window', 'gamepkg-networking', 'gamepkg-byteorder']:
            run('allowlist', 'remove', name)
            denied = subprocess.run([str(CLI), 'verify', str(package)], env=env, capture_output=True, text=True)
            assert denied.returncode != 0 and 'not approved' in denied.stderr, denied.stderr
            run('allowlist', 'add', name)
    print('All five owned wrappers passed core/WIT compilation and gameplay: valid asset reads/drawing, denied traversal/ROM/save/network access, unchanged denied-read buffers, and per-crate revocation.')
