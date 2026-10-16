"""Verify the isolated compiler exchange without transporting player data."""
import hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
PROJECT=Path(__file__).resolve().parents[1];CLI=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-worker-test-') as directory:
 root=Path(directory);home=root/'library';config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(config))
 def run(*args,good=True):
  r=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert (r.returncode==0)==good,r.stdout+r.stderr;return r.stdout+r.stderr
 key=root/'key';run('keygen','-o',key);package=root/'game.gamepkg';store=root/'game.gamepkgstore'
 run('pack',PROJECT/'examples/good/shader-switch','-o',package,'--key',key,'--shared-assets')
 run('install',package,'--trust-creator')
 (home/'saves').mkdir();(home/'saves/private').write_text('private save')
 (home/'progress').mkdir();(home/'progress/private').write_text('private history')
 run('prepare-build-worker','com.gamepkg.shader-switch',store)
 import zipfile
 with zipfile.ZipFile(store) as bundle:
  assert all(name=='store.toml' or name.startswith(('packages/','blobs/')) for name in bundle.namelist())
 before=store.read_bytes();run('prepare-build-worker','com.gamepkg.shader-switch',store,good=False);assert store.read_bytes()==before
 missing=root/'missing.gamepkgstore';run('prepare-build-worker','unknown',missing,good=False);assert not missing.exists()
 output=root/'output'
 saved_path=env['PATH'];env['PATH']='/usr/bin:/bin'
 try:run('build-worker',store,output)
 finally:env['PATH']=saved_path
 info=json.loads((output/'exchange.json').read_text());assert info['package_digest']==hashlib.sha256(package.read_bytes()).hexdigest();assert info['id']=='com.gamepkg.shader-switch' and info['backend']=='glsl'
 assert {p.name for p in output.iterdir()}=={'exchange.json','builds','shader-builds'}
 assert not list(output.rglob('*.key')) and not (output/'progress').exists() and not (output/'saves').exists()
 run('store-import',store,'--trust-creators')
 run('accept-build-worker','com.gamepkg.shader-switch',output)
 protected={p:p.read_bytes() for p in (home/'builds').rglob('*') if p.is_file()}
 def reject(change):
  candidate=root/'candidate';shutil.copytree(output,candidate)
  change(candidate)
  run('accept-build-worker','com.gamepkg.shader-switch',candidate,good=False)
  assert all(p.read_bytes()==data for p,data in protected.items())
  shutil.rmtree(candidate)
 def metadata(field,value):
  def change(candidate):
   path=candidate/'exchange.json';data=json.loads(path.read_text());data[field]=value;path.write_text(json.dumps(data))
  return change
 for field,value in [('id','wrong'),('package_digest','0'*64),('backend','wgsl'),('format_version',2),('compiler_host_version','wrong'),('unknown',True)]:reject(metadata(field,value))
 def damage(pattern):
  def change(candidate):
   path=next(candidate.rglob(pattern));path.write_bytes(path.read_bytes()+b'broken')
  return change
 for pattern in ['game.wasm','game.sha256','*.reflection.json','*.shader']:reject(damage(pattern))
 reject(lambda candidate:next(candidate.rglob('shaders-glsl.json')).unlink())
 reject(lambda candidate:(next((candidate/'builds').iterdir())/'extra').write_text('extra'))
 reject(lambda candidate:(next((candidate/'builds').iterdir())/'nested').mkdir())
 def linked(candidate):
  path=next(candidate.rglob('game.wasm'));path.unlink();path.symlink_to(package)
 reject(linked)
 run('accept-build-worker','com.gamepkg.shader-switch',output)
 transport=root/'transport';run('build-worker',store,transport,'--copy-output')
 run('accept-build-worker','com.gamepkg.shader-switch',transport)
 run('build-worker',store,transport,'--copy-output',good=False)
 assert (transport/'exchange.json').exists()
 # Loading the transferred cache needs no compiler or private worker library.
 env.pop('GAMEPKG_SLANG_ROOT',None)
 run('run','com.gamepkg.shader-switch','--frames','3')
 run('integrity-check','com.gamepkg.shader-switch')
 marker=output/'keep';marker.write_text('unchanged');run('build-worker',store,output,good=False);assert marker.read_text()=='unchanged'
 bad=root/'bad';run('build-worker',store,bad,'--shader-backend','unknown',good=False);assert not bad.exists()
 other=root/'other.gamepkg';run('pack',PROJECT/'examples/good/collect-stars','-o',other,'--key',key)
 multi=root/'multi.gamepkgstore';run('store-export',package,other,'-o',multi);run('build-worker',multi,root/'multi-output',good=False);assert not (root/'multi-output').exists()
 assert '--no-approved-crates' not in run('build-worker','--help')
 dependency=root/'dependency-source';shutil.copytree(PROJECT/'examples/good/byteorder-game',dependency)
 with (dependency/'source/Cargo.toml').open('a') as cargo:cargo.write('\n[dependencies]\nserde="1"\n')
 rejected=root/'dependency.gamepkg';denied=run('pack',dependency,'-o',rejected,'--key',key,good=False)
 assert 'guest crates' in denied and not rejected.exists(),denied
 print('Worker exchange passed: verified shared assets, compilation without a login PATH, package binding, transferred shader/Wasm playback without a compiler, no player data, and atomic failure checks.')
