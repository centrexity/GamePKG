"""Prepare or consume a genuine isolated compiler exchange across native hosts."""
import argparse,json,os,subprocess,tempfile
from pathlib import Path
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser(description=__doc__);p.add_argument('mode',choices=['prepare','consume']);p.add_argument('directory',type=Path);args=p.parse_args()
cli=PROJECT/'target/release'/('gamepkg.exe' if os.name=='nt' else 'gamepkg')
with tempfile.TemporaryDirectory(prefix='gamepkg-native-exchange-') as temporary:
 root=Path(temporary);env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*arguments,good=True):
  result=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=180)
  assert (result.returncode==0)==good,result.stdout+result.stderr
  return result.stdout+result.stderr
 fixture=args.directory.resolve()
 if args.mode=='prepare':
  fixture.mkdir(parents=True,exist_ok=False)
  key=root/'creator.key';package=root/'game.gamepkg'
  run('keygen','-o',key)
  run('pack',PROJECT/'examples/good/shader-switch','-o',package,'--key',key,'--shared-assets')
  run('store-export',package,'-o',fixture/'game.gamepkgstore')
  run('build-worker',fixture/'game.gamepkgstore',fixture/'exchange')
  assert not list(fixture.rglob('*.key'))
 else:
  env.pop('GAMEPKG_SLANG_ROOT',None)
  run('store-import',fixture/'game.gamepkgstore','--trust-creators')
  run('accept-build-worker','com.gamepkg.shader-switch',fixture/'exchange')
  run('run','com.gamepkg.shader-switch','--frames','3')
  run('integrity-check','com.gamepkg.shader-switch')
  info=fixture/'exchange/exchange.json';original=info.read_bytes();metadata=json.loads(original);metadata['package_digest']='0'*64
  info.write_text(json.dumps(metadata))
  try:run('accept-build-worker','com.gamepkg.shader-switch',fixture/'exchange',good=False)
  finally:info.write_bytes(original)
  run('run','com.gamepkg.shader-switch','--frames','3')
print('Cross-platform compiler exchange passed:',args.mode)
