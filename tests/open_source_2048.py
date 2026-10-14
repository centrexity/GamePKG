"""Verify the adapted open-source game through the actual isolated build and runtime."""
import os
from pathlib import Path
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-2048-') as directory:
    root = Path(directory)
    key = root / 'creator.key'
    key.write_text('35' * 32)
    config = root / 'settings.toml'
    config.write_text('automatic_update_checks = false\n')
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'), GAMEPKG_CONFIG=str(config))
    def run(*args, input=None):
        result = subprocess.run([str(CLI), *map(str,args)], env=env, input=input,
                                capture_output=True, text=True, timeout=120)
        assert result.returncode == 0, result.stderr
        return result.stdout
    archive = root / '2048.gamepkg'
    run('pack', PROJECT / 'ports/2048', '-o', archive, '--key', key)
    run('install', archive, '--trust-creator')
    run('build', 'com.gamepkg.2048')
    assert 'Ran 60 frames' in run('run', 'com.gamepkg.2048', '--frames', '60')
    run('integrity-check', 'com.gamepkg.2048')
    print('Open-source 2048: signed install, isolated compilation, gameplay and integrity passed.')
