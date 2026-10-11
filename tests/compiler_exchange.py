"""Prepare or consume genuine isolated compiler exchanges across native hosts."""
import argparse,hashlib,json,os,subprocess,tempfile
from pathlib import Path
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser(description=__doc__);p.add_argument('mode',choices=['prepare','consume']);p.add_argument('directory',type=Path);args=p.parse_args()
cli=PROJECT/'target/release'/('gamepkg.exe' if os.name=='nt' else 'gamepkg')
cases=[('core-glsl','shader-switch','glsl'),('component-glsl','component-playground','glsl'),('component-spirv','component-playground','spirv'),('component-wgsl','component-playground','wgsl')]
with tempfile.TemporaryDirectory(prefix='gamepkg-native-exchange-') as temporary:
 root=Path(temporary);config=root/'settings.toml'
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config))
 def run(*arguments,good=True):
  result=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180)
  assert (result.returncode==0)==good,result.stdout+result.stderr
  return result.stdout+result.stderr
 fixture=args.directory.resolve()
 if args.mode=='prepare':
  fixture.mkdir(parents=True,exist_ok=False)
  key=root/'creator.key';run('keygen','-o',key)
 else:env.pop('GAMEPKG_SLANG_ROOT',None)
 for case,example,backend in cases:
  folder=fixture/case;game_id='com.gamepkg.'+example
  config.write_text(f'automatic_update_checks=false\nshader_backend="{backend}"\n')
  if args.mode=='prepare':
   folder.mkdir();package=root/(case+'.gamepkg')
   run('pack',PROJECT/'examples/good'/example,'-o',package,'--key',key,'--shared-assets')
   if backend=='glsl':run('install',package,'--trust-creator')
   run('prepare-build-worker',game_id,folder/'game.gamepkgstore')
   run('build-worker',folder/'game.gamepkgstore',folder/'exchange','--shader-backend',backend,'--copy-output')
   assert not list(folder.rglob('*.key'))
  else:
   run('store-import',folder/'game.gamepkgstore','--trust-creators')
   run('accept-build-worker',game_id,folder/'exchange')
   run('run',game_id,'--frames','3')
   run('integrity-check',game_id)
   info=folder/'exchange/exchange.json';original=info.read_bytes();metadata=json.loads(original);metadata['package_digest']='0'*64
   info.write_text(json.dumps(metadata))
   try:run('accept-build-worker',game_id,folder/'exchange',good=False)
   finally:info.write_bytes(original)
   other='component-glsl' if case=='core-glsl' else 'core-glsl'
   replacement=next((fixture/other/'exchange/builds').rglob('game.wasm')).read_bytes()
   wasm=next((folder/'exchange/builds').rglob('game.wasm'));receipt=wasm.with_suffix('.sha256')
   original_wasm=wasm.read_bytes();original_receipt=receipt.read_bytes()
   wasm.write_bytes(replacement);receipt.write_text(hashlib.sha256(replacement).hexdigest())
   try:run('accept-build-worker',game_id,folder/'exchange',good=False)
   finally:wasm.write_bytes(original_wasm);receipt.write_bytes(original_receipt)
   run('run',game_id,'--frames','3')
  print('Cross-platform compiler exchange passed:',args.mode,case)
