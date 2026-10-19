"""Mock SOCKS5 integration: tests remote DNS/routing, not the live Tor network."""
import os
from pathlib import Path
import shutil
import socketserver
import subprocess
import tempfile
import threading

PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
HOST = 'a' * 56 + '.onion'
seen = []
response = b''
redirect = False
errors = []

def read_exact(stream, length):
    result = b''
    while len(result) < length:
        data = stream.recv(length - len(result))
        if not data:
            raise RuntimeError('unexpected EOF')
        result += data
    return result

class Proxy(socketserver.BaseRequestHandler):
    def handle(self):
        try:
            self.request.settimeout(10)
            version, count = read_exact(self.request, 2)
            assert version == 5
            read_exact(self.request, count)
            self.request.sendall(b'\x05\x00')
            version, command, _, address_type = read_exact(self.request, 4)
            assert (version, command, address_type) == (5, 1, 3), 'DNS must resolve at SOCKS proxy'
            length = read_exact(self.request, 1)[0]
            host = read_exact(self.request, length).decode()
            port = int.from_bytes(read_exact(self.request, 2), 'big')
            assert host == HOST and port == 80
            seen.append(host)
            self.request.sendall(b'\x05\x00\x00\x01\x7f\x00\x00\x01\x00\x50')
            data = b''
            while b'\r\n\r\n' not in data:
                data += self.request.recv(4096)
            if redirect:
                self.request.sendall(b'HTTP/1.1 302 Found\r\nLocation: https://example.invalid/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n')
            else:
                header = f'HTTP/1.1 200 OK\r\nContent-Length: {len(response)}\r\nConnection: close\r\n\r\n'.encode()
                self.request.sendall(header + response)
        except Exception as error:
            errors.append(str(error))

with tempfile.TemporaryDirectory(prefix='gamepkg-tor-') as directory:
    root = Path(directory)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/minimal', source)
    original = (source / 'manifest.toml').read_text()
    key = root / 'creator.key'
    key.write_text('05' * 32)
    package = root / 'test.gamepkg'
    url = f'http://{HOST}/latest.gamepkg'
    server = socketserver.ThreadingTCPServer(('127.0.0.1', 0), Proxy)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    config = root / 'settings.toml'
    config.write_text(f"tor_proxy='127.0.0.1:{server.server_address[1]}'\n")
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'), GAMEPKG_CONFIG=str(config),
               NO_PROXY='*', no_proxy='*')
    env.pop('GAMEPKG_TOR_PROXY', None)
    def run(*args, success=True):
        result = subprocess.run([str(CLI), *map(str, args)], env=env,
                                capture_output=True, text=True, timeout=30)
        assert (result.returncode == 0) == success, result.stderr
        return result
    def publish(version):
        global response
        (source / 'manifest.toml').write_text(f'update_url = "{url}"\n' + original.replace('version = "0.1.0"', f'version = "{version}"'))
        package.unlink(missing_ok=True)
        run('pack', source, '-o', package, '--key', key)
        response = package.read_bytes()
    try:
        publish('0.1.0')
        run('install', url, '--trust-creator')
        publish('0.2.0')
        assert 'Update available' in run('check-updates', 'com.gamepkg.minimal').stdout
        assert '0.1.0' in run('list').stdout
        run('update', 'com.gamepkg.minimal')
        assert '0.2.0' in run('list').stdout
        redirect = True
        result = run('check-updates', 'com.gamepkg.minimal', success=False)
        assert 'cannot redirect to an ordinary host' in result.stderr
        assert len(seen) == 4
        assert not errors, errors
        print('Tor routing passed: remote DNS, NO_PROXY override, install/update, and ordinary-host redirect rejection.')
    finally:
        server.shutdown()
        server.server_close()
        worker.join()
