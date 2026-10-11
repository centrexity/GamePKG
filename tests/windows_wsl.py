"""Opt-in actual Windows/WSL 2 build, playback and integrity-repair integration."""
import argparse,json,os,platform,subprocess,tempfile
from pathlib import Path
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--distribution',required=True);p.add_argument('--compiler',required=True);args=p.parse_args()
assert platform.system()=='Windows','This test requires Windows and a configured WSL 2 compiler'
cli=PROJECT/'target/release/gamepkg.exe'
with tempfile.TemporaryDirectory(prefix='gamepkg-wsl-test-') as directory:
 root=Path(directory);home=root/'library';config=root/'settings.toml'
 config.write_text('automatic_update_checks=false\nwindows_compiler_distribution='+json.dumps(args.distribution)+'\nwindows_compiler_executable='+json.dumps(args.compiler)+'\n')
 env=dict(os.environ,GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(config))
 def run(*arguments,good=True):
  result=subprocess.run([str(cli),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=600)
  assert (result.returncode==0)==good,result.stdout+result.stderr
  return result.stdout+result.stderr
 key=root/'creator.key';run('keygen','-o',key)
 for example in ['shader-switch','component-playground']:
  package=root/(example+'.gamepkg');game_id='com.gamepkg.'+example
  run('pack',PROJECT/'examples/good'/example,'-o',package,'--key',key,'--shared-assets')
  run('install',package,'--trust-creator');run('build',game_id)
  assert 'Ran 3 frames' in run('run',game_id,'--frames','3')
  run('integrity-check',game_id)
  import hashlib
  build=home/'builds'/hashlib.sha256(package.read_bytes()).hexdigest()/'game.wasm'
  build.write_bytes(build.read_bytes()+b'broken')
  run('integrity-check',game_id,good=False)
  run('integrity-fix',game_id)
  run('integrity-check',game_id)
  assert 'Ran 3 frames' in run('run',game_id,'--frames','3')
  assert not (home/'.install-lock').exists()
print('Actual Windows/WSL 2 source compilation, shared assets, core/component playback and integrity repair passed.')
