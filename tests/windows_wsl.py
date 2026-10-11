"""Opt-in actual Windows/WSL 2 build, playback and integrity-repair integration."""
import argparse,json,os,platform,shutil,subprocess,tempfile
from pathlib import Path
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--distribution',required=True);p.add_argument('--compiler',required=True);p.add_argument('--gui',action='store_true');args=p.parse_args()
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
 for profile in ['core','component']:
  source=root/profile;shutil.copytree(PROJECT/'examples/good/shader-switch',source)
  game_id='com.gamepkg.shader-switch'
  if profile=='component':
   game_id='com.gamepkg.component-shader-switch'
   manifest=source/'manifest.toml';manifest.write_text('runtime_profile="component-v1"\n'+manifest.read_text().replace('com.gamepkg.shader-switch',game_id))
  package=root/(profile+'.gamepkg')
  run('pack',source,'-o',package,'--key',key,'--shared-assets')
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
  print('Windows WSL build, CLI playback and repair passed:',profile,flush=True)
  if args.gui:
   from windows_gui_support import verify_shader_input
   verify_shader_input(PROJECT,env,game_id,profile)
print('Actual Windows/WSL 2 source compilation, shared assets, core/component playback and integrity repair passed.')
