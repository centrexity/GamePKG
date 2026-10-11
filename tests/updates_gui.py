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
import time
from PIL import Image

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
    requests=[]
    class Handler(http.server.SimpleHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            super().do_GET()
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
               CURL_CA_BUNDLE=str(cert), GAMEPKG_CONFIG=str(root/'settings.toml'))
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
    display=None
    try:
        publish('0.1.0');run('install',web/'latest.gamepkg','--trust-creator')
        installed=root/'library/com.gamepkg.minimal.gamepkg'
        original_installed=installed.read_bytes()
        publish('0.2.0')
        display=subprocess.Popen(['Xvfb',':97','-screen','0','1600x900x24','-ac'],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
        time.sleep(1)
        env.update(DISPLAY=':97',LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
        for enabled in [True,False]:
            (root/'settings.toml').write_text(f'automatic_update_checks = {str(enabled).lower()}\nupdate_interval_hours = 1\n')
            requests.clear()
            app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            try:
                window=None
                for _ in range(100):
                    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
                    state=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
                    if state.returncode==0:
                        window=state.stdout.strip().splitlines()[0];break
                    time.sleep(.1)
                assert window,'GUI window did not become visible'
                subprocess.run(['xdotool','windowsize',window,'1500','850'],env=env,check=True)
                screenshot=PROJECT/('target/automatic-updates.png' if enabled else 'target/automatic-updates-disabled.png')
                def green_notice():
                    subprocess.run(['import','-display',':97','-window',window,str(screenshot)],env=env,check=True)
                    image=Image.open(screenshot).convert('RGB')
                    return sum(1 for y in range(min(250,image.height)) for x in range(image.width) if (lambda c:c[0]<50 and c[1]>200 and c[2]<50)(image.getpixel((x,y))))>10
                if enabled:
                    for _ in range(30):
                        if requests and green_notice():break
                        time.sleep(.2)
                    assert requests and green_notice(),'Verified update notice was not rendered'
                    assert len(requests)==1,requests
                    time.sleep(.5);assert len(requests)==1,'Update checker retried before its interval'
                else:
                    time.sleep(1.5)
                    assert not requests,'Disabled update checks contacted the server'
                    assert not green_notice(),'Disabled fresh session showed an update notice'
                assert installed.read_bytes()==original_installed,'Automatic check installed an update'
            finally:
                app.terminate();app.wait(timeout=5)
        print('GUI periodic checks verified: signed update notice, installed package preserved, deadline respected, disabled checks make no requests.')
    finally:
        if display is not None:display.terminate();display.wait(timeout=5)
        server.shutdown();server.server_close();worker.join()
