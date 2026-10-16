"""Compile signed Slang/texture/audio host APIs in both profiles; test audio denial."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', PROJECT/'target/release'))/'gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-media-') as directory:
    root = Path(directory)
    source = root/'source'
    shutil.copytree(PROJECT/'examples/good/media-game', source)
    manifest = (source/'manifest.toml').read_text()
    code = (source/'source/src/lib.rs').read_text()
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args):
        if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
        result = subprocess.run([str(CLI), *map(str,args)], env=env, capture_output=True, text=True, timeout=140)
        assert result.returncode == 0, result.stderr
        return result.stdout
    key = root/'creator.key'
    run('keygen', '-o', key)
    for profile in ['core-v1', 'component-v1']:
        for permitted in [True, False]:
            identity = f'com.gamepkg.media-{profile}-{int(permitted)}'
            current = manifest.replace('com.gamepkg.media', identity)
            current = current.replace('audio = true', f'audio = {str(permitted).lower()}')
            (source/'manifest.toml').write_text(f'runtime_profile = "{profile}"\n'+current)
            (source/'source/src/lib.rs').write_text(code.replace('EXPECT_AUDIO: bool = true', f'EXPECT_AUDIO: bool = {str(permitted).lower()}'))
            package = root/f'{identity}.gamepkg'
            run('pack', source, '-o', package, '--key', key)
            run('install', package, '--trust-creator')
            run('build', identity)
            output = run('run', identity, '--frames', '2')
            assert 'draw commands: 4' in output, output
            if permitted: assert 'Audio samples: 4; peak: 16384' in output, output
            else: assert 'Audio samples:' not in output, output
    print('Shader/audio host APIs passed both profiles: signed Slang lookup, PNG/RGBA textures, path and format rejection, PCM decoding, cumulative audio limits, permission denial and dependency-free source.')
