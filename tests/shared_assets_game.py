"""Exercise signed store packages and large shared chunk reads in both runtimes."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-shared-') as directory:
    root = Path(directory)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/minimal', source)
    data = source / 'assets/large.bin'
    data.write_bytes(b'0' * (16 * 1024 * 1024 - 3) + b'ABC')
    (source / 'source/src/lib.rs').write_text('''pub fn game_tick(_: u32, _: u64) -> u64 {
        let mut buffer = [0; 8];
        assert_eq!(gamepkg::asset_read("large.bin", 16777213, &mut buffer), Some(3));
        assert_eq!(&buffer[..3], b"ABC");
        assert_eq!(gamepkg::asset_read("large.bin", 16777216, &mut buffer), Some(0));
        assert_eq!(gamepkg::asset_read("large.bin", 16777217, &mut buffer), None);
        assert_eq!(gamepkg::asset_read("../large.bin", 0, &mut buffer), None);
        42u64 << 32
    }''')
    key = root / 'creator.key'
    original = (source / 'manifest.toml').read_text()
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args, success=True, environment=env):
        result = subprocess.run([str(CLI), *map(str,args)],env=environment,capture_output=True,text=True,timeout=120)
        assert (result.returncode == 0) == success, result.stderr
        return result.stdout
    run('keygen','-o',key)
    for profile in ['core-v1','component-v1']:
        (source/'manifest.toml').write_text(f'runtime_profile = "{profile}"\n'+original.replace('com.gamepkg.minimal',f'com.gamepkg.{profile}'))
        package = root / f'{profile}.gamepkg'
        run('pack',source,'-o',package,'--key',key,'--shared-assets')
        assert package.stat().st_size < 1024 * 1024, 'assets remained embedded'
        run('verify',package)
        missing = dict(env,GAMEPKG_HOME=str(root/'missing-library'))
        run('install',package,'--trust-creator',success=False,environment=missing)
        run('install',package,'--trust-creator')
        run('build',f'com.gamepkg.{profile}')
        output=run('run',f'com.gamepkg.{profile}','--frames','2')
        assert '42' in output, output
    bundle = root/'bundle.gamepkgstore'
    run('store-export',root/'core-v1.gamepkg',root/'component-v1.gamepkg','-o',bundle)
    import functools, http.server, ssl, threading
    cert, tls_key = root/'cert.pem', root/'tls.key'
    subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(tls_key),'-out',str(cert),'-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost'],check=True,capture_output=True)
    class Handler(http.server.SimpleHTTPRequestHandler):
        def do_GET(self):
            if self.path != '/bundle.gamepkgstore': self.send_error(404); return
            super().do_GET()
        def log_message(self,*args): pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(Handler,directory=str(root)))
    context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(cert,tls_key)
    server.socket=context.wrap_socket(server.socket,server_side=True)
    worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
    recipient=dict(env,GAMEPKG_HOME=str(root/'recipient-library'),CURL_CA_BUNDLE=str(cert))
    url=f'https://localhost:{server.server_port}/bundle.gamepkgstore'
    try:
        report=run('store-inspect',url,environment=recipient)
        assert 'com.gamepkg.core-v1' in report and 'com.gamepkg.component-v1' in report
        assert not (root/'recipient-library').exists(), 'inspection installed games'
        run('store-import',url,success=False,environment=recipient)
        assert not (root/'recipient-library').exists(), 'unapproved bundle changed the library'
        run('store-import',url,'--trust-creators',environment=recipient)
        for profile in ['core-v1','component-v1']:
            run('build',f'com.gamepkg.{profile}',environment=recipient)
            assert '42' in run('run',f'com.gamepkg.{profile}','--frames','2',environment=recipient)
        assert 'Installed 0 store games' in run('store-import',url,environment=recipient)
    finally:
        server.shutdown();server.server_close();worker.join(timeout=5)
    assert len(list((root/'library/asset-pool').glob('*.blob'))) == 2
    print('HTTPS bundle transfer and recipient gameplay passed. Shared assets passed: signed thin packages, missing-blob refusal, deduplication, and large range reads in core and WIT games.')
