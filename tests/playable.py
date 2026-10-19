"""End-to-end signed source -> isolated compiler -> Wasmtime gameplay.
Requires Linux bubblewrap, Rust Wasm target, and built release binaries.
"""
import os
from pathlib import Path
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-play-') as directory:
    root = Path(directory)
    key = root / 'creator.key'
    key.write_text(os.urandom(32).hex())
    package = root / 'stars.gamepkg'
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'))
    def run(*args, input=None):
        if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
        result = subprocess.run([str(CLI), *map(str,args)], env=env, input=input,
                                capture_output=True, text=True, timeout=75)
        assert result.returncode == 0, result.stderr
        return result.stdout
    run('pack', PROJECT / 'examples/good/collect-stars', '-o', package, '--key', key)
    run('install', package, '--trust-creator')
    run('build', 'com.gamepkg.collect-stars')
    assert 'Ran 60 frames' in run('run', 'com.gamepkg.collect-stars', '--frames', '60')
    output = run('run', 'com.gamepkg.collect-stars', input='a\n' * 6 + 'w\n' * 6 + 'q\n')
    assert 'Score 1' in output, output
    print('Playable game passed: signed package, isolated build, Wasmtime execution, movement and target collection.')
