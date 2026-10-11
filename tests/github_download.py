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
elif url == 'https://github.com/creator/game/releases/download/v1/games.gamepkglist':
    out.write_bytes((root / 'games.gamepkglist').read_bytes())
elif url == 'https://github.com/creator/game/releases/download/v1/games.gamepkgstore':
    out.write_bytes((root / 'games.gamepkgstore').read_bytes())
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
    release = json.loads((root / 'release.json').read_text())
    release['assets'].append({'name': 'games.gamepkglist', 'size': catalog.stat().st_size,
                              'browser_download_url': URL + '/releases/download/v1/games.gamepkglist'})
    (root / 'release.json').write_text(json.dumps(release))
    run('import-list', URL)
    assert 'Not downloaded' in run('list').stdout
    assert not list((root / 'library').glob('*.gamepkg'))
    bundle = root / 'games.gamepkgstore'
    run('store-export', root / 'latest.gamepkg', '-o', bundle)
    release['assets'].append({'name': 'games.gamepkgstore', 'size': bundle.stat().st_size,
                              'browser_download_url': URL + '/releases/download/v1/games.gamepkgstore'})
    (root / 'release.json').write_text(json.dumps(release))
    assert 'com.gamepkg.minimal' in run('store-inspect', URL).stdout
    library = env['GAMEPKG_HOME']
    env['GAMEPKG_HOME'] = str(root / 'store-library')
    run('store-import', URL, success=False)
    run('store-import', URL, '--trust-creators')
    assert 'com.gamepkg.minimal' in run('list').stdout
    env['GAMEPKG_HOME'] = library
    run('download', 'com.gamepkg.minimal', '--trust-creator')
    assert '0.1.0' in run('list').stdout
    (Path(library) / 'unrelated-broken.gamepkg').write_bytes(b'damaged unrelated package')
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

    if '--gui' in __import__('sys').argv:
        import csv
        import io
        import time
        from gui_test_support import start_display
        publish('0.1.0')
        shutil.rmtree(root / 'library')
        (root / 'settings.toml').write_text('automatic_update_checks = false\n')
        env['LIBGL_ALWAYS_SOFTWARE'] = '1'
        env.pop('WAYLAND_DISPLAY', None)
        display = start_display(env, 1600, 1100)
        app = subprocess.Popen([str(PROJECT / 'target/release/gamepkg-gui')],
                               env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            for _ in range(100):
                result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^GamePKG$'],
                                        env=env, capture_output=True, text=True)
                if result.returncode == 0:
                    window = result.stdout.splitlines()[0]
                    break
                if app.poll() is not None:
                    raise RuntimeError(app.stderr.read().decode())
                time.sleep(.1)
            else:
                raise AssertionError('GUI did not open')
            subprocess.run(['xdotool', 'windowsize', window, '1500', '1050'], env=env, check=True)
            time.sleep(.5)

            def screen():
                screenshot = PROJECT / 'target/github-download-gui.png'
                subprocess.run(['import', '-display', env['DISPLAY'], '-window', window, str(screenshot)],
                               env=env, check=True)
                tsv = subprocess.run(['tesseract', str(screenshot), 'stdout', '--psm', '6', 'tsv'],
                                     capture_output=True, text=True, check=True).stdout
                words = [w for w in csv.DictReader(io.StringIO(tsv), delimiter='\t') if w['text'].strip()]
                return ' '.join(w['text'] for w in words), words

            def click(word, dy=0):
                subprocess.run(['xdotool', 'mousemove', '--window', window,
                                str(int(word['left']) + int(word['width']) // 2),
                                str(int(word['top']) + int(word['height']) // 2 + dy), 'click', '1'],
                               env=env, check=True)

            text, words = screen()
            label = next(w for w in words if w['text'] == 'Local')
            subprocess.run(['xdotool', 'windowfocus', '--sync', window], env=env, check=True)
            click(label, 20)
            subprocess.run(['xdotool', 'type', '--clearmodifiers', '--delay', '20', URL], env=env, check=True)
            time.sleep(.2)
            text, words = screen()
            assert 'github.com/creator/' in text, 'URL was not entered: ' + text
            click(next(w for w in words if w['text'] == 'Download'))
            for _ in range(30):
                text, words = screen()
                if 'Trust this creator and install' in text:
                    break
                time.sleep(.1)
            else:
                raise AssertionError('Verified package review did not appear: ' + text)
            assert 'com.gamepkg.minimal' in text and 'creator fingerprint:' in text.lower(), text
            assert not list((root / 'library').glob('*.gamepkg')), 'Inspection installed without approval'
            click(next(w for w in words if w['text'] == 'Trust'))
            installed = root / 'library/com.gamepkg.minimal.gamepkg'
            for _ in range(50):
                if installed.exists():
                    break
                time.sleep(.1)
            assert installed.exists(), 'Approved GUI install did not publish the package'
            time.sleep(.3)
            text, _ = screen()
            assert 'com.gamepkg.minimal' in text and '0.1.0' in text, text
            print('GitHub desktop download passed: project URL, verified creator review, explicit install, and library row.')
        finally:
            app.terminate()
            app.wait(timeout=10)
            display.terminate()
            display.wait(timeout=10)
