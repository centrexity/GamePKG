"""Opt-in live Tor install/update test; requires a supplied Tor binary and network access."""
import subprocess,tempfile,time,threading,http.server,functools,os,shutil,argparse,socket
from pathlib import Path
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--tor',type=Path,required=True);args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
with socket.socket() as reserved:
 reserved.bind(('127.0.0.1',0));socks_port=reserved.getsockname()[1]
root=Path(tempfile.mkdtemp(prefix='gamepkg-live-onion-'));root.chmod(0o700)
handler=functools.partial(http.server.SimpleHTTPRequestHandler,directory=str(root/'public'))
(root/'public').mkdir();server=http.server.ThreadingHTTPServer(('127.0.0.1',0),handler);threading.Thread(target=server.serve_forever,daemon=True).start()
log=(root/'tor.log').open('w')
tor=subprocess.Popen([str(args.tor.resolve()),'--SocksPort',f'127.0.0.1:{socks_port}','--DataDirectory',str(root/'data'),'--HiddenServiceDir',str(root/'onion'),'--HiddenServicePort',f'80 127.0.0.1:{server.server_port}','--Log','notice stdout'],stdout=log,stderr=subprocess.STDOUT)
try:
 deadline=time.monotonic()+180
 while 'Bootstrapped 100%' not in (root/'tor.log').read_text():
  assert tor.poll() is None,'Tor exited: '+(root/'tor.log').read_text()
  assert time.monotonic()<deadline,'Tor bootstrap timed out: '+(root/'tor.log').read_text()
  time.sleep(1)
 host=(root/'onion/hostname').read_text().strip();url='http://'+host+'/latest.gamepkg'
 source=root/'source';shutil.copytree(project/'examples/minimal',source);original=(source/'manifest.toml').read_text();key=root/'creator.key';key.write_text('07'*32);key.chmod(0o600)
 config=root/'settings.toml';config.write_text(f"tor_proxy='127.0.0.1:{socks_port}'\nautomatic_update_checks=false\n")
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),NO_PROXY='*',no_proxy='*');env.pop('GAMEPKG_TOR_PROXY',None)
 cli=str(project/'target/release/gamepkg')
 def run(*args):
  r=subprocess.run([cli,*map(str,args)],env=env,capture_output=True,text=True,timeout=120);assert r.returncode==0,r.stderr;return r.stdout
 def publish(version):
  manifest='update_url = "'+url+'"\n'+original.replace('version = "0.1.0"','version = "'+version+'"');(source/'manifest.toml').write_text(manifest)
  package=root/'public/latest.gamepkg';package.unlink(missing_ok=True);run('pack',source,'-o',package,'--key',key)
 publish('0.1.0');time.sleep(10);run('install',url,'--trust-creator')
 publish('0.2.0');assert 'Update available' in run('check-updates','com.gamepkg.minimal');run('update','com.gamepkg.minimal');assert '0.2.0' in run('list')
 print('Live Tor v3 service: signed install, update check, same-creator update and NO_PROXY override passed.',flush=True)
finally:
 tor.terminate();tor.wait(timeout=15);log.close();server.shutdown()
 shutil.rmtree(root)
