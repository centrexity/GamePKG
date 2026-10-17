"""Maintainer test: install, build once, remember data, play and verify a signed package."""
from pathlib import Path
import hashlib, json, os, subprocess, tempfile, sys, shutil
PROJECT=Path(__file__).resolve().parents[1]
root=PROJECT/'target/cube2-package-test'
root.mkdir(parents=True,exist_ok=True)
config=root/'settings.toml';config.write_text('memory_ceiling_mib=2048\ntick_deadline_ms=500\nautomatic_update_checks=false\n')
env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config))
cli=PROJECT/'target/release/gamepkg'
def run(*args,timeout=120):
 result=subprocess.run([str(cli),*map(str,args)],env=env,text=True,capture_output=True,timeout=timeout)
 assert result.returncode==0,(args,result.stdout,result.stderr)
 return result.stdout
key=root/'test.key'
if not key.exists():key.write_text('7b'*32)
package=root/'cube2.gamepkg'
if '--reuse-build' not in sys.argv:
 if (root/'library').exists():shutil.rmtree(root/'library')
 if package.exists():package.unlink()
if not package.exists():run('pack',PROJECT/'ports/cube2','-o',package,'--key',key)
if not (root/'library/com.gamepkg.cube2.gamepkg').exists():run('install',package,'--trust-creator')
if '--reuse-build' not in sys.argv:run('build','com.gamepkg.cube2')
version=run('--version').strip().split()[-1]
fingerprint=hashlib.sha256(b''.join((PROJECT/p).read_bytes() for p in ['engines/cube2/upstream-lock.json','engines/cube2/adapters/frame_bridge.h','engines/cube2/adapters/audio.h','engines/cube2/adapters/persistence.h','engines/cube2/adapters/network.h','engines/cube2/adapters/syscall_guard.h','src/cube2/build.rs'])).hexdigest()[:16]
artifacts=[p for p in (root/'library/builds').rglob('cube2-*') if p.name=='cube2-'+fingerprint and any(part.startswith('native-v1-'+version+'-') for part in p.parts)]
assert len(artifacts)==1,artifacts
binary=artifacts[0];stamp=binary.stat().st_mtime_ns;digest=hashlib.sha256(binary.read_bytes()).hexdigest()
run('run','com.gamepkg.cube2','--game-data',PROJECT/'target/cube2-test-data','--frames','12')
run('run','com.gamepkg.cube2','--frames','3')
assert binary.stat().st_mtime_ns==stamp and hashlib.sha256(binary.read_bytes()).hexdigest()==digest,'Play rebuilt the engine'
assert (root/'library/game-data-selections.json').is_file()
saves=list((root/'library/saves').glob('*.cube2'));assert saves and saves[0].read_bytes().startswith(b'GPS2')
records=list((root/'library/progress').glob('*.json'));assert records
assert json.loads(records[0].read_text())['played_ms']>0
run('integrity-check','com.gamepkg.cube2')
original=binary.read_bytes()
try:
 corrupted=bytearray(original);corrupted[-1]^=1;binary.write_bytes(corrupted)
 rejected=subprocess.run([str(cli),'run','com.gamepkg.cube2','--frames','1'],env=env,capture_output=True,text=True,timeout=30)
 assert rejected.returncode!=0 and 'integrity' in rejected.stderr.lower(),rejected.stderr
finally:binary.write_bytes(original)
run('integrity-check','com.gamepkg.cube2')
network=root/'library/network-selections.json'
installed=root/'library/com.gamepkg.cube2.gamepkg';signed_digest=hashlib.sha256(installed.read_bytes()).hexdigest()
before=network.read_bytes() if network.exists() else None
denied=subprocess.run([str(cli),'configure-network','com.gamepkg.cube2','--server','127.0.0.1:28785'],env=env,capture_output=True,text=True)
assert denied.returncode!=0 and (network.read_bytes() if network.exists() else None)==before
run('configure-network','com.gamepkg.cube2','--server','127.0.0.1:28785','--allow-local')
choices=list(json.loads(network.read_text()).values());assert len(choices)==1 and choices[0]['server']=='127.0.0.1:28785' and choices[0]['allow_local']
run('configure-network','com.gamepkg.cube2','--clear');assert not json.loads(network.read_text())
run('configure-network','com.gamepkg.cube2','--master','master.example:28787')
assert list(json.loads(network.read_text()).values())[0]['master']=='master.example:28787'
run('configure-network','com.gamepkg.cube2','--clear');assert not json.loads(network.read_text())
assert hashlib.sha256(installed.read_bytes()).hexdigest()==signed_digest,'Server choices changed the signed package'
print('Signed Cube 2 package: install, native build, cached playback, remembered read-only data, scoped save and progress passed.')
print(root)
