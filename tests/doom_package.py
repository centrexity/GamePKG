"""Pinned DOOM package/build/worker boundaries, without commercial game data."""
from pathlib import Path
import os,shutil,subprocess,tempfile
PROJECT=Path(__file__).resolve().parents[1]
CLI=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-package-') as temp:
 root=Path(temp);config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config))
 def run(*args,good=True):
  r=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=180)
  assert (r.returncode==0)==good,r.stdout+r.stderr
  return r.stdout+r.stderr
 key=root/'key';run('keygen','-o',key);package=root/'doom.gamepkg'
 run('pack',PROJECT/'ports/doom','-o',package,'--key',key);run('verify',package);run('install',package,'--trust-creator');run('build','com.gamepkg.doom')
 run('integrity-check','com.gamepkg.doom')
 assert 'ROM' in run('run','com.gamepkg.doom','--frames','1',good=False)
 bad_wad=root/'invalid.wad';bad_wad.write_bytes(b'IWAD'+b'\xff'*8)
 assert 'WAD' in run('run','com.gamepkg.doom','--rom',bad_wad,'--frames','1',good=False)
 config.write_text('automatic_update_checks=false\nmemory_ceiling_mib=64\n')
 assert 'memory ceiling' in run('run','com.gamepkg.doom','--rom',bad_wad,'--frames','1',good=False)
 config.write_text('automatic_update_checks=false\n')
 store=root/'request.gamepkgstore';run('prepare-build-worker','com.gamepkg.doom',store)
 exchange=root/'exchange';run('build-worker',store,exchange)
 engines=list(exchange.rglob('doom.wasm'));assert len(engines)==1 and not list(exchange.rglob('zelda3.wasm'))
 run('accept-build-worker','com.gamepkg.doom',exchange)
 assert 'native player' in run('web-export','com.gamepkg.doom','-o',root/'web',good=False)
 engine=next((root/'library/builds').glob('*/doom.wasm'));original=engine.read_bytes();engine.write_bytes(original+b'corrupt')
 assert 'built' not in run('list').split('com.gamepkg.doom',1)[-1].lower().replace('unbuilt','').replace('not built','')
 run('integrity-check','com.gamepkg.doom',good=False)
 run('accept-build-worker','com.gamepkg.doom',exchange);run('integrity-check','com.gamepkg.doom')
 print('DOOM package passed: dependency-free signed recipe, isolated engine build, WAD/memory rejection, scoped worker transfer, browser rejection and corrupt-cache repair without any commercial assets.')
