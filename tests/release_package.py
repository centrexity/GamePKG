"""Verify the actual extracted distribution, including isolated build and desktop play."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time
import tomllib
from gui_test_support import start_display

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('archive', type=Path)
args = parser.parse_args()
archive = args.archive.resolve()
expected = archive.with_suffix(archive.suffix + '.sha256').read_text().split()[0]
assert hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest() == expected
with tempfile.TemporaryDirectory(prefix='gamepkg-release-test-') as directory:
    root = Path(directory)
    with tarfile.open(archive) as package:
        assert all(not member.issym() and not member.islnk() for member in package.getmembers())
        package.extractall(root / 'distribution', filter='data')
    distribution, = (root / 'distribution').iterdir()
    for port in ['2048', 'arkanoo', 'zelda3']:
        assert (distribution / 'ports' / port / 'manifest.toml').is_file()
    assert not list((distribution / 'ports').rglob('.git'))
    build = json.loads((distribution / 'BUILD.json').read_text())
    for name, digest in build['executables_sha256'].items():
        with (distribution / 'bin' / name).open('rb') as stream:
            assert hashlib.file_digest(stream, 'sha256').hexdigest() == digest
    dependencies = json.loads((distribution / 'DEPENDENCIES.json').read_text())
    assert dependencies
    assert all((distribution / 'third-party-sources' / f"{p['name']}-{p['version']}" / 'Cargo.toml').is_file() for p in dependencies)
    assert (distribution / 'LICENSE').is_file() and (distribution / 'NOTICE').is_file()
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'), GAMEPKG_CONFIG=str(root / 'settings.toml'), LIBGL_ALWAYS_SOFTWARE='1')
    env.pop('WAYLAND_DISPLAY', None)
    (root / 'settings.toml').write_text('automatic_update_checks = false\n')
    cli = distribution / 'bin' / 'gamepkg'
    def run(*arguments):
        result = subprocess.run([str(cli), *map(str, arguments)], cwd=distribution, env=env, capture_output=True, text=True, timeout=140)
        assert result.returncode == 0, result.stderr
        return result.stdout
    assert build['version'] in run('--version')
    gui_version = subprocess.run([str(distribution / 'bin' / 'gamepkg-gui'),'--version'],env=env,capture_output=True,text=True,timeout=10)
    assert gui_version.returncode == 0 and build['version'] in gui_version.stdout, gui_version.stderr
    assert 'web-library' in run('--help')
    assert '--store' in run('integrity-fix','--help')
    run('keygen', '-o', root / 'creator.key')
    run('pack', 'examples/good/byteorder-game', '-o', root / 'game.gamepkg', '--key', root / 'creator.key')
    run('install', root / 'game.gamepkg', '--trust-creator')
    game_id = tomllib.loads((distribution / 'examples/good/byteorder-game/manifest.toml').read_text())['id']
    run('build', game_id)
    run('run', game_id, '--frames', '3')
    assert 'compiled game verified' in run('integrity-check', game_id)
    display = start_display(env)
    app = subprocess.Popen([str(distribution / 'bin' / 'gamepkg-gui'), '--play', game_id], cwd=distribution, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        for _ in range(100):
            if app.poll() is not None:
                raise RuntimeError(app.stderr.read().decode())
            window = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^GamePKG$'], env=env, capture_output=True, text=True)
            if window.returncode == 0:
                break
            time.sleep(.1)
        else:
            raise AssertionError('extracted desktop did not open')
        time.sleep(.5)
        screenshot = root / 'play.png'
        subprocess.run(['import', '-display', env['DISPLAY'], '-window', window.stdout.splitlines()[0], str(screenshot)], env=env, check=True)
        text = subprocess.check_output(['tesseract', str(screenshot), 'stdout'], stderr=subprocess.DEVNULL, text=True)
        assert 'Score:' in text and 'Game stopped' not in text, text
    finally:
        app.terminate(); app.wait(timeout=10)
        display.terminate(); display.wait(timeout=10)
print('Distribution passed: checksums, dependency sources, extracted CLI, bundled dependency game build/play, and actual desktop gameplay.')
