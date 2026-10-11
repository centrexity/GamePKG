"""Exercise GitHub project catalogs and signed updates with deterministic release fixtures.
The curl fixture runs only in the test child processes; no GitHub account is required.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
CLI = Path(os.environ.get('GAMEPKG_TEST_CLI', PROJECT / 'target/release/gamepkg'))
URL = 'https://github.com/creator/game'
with tempfile.TemporaryDirectory(prefix='gamepkg-github-') as directory:
    root = Path(directory)
    tools = root / 'tools'
    tools.mkdir()
    curl = tools / 'curl'
    curl.write_text('''#!/usr/bin/env python3
import os, pathlib, sys
args = sys.argv[1:]
url = args[args.index('--url') + 1]
out = pathlib.Path(args[args.index('--output') + 1])
root = pathlib.Path(os.environ['GITHUB_FIXTURE'])
if url == 'https://api.github.com/repos/creator/game/releases/latest':
    out.write_bytes((root / 'release.json').read_bytes())
elif url == 'https://github.com/creator/game/releases/download/v1/game.gamepkg':
    out.write_bytes((root / 'latest.gamepkg').read_bytes())
else:
    sys.exit(23)
print('200', end='')
''')
    curl.chmod(0o755)
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'),
               GAMEPKG_CONFIG=str(root / 'settings.toml'), GITHUB_FIXTURE=str(root),
               PATH=str(tools) + os.pathsep + os.environ['PATH'])
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/minimal', source)
    original = (source / 'manifest.toml').read_text()
    key = root / 'creator.key'
    key.write_text('05' * 32)
    other = root / 'other.key'
    other.write_text('06' * 32)

    def run(*args, success=True):
        result = subprocess.run([str(CLI), *map(str, args)], env=env,
                                capture_output=True, text=True, timeout=30)
        assert (result.returncode == 0) == success, result.stderr
        return result

    def publish(version, signing_key=key, package_id='com.gamepkg.minimal'):
        (source / 'manifest.toml').write_text(
            f'update_url = "{URL}"\n' + original.replace(
                'version = "0.1.0"', f'version = "{version}"').replace(
                'com.gamepkg.minimal', package_id))
        archive = root / 'latest.gamepkg'
        archive.unlink(missing_ok=True)
        run('pack', source, '-o', archive, '--key', signing_key)
        release = {'draft': False, 'assets': [{
            'name': 'game.gamepkg', 'size': archive.stat().st_size,
            'browser_download_url': URL + '/releases/download/v1/game.gamepkg'}]}
        (root / 'release.json').write_text(json.dumps(release))
        return release

    publish('0.1.0')
    run('install', URL, success=False)
    catalog = root / 'games.gamepkglist'
    catalog.write_text(f'format_version=1\n[[games]]\nid="com.gamepkg.minimal"\ntitle="GitHub game"\ndownload_url="{URL}"\n')
    run('import-list', catalog)
    assert 'Not downloaded' in run('list').stdout
    assert not list((root / 'library').glob('*.gamepkg'))
    run('download', 'com.gamepkg.minimal', '--trust-creator')
    assert '0.1.0' in run('list').stdout
    publish('0.2.0')
    assert 'Update available' in run('check-updates', 'com.gamepkg.minimal').stdout
    assert '0.1.0' in run('list').stdout
    run('update', 'com.gamepkg.minimal')
    assert '0.2.0' in run('list').stdout
    assert 'up to date' in run('check-updates', 'com.gamepkg.minimal').stdout
    publish('9.0.0', other)
    run('update', 'com.gamepkg.minimal', success=False)
    publish('9.0.0', package_id='com.gamepkg.other')
    run('update', 'com.gamepkg.minimal', success=False)
    release = publish('0.3.0')
    release['assets'].append(dict(release['assets'][0], name='other.gamepkg'))
    (root / 'release.json').write_text(json.dumps(release))
    run('check-updates', 'com.gamepkg.minimal', success=False)
    assert '0.2.0' in run('list').stdout
    print('GitHub catalogs, creator approval, update discovery/install, and signer/ID/ambiguous asset rejection passed.')
