"""Local HTTPS download/update integration check. Requires curl and openssl.
Run after building: python3 tests/https_download.py
"""
import functools
import http.server
import os
from pathlib import Path
import shutil
import ssl
import subprocess
import tempfile
import threading

PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'

with tempfile.TemporaryDirectory(prefix='gamepkg-https-') as directory:
    root = Path(directory)
    cert, tls_key = root / 'cert.pem', root / 'tls.key'
    subprocess.run([
        'openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes',
        '-keyout', str(tls_key), '-out', str(cert), '-days', '1',
        '-subj', '/CN=localhost', '-addext', 'subjectAltName=DNS:localhost',
    ], check=True, capture_output=True)
    web = root / 'web'
    web.mkdir()
    class Handler(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *args):
            pass
    server = http.server.ThreadingHTTPServer(
        ('127.0.0.1', 0), functools.partial(Handler, directory=str(web)))
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(cert, tls_key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'),
               CURL_CA_BUNDLE=str(cert))
    url = f'https://localhost:{server.server_port}/latest.gamepkg'
    seed = root / 'creator.key'
    seed.write_text('05' * 32)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/minimal', source)
    original = (source / 'manifest.toml').read_text()
    def run(*args, success=True):
        result = subprocess.run([str(CLI), *map(str, args)], env=env,
                                capture_output=True, text=True, timeout=30)
        assert (result.returncode == 0) == success, result.stderr
        return result
    def publish(version, key=seed, package_id='com.gamepkg.minimal'):
        (source / 'manifest.toml').write_text(
            f'update_url = "{url}"\n' + original.replace(
                'version = "0.1.0"', f'version = "{version}"').replace(
                'com.gamepkg.minimal', package_id))
        output = web / 'latest.gamepkg'
        output.unlink(missing_ok=True)
        run('pack', source, '-o', output, '--key', key)
    try:
        publish('0.1.0')
        run('install', url, success=False)
        catalog_url = f'https://localhost:{server.server_port}/games.gamepkglist'
        (web / 'games.gamepkglist').write_text(f"format_version=1\n[[games]]\nid='com.gamepkg.minimal'\ntitle='Minimal'\ndownload_url='{url}'\n[[games]]\nid='com.gamepkg.wrong-id'\ntitle='Wrong ID'\ndownload_url='{url}'\n")
        run('import-list', catalog_url)
        assert 'Not downloaded' in run('list').stdout
        run('download', 'com.gamepkg.wrong-id', '--trust-creator', success=False)
        run('download', 'com.gamepkg.minimal', '--trust-creator')
        assert '0.1.0' in run('list').stdout
        publish('0.2.0')
        assert 'Update available' in run('check-updates', 'com.gamepkg.minimal').stdout
        assert '0.1.0' in run('list').stdout
        run('update', 'com.gamepkg.minimal')
        assert '0.2.0' in run('list').stdout
        assert 'up to date' in run('check-updates', 'com.gamepkg.minimal').stdout
        other_key = root / 'other.key'
        other_key.write_text('06' * 32)
        publish('9.0.0', other_key)
        run('update', 'com.gamepkg.minimal', success=False)
        publish('9.0.0', package_id='com.gamepkg.other')
        run('update', 'com.gamepkg.minimal', success=False)
        assert '0.2.0' in run('list').stdout
        print('HTTPS download/install/update passed; signer and package-ID substitution rejected.')
    finally:
        server.shutdown()
        server.server_close()
        worker.join()
