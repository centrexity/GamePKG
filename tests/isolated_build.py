"""Linux build smoke test; needs bubblewrap, timeout, Rust, and the Wasm target."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-build-') as directory:
    root = Path(directory)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/minimal', source)
    key = root / 'creator.key'
    key.write_text('05' * 32)
    archive = root / 'demo.gamepkg'
    library = root / 'library'
    config = root / "settings.toml"
    config.write_text("automatic_update_checks = false\n")
    env = dict(os.environ, GAMEPKG_HOME=str(library), GAMEPKG_CONFIG=str(config))
    def run(*args):
        p = subprocess.run([str(CLI), *map(str, args)], env=env, capture_output=True, text=True, timeout=75)
        assert p.returncode == 0, p.stderr
        return p
    run('pack', source, '-o', archive, '--key', key)
    run('install', archive, '--trust-creator')
    run('build', 'com.gamepkg.minimal')
    artifacts = list((library / 'builds').glob('*/game.wasm'))
    assert len(artifacts) == 1
    assert artifacts[0].read_bytes().startswith(b'\0asm\x01\0\0\0')
    run('build', 'com.gamepkg.minimal')
    assert len(list((library / 'builds').glob('*/game.wasm'))) == 1
    print('Isolated Wasm build and rebuild passed.')
